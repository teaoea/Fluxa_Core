//! 请求和响应共用有上限的 HTTP/1 分帧工具，拒绝歧义分帧和跨主机转发。
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

pub const MAX_HEADER: usize = 32 * 1024;
pub const MAX_BODY: usize = 8 * 1024 * 1024;
/// 仅供已预留数据库预算的下载入口；普通 HTTP 请求仍限 8 MiB。
pub(crate) const MAX_DATABASE_BODY: usize = 256 * 1024 * 1024;
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
    read_head_bounded(reader, MAX_BODY).await
}
/// 数据库专用头检查仍限制 32 KiB，只放宽已预留预算的正文长度。
pub(crate) async fn read_database_head<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
) -> Result<Head> {
    read_head_bounded(reader, MAX_DATABASE_BODY).await
}
async fn read_head_bounded<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    maximum: usize,
) -> Result<Head> {
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
    framing(&head, maximum)?;
    Ok(head)
}
fn framing(head: &Head, maximum: usize) -> Result<Option<usize>> {
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
            if n > maximum {
                Err(Error::ResourceLimit)
            } else {
                Ok(n)
            }
        })
        .transpose()
}
/// 所有宿主复用相同分帧，只允许降低读取上限。
pub async fn read_body<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    head: &Head,
    until_eof: bool,
) -> Result<Vec<u8>> {
    read_body_limited(reader, head, until_eof, MAX_BODY).await
}
pub async fn read_body_limited<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    head: &Head,
    until_eof: bool,
    maximum: usize,
) -> Result<Vec<u8>> {
    if maximum == 0 || maximum > MAX_BODY {
        return Err(Error::InvalidInput);
    }
    read_body_bounded(reader, head, until_eof, maximum).await
}
/// GeoIP 在调用前持有最大正文预算，复用分帧校验，不放宽普通响应入口。
pub(crate) async fn read_database_body<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    head: &Head,
) -> Result<Vec<u8>> {
    read_body_bounded(reader, head, true, MAX_DATABASE_BODY).await
}
async fn read_body_bounded<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    head: &Head,
    until_eof: bool,
    maximum: usize,
) -> Result<Vec<u8>> {
    if let Some(length) = framing(head, maximum)? {
        if length > maximum {
            return Err(Error::ResourceLimit);
        }
        let mut body = Vec::new();
        body.try_reserve_exact(length)
            .map_err(|_| Error::ResourceLimit)?;
        body.resize(length, 0);
        reader.read_exact(&mut body).await.map_err(|error| {
            if maximum == MAX_DATABASE_BODY && error.kind() == std::io::ErrorKind::UnexpectedEof {
                Error::GeoipIntegrityFailed
            } else {
                Error::from(error)
            }
        })?;
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
                .filter(|n| *n <= maximum)
                .ok_or(Error::ResourceLimit)?;
            let start = body.len();
            body.try_reserve_exact(end - start)
                .map_err(|_| Error::ResourceLimit)?;
            body.resize(end, 0);
            reader
                .read_exact(&mut body[start..])
                .await
                .map_err(|error| {
                    if maximum == MAX_DATABASE_BODY
                        && error.kind() == std::io::ErrorKind::UnexpectedEof
                    {
                        Error::GeoipIntegrityFailed
                    } else {
                        Error::from(error)
                    }
                })?;
            if reader.read_u16().await? != 0x0d0a {
                return Err(Error::InvalidInput);
            }
        }
    }
    let mut body = Vec::new();
    if until_eof {
        // 固定小块读入，先检查增长再分配，避免 read_to_end 的几何扩容超过上限。
        let mut block = [0; 8192];
        loop {
            let count = reader.read(&mut block).await?;
            if count == 0 {
                break;
            }
            body.len()
                .checked_add(count)
                .filter(|length| *length <= maximum)
                .ok_or(Error::ResourceLimit)?;
            body.try_reserve_exact(count)
                .map_err(|_| Error::ResourceLimit)?;
            body.extend_from_slice(&block[..count]);
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
