//! 有界 YAML 事件直接反序列化：不构造整份中间 JSON 树，先检查深度和重复键。
use crate::error::{Error, FieldName, Result};
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
    fn unknown_field(field: &str, _: &'static [&'static str]) -> Self {
        Self::ConfigUnknownField(FieldName::new(field))
    }
    fn duplicate_field(field: &'static str) -> Self {
        Self::ConfigDuplicateField(FieldName::new(field))
    }

    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Self::InvalidInput
    }
}
struct Decoder<'a> {
    parser: Parser<Chars<'a>>,
    pending: Option<Event>,
    nodes: usize,
    depth: usize,
    scalar_limit: usize,
    path: String,
    failure: Option<(String, usize)>,
    position: usize,
    target: Option<String>,
    found: Option<usize>,
    target_mapping: Option<(String, String)>,
    found_mapping: Option<(usize, usize)>,
}
impl Decoder<'_> {
    fn next(&mut self) -> Result<Event> {
        if let Some(event) = self.pending.take() {
            return Ok(event);
        }
        let limit = std::mem::replace(&mut self.scalar_limit, 1024);
        let (event, marker) = self.parser.next_token().map_err(|error| {
            self.position = error.marker().index();
            self.capture();
            Error::ConfigYamlSyntax
        })?;
        self.position = marker.index();
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(Error::ResourceLimit);
        }
        match &event {
            Event::Scalar(value, _, anchor, tag) => {
                if *anchor != 0 || tag.is_some() {
                    return Err(Error::InvalidInput);
                }
                if value.len() > limit {
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
/// 解析诊断独立于固定错误协议，不解析或公开底层错误字符串。
pub(crate) struct DecodeIssue {
    pub error: Error,
    pub path: String,
    pub offset: usize,
}
impl Decoder<'_> {
    fn capture(&mut self) {
        if self.failure.is_none() {
            self.failure = Some((self.path.clone(), self.position));
        }
    }
    fn locate(&mut self) {
        if self.target.as_deref() == Some(self.path.as_str()) && self.found.is_none() {
            self.found = Some(self.position);
        }
    }
    fn enter(&mut self, part: &str) -> usize {
        let length = self.path.len();
        if self.path.len() + part.len() < 1024 {
            if !part.starts_with('[') && !self.path.is_empty() {
                self.path.push('.');
            }
            self.path.push_str(part);
        }
        length
    }
}
fn decoder<'a>(input: &'a str, target: Option<&str>) -> Decoder<'a> {
    Decoder {
        parser: Parser::new(input.chars()),
        pending: None,
        nodes: 0,
        depth: 0,
        scalar_limit: 1024,
        path: String::new(),
        failure: None,
        position: 0,
        target: target.map(str::to_owned),
        found: None,
        target_mapping: None,
        found_mapping: None,
    }
}
fn parse<T: DeserializeOwned>(decoder: &mut Decoder<'_>) -> Result<T> {
    if decoder.next()? != Event::StreamStart || decoder.next()? != Event::DocumentStart {
        return Err(Error::InvalidInput);
    }
    let value = T::deserialize(&mut *decoder)?;
    if decoder.next()? != Event::DocumentEnd || decoder.next()? != Event::StreamEnd {
        return Err(Error::InvalidInput);
    }
    Ok(value)
}
pub(crate) fn decode<T: DeserializeOwned>(input: &str) -> Result<T> {
    decode_diagnosed(input).map_err(|issue| issue.error)
}
pub(crate) fn decode_diagnosed<T: DeserializeOwned>(
    input: &str,
) -> std::result::Result<T, DecodeIssue> {
    if input.len() > MAX_BYTES {
        return Err(DecodeIssue {
            error: Error::ResourceLimit,
            path: String::new(),
            offset: 0,
        });
    }
    let mut decoder = decoder(input, None);
    parse(&mut decoder).map_err(|error| {
        decoder.capture();
        let (path, offset) = decoder.failure.take().expect("诊断已记录");
        DecodeIssue {
            error,
            path,
            offset,
        }
    })
}
/// 第二遍仅扫描事件，不构造配置树；按实际键/序列路径取得语义错误位置。
pub(crate) fn field_offset(input: &str, path: &str) -> Option<usize> {
    if input.len() > MAX_BYTES {
        return None;
    }
    let mut decoder = decoder(input, Some(path));
    let _result = parse::<serde::de::IgnoredAny>(&mut decoder);
    decoder.found
}
/// 已通过严格解析的映射，按实际键取得原文条目序号及键标记的标量偏移。
/// 键仅用于内部匹配，不进入公开字段路径、错误参数或日志。
pub(crate) fn mapping_key_position(input: &str, path: &str, key: &str) -> Option<(usize, usize)> {
    if input.len() > MAX_BYTES || path.len() >= 1024 || key.len() > 1024 {
        return None;
    }
    let mut decoder = decoder(input, None);
    decoder.target_mapping = Some((path.to_owned(), key.to_owned()));
    parse::<serde::de::IgnoredAny>(&mut decoder).ok()?;
    decoder.found_mapping
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
        self.decoder.peek()?;
        let previous = self.decoder.enter(&format!("[{}]", self.count - 1));
        self.decoder.locate();
        let result = seed.deserialize(&mut *self.decoder).map(Some);
        if result.is_err() {
            self.decoder.capture();
        }
        self.decoder.path.truncate(previous);
        result
    }
}
struct Mapping<'a, 'b> {
    decoder: &'a mut Decoder<'b>,
    keys: HashSet<String>,
    pending_value: bool,
    key: String,
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
        if self
            .decoder
            .target_mapping
            .as_ref()
            .is_some_and(|(path, target)| path == &self.decoder.path && target == &key)
        {
            self.decoder.found_mapping = Some((self.keys.len(), self.decoder.position));
        }
        self.key = if self.decoder.path == "dns.hosts" {
            format!("[{}]", self.keys.len())
        } else if key.len() <= 64
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            key.clone()
        } else {
            "[field]".into()
        };
        if !self.keys.insert(key.clone()) {
            let previous = self.decoder.enter(&self.key);
            self.decoder.capture();
            self.decoder.path.truncate(previous);
            return Err(Error::ConfigDuplicateField(FieldName::new(&key)));
        }
        self.pending_value = true;
        if key == "payload_yaml" {
            self.decoder.scalar_limit = 8 * 1024 * 1024;
        }
        let previous = self.decoder.enter(&self.key);
        self.decoder.locate();
        let result = seed
            .deserialize(de::value::StringDeserializer::<Error>::new(key))
            .map(Some);
        if result.is_err() {
            self.decoder.capture();
        }
        self.decoder.path.truncate(previous);
        result
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        if !self.pending_value {
            return Err(Error::InvalidInput);
        }
        self.pending_value = false;
        let previous = self.decoder.enter(&self.key);
        let result = seed.deserialize(&mut *self.decoder);
        if result.is_err() {
            self.decoder.capture();
        }
        self.decoder.path.truncate(previous);
        result
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
                    key: String::new(),
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
/// 逐条直接写入最终 YAML 缓冲区，避免逐行格式化与中间文本副本。
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
    fn indent(&mut self, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(Error::ResourceLimit);
        }
        // 最大深度固定；从常量切片取缩进，不为每条规则分配缩进字符串。
        const SPACES: &str = "                                                                ";
        self.append(&SPACES[..depth * 2])
    }
    fn scalar<T: serde::Serialize>(&mut self, value: &T) -> Result<()> {
        self.append(&serde_json::to_string(value).map_err(|_| Error::Internal)?)
    }
    fn nested(value: &Value) -> bool {
        matches!(value, Value::Object(v) if !v.is_empty())
            || matches!(value, Value::Array(v) if !v.is_empty())
    }
    fn item(&mut self, value: &Value, depth: usize) -> Result<()> {
        if Self::nested(value) {
            self.append("\n")?;
            self.value(value, depth + 1)
        } else {
            self.append(" ")?;
            self.scalar(value)?;
            self.append("\n")
        }
    }
    fn value(&mut self, value: &Value, depth: usize) -> Result<()> {
        self.indent(depth)?;
        match value {
            Value::Object(values) if !values.is_empty() => {
                for (index, (key, value)) in values.iter().enumerate() {
                    if index != 0 {
                        self.indent(depth)?;
                    }
                    self.scalar(key)?;
                    self.append(":")?;
                    self.item(value, depth)?;
                }
            }
            Value::Array(values) if !values.is_empty() => {
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        self.indent(depth)?;
                    }
                    self.append("-")?;
                    self.item(value, depth)?;
                }
            }
            _ => {
                self.scalar(value)?;
                self.append("\n")?;
            }
        }
        Ok(())
    }
    fn key(&mut self, name: &str, depth: usize) -> Result<()> {
        self.indent(depth)?;
        self.scalar(&name)?;
        self.append(":")
    }
    pub(super) fn field<T: serde::Serialize>(
        &mut self,
        name: &str,
        value: &T,
        depth: usize,
    ) -> Result<()> {
        let value = serde_json::to_value(value).map_err(|_| Error::Internal)?;
        self.key(name, depth)?;
        if !value.is_object() && !value.is_array() {
            self.append(" ")?;
            self.scalar(&value)?;
            self.append("\n")
        } else {
            self.append("\n")?;
            self.value(&value, depth + 1)
        }
    }
    pub(super) fn sequence<T: serde::Serialize>(
        &mut self,
        name: &str,
        values: &[T],
        depth: usize,
    ) -> Result<()> {
        self.key(name, depth)?;
        if values.is_empty() {
            return self.append(" []\n");
        }
        self.append("\n")?;
        for value in values {
            self.indent(depth + 1)?;
            self.append("-\n")?;
            let value = serde_json::to_value(value).map_err(|_| Error::Internal)?;
            self.value(&value, depth + 2)?;
        }
        Ok(())
    }
    pub(super) fn mapping(&mut self, name: &str) -> Result<()> {
        self.key(name, 0)?;
        self.append("\n")
    }
    pub(super) fn finish(self) -> String {
        self.output
    }
}

#[cfg(test)]
#[path = "../../tests/test_routing_yaml.rs"]
mod test_routing_yaml;
