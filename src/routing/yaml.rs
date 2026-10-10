//! 有界 YAML 事件直接反序列化：不构造整份中间 JSON 树，先检查深度和重复键。
use crate::error::{Error, Result};
use serde::de::{self, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::{collections::HashSet, str::Chars};
use yaml_rust2::{
    parser::{Event, Parser},
    scanner::TScalarStyle,
};
pub(super) const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_NODES: usize = 3_000_000;
const MAX_DEPTH: usize = 32;
impl de::Error for Error {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Self::InvalidInput
    }
}
struct Decoder<'a> {
    parser: Parser<Chars<'a>>,
    pending: Option<Event>,
    nodes: usize,
    depth: usize,
}
impl Decoder<'_> {
    fn next(&mut self) -> Result<Event> {
        if let Some(event) = self.pending.take() {
            return Ok(event);
        }
        let (event, _) = self.parser.next_token().map_err(|_| Error::InvalidInput)?;
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(Error::ResourceLimit);
        }
        match &event {
            Event::Scalar(value, _, anchor, tag) => {
                if *anchor != 0 || tag.is_some() {
                    return Err(Error::InvalidInput);
                }
                if value.len() > 1024 {
                    return Err(Error::ResourceLimit);
                }
            }
            Event::MappingStart(anchor, tag) | Event::SequenceStart(anchor, tag) => {
                if *anchor != 0 || tag.is_some() {
                    return Err(Error::InvalidInput);
                }
            }
            Event::Alias(_) | Event::Nothing => return Err(Error::InvalidInput),
            _ => (),
        }
        Ok(event)
    }
    fn peek(&mut self) -> Result<&Event> {
        if self.pending.is_none() {
            self.pending = Some(self.next()?);
        }
        self.pending.as_ref().ok_or(Error::Internal)
    }
}
pub(super) fn decode<T: DeserializeOwned>(input: &str) -> Result<T> {
    if input.len() > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let mut decoder = Decoder {
        parser: Parser::new(input.chars()),
        pending: None,
        nodes: 0,
        depth: 0,
    };
    if decoder.next()? != Event::StreamStart || decoder.next()? != Event::DocumentStart {
        return Err(Error::InvalidInput);
    }
    let value = T::deserialize(&mut decoder)?;
    if decoder.next()? != Event::DocumentEnd || decoder.next()? != Event::StreamEnd {
        return Err(Error::InvalidInput);
    }
    Ok(value)
}
struct Sequence<'a, 'b> {
    decoder: &'a mut Decoder<'b>,
    count: usize,
}
impl<'de> SeqAccess<'de> for Sequence<'_, '_> {
    type Error = Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        if matches!(self.decoder.peek()?, Event::SequenceEnd) {
            self.decoder.next()?;
            return Ok(None);
        }
        self.count += 1;
        if self.count > super::MAX_RULES {
            return Err(Error::ResourceLimit);
        }
        seed.deserialize(&mut *self.decoder).map(Some)
    }
}
struct Mapping<'a, 'b> {
    decoder: &'a mut Decoder<'b>,
    keys: HashSet<String>,
    pending_value: bool,
}
impl<'de> MapAccess<'de> for Mapping<'_, '_> {
    type Error = Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        if self.pending_value {
            return Err(Error::InvalidInput);
        }
        if matches!(self.decoder.peek()?, Event::MappingEnd) {
            self.decoder.next()?;
            return Ok(None);
        }
        let Event::Scalar(key, _, _, _) = self.decoder.next()? else {
            return Err(Error::InvalidInput);
        };
        if self.keys.len() >= 64 {
            return Err(Error::ResourceLimit);
        }
        if !self.keys.insert(key.clone()) {
            return Err(Error::InvalidInput);
        }
        self.pending_value = true;
        seed.deserialize(de::value::StringDeserializer::<Error>::new(key))
            .map(Some)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        if !self.pending_value {
            return Err(Error::InvalidInput);
        }
        self.pending_value = false;
        seed.deserialize(&mut *self.decoder)
    }
}
impl<'de> de::Deserializer<'de> for &mut Decoder<'_> {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.next()? {
            Event::Scalar(value, style, _, _) => {
                let value = if style != TScalarStyle::Plain {
                    Value::String(value)
                } else if value.is_empty() || value == "~" || value == "null" {
                    Value::Null
                } else if value == "true" {
                    Value::Bool(true)
                } else if value == "false" {
                    Value::Bool(false)
                } else {
                    serde_json::from_str::<Value>(&value)
                        .ok()
                        .filter(|v| v.is_number())
                        .unwrap_or(Value::String(value))
                };
                de::Deserializer::deserialize_any(value, visitor).map_err(|_| Error::InvalidInput)
            }
            Event::SequenceStart(..) => {
                if self.depth >= MAX_DEPTH {
                    return Err(Error::ResourceLimit);
                }
                self.depth += 1;
                let result = visitor.visit_seq(Sequence {
                    decoder: self,
                    count: 0,
                });
                self.depth -= 1;
                result
            }
            Event::MappingStart(..) => {
                if self.depth >= MAX_DEPTH {
                    return Err(Error::ResourceLimit);
                }
                self.depth += 1;
                let result = visitor.visit_map(Mapping {
                    decoder: self,
                    keys: HashSet::new(),
                    pending_value: false,
                });
                self.depth -= 1;
                result
            }
            _ => Err(Error::InvalidInput),
        }
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if matches!(self.peek()?, Event::Scalar(value, TScalarStyle::Plain, _, _) if value.is_empty() || value == "~" || value == "null")
        {
            self.next()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let Event::Scalar(value, _, _, _) = self.next()? else {
            return Err(Error::InvalidInput);
        };
        visitor.visit_enum(de::value::StringDeserializer::<Error>::new(value))
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }
    serde::forward_to_deserialize_any! { bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf unit unit_struct seq tuple tuple_struct map struct identifier ignored_any }
}
/// 用标准 YAML 块集合和 JSON 兼容引号输出，避免值被当作布尔、数字或标签。
pub(super) fn emit(value: &Value) -> Result<String> {
    fn write(value: &Value, depth: usize, output: &mut String) -> Result<()> {
        if depth > MAX_DEPTH || output.len() > MAX_BYTES {
            return Err(Error::ResourceLimit);
        }
        let indent = "  ".repeat(depth);
        match value {
            Value::Object(values) if !values.is_empty() => {
                for (key, value) in values {
                    output.push_str(&indent);
                    output.push_str(&serde_json::to_string(key).map_err(|_| Error::Internal)?);
                    output.push(':');
                    if matches!(value, Value::Object(v) if !v.is_empty())
                        || matches!(value, Value::Array(v) if !v.is_empty())
                    {
                        output.push('\n');
                        write(value, depth + 1, output)?;
                    } else {
                        output.push(' ');
                        output
                            .push_str(&serde_json::to_string(value).map_err(|_| Error::Internal)?);
                        output.push('\n');
                    }
                }
            }
            Value::Array(values) if !values.is_empty() => {
                for value in values {
                    output.push_str(&indent);
                    output.push('-');
                    if matches!(value, Value::Object(v) if !v.is_empty())
                        || matches!(value, Value::Array(v) if !v.is_empty())
                    {
                        output.push('\n');
                        write(value, depth + 1, output)?;
                    } else {
                        output.push(' ');
                        output
                            .push_str(&serde_json::to_string(value).map_err(|_| Error::Internal)?);
                        output.push('\n');
                    }
                }
            }
            _ => {
                output.push_str(&indent);
                output.push_str(&serde_json::to_string(value).map_err(|_| Error::Internal)?);
                output.push('\n');
            }
        }
        if output.len() > MAX_BYTES {
            return Err(Error::ResourceLimit);
        }
        Ok(())
    }
    let mut output = String::new();
    write(value, 0, &mut output)?;
    Ok(output)
}

/// 逐条生成标准 YAML，避免把十万条规则复制到完整中间值树。
pub(super) struct Writer {
    output: String,
}
impl Writer {
    pub(super) fn new() -> Self {
        Self {
            output: String::new(),
        }
    }
    fn append(&mut self, text: &str) -> Result<()> {
        if self
            .output
            .len()
            .checked_add(text.len())
            .is_none_or(|size| size > MAX_BYTES)
        {
            return Err(Error::ResourceLimit);
        }
        self.output.push_str(text);
        Ok(())
    }
    pub(super) fn field<T: serde::Serialize>(
        &mut self,
        name: &str,
        value: &T,
        depth: usize,
    ) -> Result<()> {
        let value = serde_json::to_value(value).map_err(|_| Error::Internal)?;
        let prefix = format!(
            "{}{}:",
            "  ".repeat(depth),
            serde_json::to_string(name).map_err(|_| Error::Internal)?
        );
        self.append(&prefix)?;
        if !value.is_object() && !value.is_array() {
            return self.append(&format!(
                " {}\n",
                serde_json::to_string(&value).map_err(|_| Error::Internal)?
            ));
        }
        self.append("\n")?;
        let text = emit(&value)?;
        for line in text.lines() {
            self.append(&format!("{}{}\n", "  ".repeat(depth + 1), line))?;
        }
        Ok(())
    }
    pub(super) fn sequence<T: serde::Serialize>(
        &mut self,
        name: &str,
        values: &[T],
        depth: usize,
    ) -> Result<()> {
        self.append(&format!(
            "{}{}:",
            "  ".repeat(depth),
            serde_json::to_string(name).map_err(|_| Error::Internal)?
        ))?;
        if values.is_empty() {
            return self.append(" []\n");
        }
        self.append("\n")?;
        for value in values {
            self.append(&format!("{}-\n", "  ".repeat(depth + 1)))?;
            let value = serde_json::to_value(value).map_err(|_| Error::Internal)?;
            for line in emit(&value)?.lines() {
                self.append(&format!("{}{}\n", "  ".repeat(depth + 2), line))?;
            }
        }
        Ok(())
    }
    pub(super) fn mapping(&mut self, name: &str) -> Result<()> {
        self.append(&format!(
            "{}:\n",
            serde_json::to_string(name).map_err(|_| Error::Internal)?
        ))
    }
    pub(super) fn finish(self) -> String {
        self.output
    }
}
