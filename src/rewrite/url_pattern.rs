//! 有界 URL 路径匹配；加载时编译，匹配时不使用正则、递归或堆分配。
use crate::error::{Error, Result};

const MAX_URL_BYTES: usize = 4096;
/// 不可变 URL 模式。路径和查询字面量区分大小写，不解码百分号或归一化路径。
/// `*` 必须独占一个路径段，匹配恰好一个非空段；`**` 独占路径段，匹配零个或多个段。
/// 查询参数未声明时不限制；声明后按原始字面量匹配。此类型不执行复写动作。
pub struct UrlPattern {
    origin: Box<str>,
    query: Option<Box<str>>,
    path: Path,
}
enum Path {
    Exact(Box<str>),
    Segments(Vec<Segment>),
}
enum Segment {
    Literal(Box<str>),
    One,
    Many,
}
impl UrlPattern {
    /// 编译一个最大 4096 字节的 HTTP/HTTPS 模式；不接受凭据、片段、主机或查询通配符。
    /// 例如 `UrlPattern::compile("https://example.com/api/*")?`。
    pub fn compile(pattern: &str) -> Result<Self> {
        let parts = Parts::parse(pattern)?;
        // 仅编译时复用标准 URL 语法校验；实际比较保留原始字符串，避免隐式扩大范围。
        let parsed = url::Url::parse(pattern).map_err(|_| Error::InvalidInput)?;
        if parsed.host_str().is_none()
            || parts.origin.contains(['*', '@'])
            || parts.query.is_some_and(|query| query.contains('*'))
        {
            return Err(Error::InvalidInput);
        }
        let path = if !parts.path.contains('*') {
            Path::Exact(parts.path.into())
        } else {
            let mut segments = Vec::new();
            segments
                .try_reserve_exact(parts.path.split('/').count())
                .map_err(|_| Error::ResourceLimit)?;
            for segment in parts.path[1..].split('/') {
                segments.push(match segment {
                    "*" => Segment::One,
                    "**" => Segment::Many,
                    literal if !literal.contains('*') => Segment::Literal(literal.into()),
                    _ => return Err(Error::InvalidInput),
                });
            }
            Path::Segments(segments)
        };
        Ok(Self {
            origin: parts.origin.into(),
            query: parts.query.map(Into::into),
            path,
        })
    }
    /// 比较整个路径；不把 `/api/a` 当作 `/api/aa` 或 `/api/a/child` 的前缀。
    /// 例如 `/api/*` 匹配 `/api/a`，不匹配 `/api/aa/a`。非法或超长目标明确返回错误。
    pub fn matches(&self, target: &str) -> Result<bool> {
        let target = Parts::parse(target)?;
        if self.origin.as_ref() != target.origin
            || self
                .query
                .as_deref()
                .is_some_and(|query| Some(query) != target.query)
        {
            return Ok(false);
        }
        Ok(match &self.path {
            Path::Exact(path) => path.as_ref() == target.path,
            Path::Segments(pattern) => match_segments(pattern, target.path),
        })
    }
}
struct Parts<'a> {
    origin: &'a str,
    path: &'a str,
    query: Option<&'a str>,
}
impl<'a> Parts<'a> {
    fn parse(address: &'a str) -> Result<Self> {
        if address.len() > MAX_URL_BYTES {
            return Err(Error::ResourceLimit);
        }
        if address.chars().any(|c| c.is_control() || c.is_whitespace())
            || address.contains(['\\', '#'])
        {
            return Err(Error::InvalidInput);
        }
        let (scheme, remaining) = address.split_once("://").ok_or(Error::InvalidInput)?;
        if !matches!(scheme, "http" | "https") {
            return Err(Error::InvalidInput);
        }
        let boundary = remaining.find(['/', '?']).unwrap_or(remaining.len());
        let authority = &remaining[..boundary];
        if authority.is_empty() || authority.contains(['@', '*']) {
            return Err(Error::InvalidInput);
        }
        let origin_end = scheme.len() + 3 + boundary;
        let tail = &address[origin_end..];
        let (path, query) = tail
            .split_once('?')
            .map_or((tail, None), |(path, query)| (path, Some(query)));
        Ok(Self {
            origin: &address[..origin_end],
            // HTTP URL 未显式写路径时，请求路径是根路径；不折叠其他斜杠或点段。
            path: if path.is_empty() { "/" } else { path },
            query,
        })
    }
}
// 只保存最近一个 ** 的重试位置；所有重试都向前消费目标段，没有递归或正则回溯栈。
// 最坏比较量受双方 4096 字节上限约束；精确路径直接字符串比较。
fn match_segments(pattern: &[Segment], path: &str) -> bool {
    let mut remaining = path[1..].split('/').peekable();
    let mut index = 0usize;
    let mut retry: Option<(usize, std::iter::Peekable<std::str::Split<'_, char>>)> = None;
    loop {
        if matches!(pattern.get(index), Some(Segment::Many)) {
            index += 1;
            retry = Some((index, remaining.clone()));
            continue;
        }
        let Some(segment) = remaining.peek() else {
            return index == pattern.len();
        };
        let matched = match pattern.get(index) {
            Some(Segment::Literal(literal)) => literal.as_ref() == *segment,
            Some(Segment::One) => !segment.is_empty(),
            _ => false,
        };
        if matched {
            index += 1;
            remaining.next();
        } else if let Some((after_star, next)) = &mut retry {
            if next.next().is_none() {
                return false;
            }
            remaining = next.clone();
            index = *after_star;
        } else {
            return false;
        }
    }
}

#[cfg(test)]
#[path = "../../tests/test_rewrite_url_pattern.rs"]
mod test_rewrite_url_pattern;
