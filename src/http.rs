//! 请求和响应共用有上限的 HTTP/1 分帧工具，拒绝歧义分帧和跨主机转发。
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

pub const MAX_HEADER: usize = 32 * 1024;
pub const MAX_BODY: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub name: String,
    pub value: String,
}
pub struct Head {
    pub first: String,
    pub headers: Vec<Header>,
}
impl Head {
    pub fn value(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str())
    }
    pub fn parts(&self) -> Vec<&str> {
        self.first.split(' ').collect()
    }
}
pub async fn read_head<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> Result<Head> {
    let mut bytes = Vec::with_capacity(1024);
    loop {
        if bytes.len() >= MAX_HEADER {
            return Err(Error::ResourceLimit);
        }
        bytes.push(reader.read_u8().await?);
        if bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let mut headers = [httparse::EMPTY_HEADER; 128];
    // 两种消息头使用同一个严格头字段解析器。
    let first_end = bytes
        .windows(2)
        .position(|w| w == b"\r\n")
        .ok_or(Error::InvalidInput)?;
    let first = std::str::from_utf8(&bytes[..first_end])
        .map_err(|_| Error::InvalidInput)?
        .to_owned();
    let httparse::Status::Complete((_, parsed)) =
        httparse::parse_headers(&bytes[first_end + 2..], &mut headers)
            .map_err(|_| Error::InvalidInput)?
    else {
        return Err(Error::InvalidInput);
    };
    let mut output = Vec::with_capacity(parsed.len());
    for header in parsed {
        let value = std::str::from_utf8(header.value)
            .map_err(|_| Error::InvalidInput)?
            .trim()
            .to_owned();
        if value.bytes().any(|b| b < 32 && b != b'\t') {
            return Err(Error::InvalidInput);
        }
        output.push(Header {
            name: header.name.to_owned(),
            value,
        });
    }
    let head = Head {
        first,
        headers: output,
    };
    framing(&head)?;
    Ok(head)
}
fn framing(head: &Head) -> Result<Option<usize>> {
    let lengths: Vec<_> = head
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("content-length"))
        .collect();
    let transfers: Vec<_> = head
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("transfer-encoding"))
        .collect();
    if lengths.len() > 1 || transfers.len() > 1 || (!lengths.is_empty() && !transfers.is_empty()) {
        return Err(Error::InvalidInput);
    }
    if transfers
        .first()
        .is_some_and(|header| !header.value.eq_ignore_ascii_case("chunked"))
    {
        return Err(Error::Unsupported);
    }
    lengths
        .first()
        .map(|h| {
            if h.value.is_empty() || !h.value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::InvalidInput);
            }
            let n = h.value.parse::<usize>().map_err(|_| Error::InvalidInput)?;
            if n > MAX_BODY {
                Err(Error::ResourceLimit)
            } else {
                Ok(n)
            }
        })
        .transpose()
}
pub async fn read_body<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    head: &Head,
    until_eof: bool,
) -> Result<Vec<u8>> {
    if let Some(length) = framing(head)? {
        let mut body = vec![0; length];
        reader.read_exact(&mut body).await?;
        return Ok(body);
    }
    if head.value("transfer-encoding").is_some() {
        let mut body = Vec::new();
        loop {
            let line = read_line(reader, 1024).await?;
            let size_text = line.split(';').next().ok_or(Error::InvalidInput)?;
            if size_text.is_empty() || !size_text.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::InvalidInput);
            }
            let size = usize::from_str_radix(size_text, 16).map_err(|_| Error::InvalidInput)?;
            if size == 0 {
                if !read_line(reader, MAX_HEADER).await?.is_empty() {
                    return Err(Error::Unsupported);
                }
                return Ok(body);
            }
            let end = body
                .len()
                .checked_add(size)
                .filter(|n| *n <= MAX_BODY)
                .ok_or(Error::ResourceLimit)?;
            let start = body.len();
            body.resize(end, 0);
            reader.read_exact(&mut body[start..]).await?;
            if reader.read_u16().await? != 0x0d0a {
                return Err(Error::InvalidInput);
            }
        }
    }
    let mut body = Vec::new();
    if until_eof {
        reader
            .take((MAX_BODY + 1) as u64)
            .read_to_end(&mut body)
            .await?;
        if body.len() > MAX_BODY {
            return Err(Error::ResourceLimit);
        }
    }
    Ok(body)
}
async fn read_line<R: AsyncRead + Unpin>(reader: &mut BufReader<R>, max: usize) -> Result<String> {
    let mut line = Vec::new();
    while line.len() < max {
        line.push(reader.read_u8().await?);
        if line.ends_with(b"\r\n") {
            line.truncate(line.len() - 2);
            return String::from_utf8(line).map_err(|_| Error::InvalidInput);
        }
    }
    Err(Error::ResourceLimit)
}
pub fn forwarded_headers(headers: &[Header]) -> Vec<Header> {
    let tokens: Vec<_> = headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("connection"))
        .flat_map(|h| h.value.split(',').map(|t| t.trim().to_ascii_lowercase()))
        .collect();
    headers
        .iter()
        .filter(|h| {
            let name = h.name.to_ascii_lowercase();
            ![
                "connection",
                "proxy-connection",
                "proxy-authorization",
                "proxy-authenticate",
                "keep-alive",
                "upgrade",
                "trailer",
            ]
            .contains(&name.as_str())
                && !tokens.contains(&name)
        })
        .cloned()
        .collect()
}
pub async fn write_message<W: AsyncWrite + Unpin>(
    writer: &mut W,
    first: &str,
    headers: &[Header],
    body: &[u8],
    omit_length: bool,
) -> Result<()> {
    let mut output = format!("{first}\r\n");
    for header in forwarded_headers(headers) {
        let name = header.name.to_ascii_lowercase();
        if name == "transfer-encoding" || (name == "content-length" && !omit_length) {
            continue;
        }
        output.push_str(&format!("{}: {}\r\n", header.name, header.value));
    }
    output.push_str("Connection: close\r\n");
    if !omit_length {
        output.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    output.push_str("\r\n");
    writer.write_all(output.as_bytes()).await?;
    writer.write_all(body).await?;
    writer.flush().await?;
    Ok(())
}
