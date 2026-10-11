//! Vision 仅在完整内层 TLS 1.3 ServerHello 与应用记录边界上允许裸流切换。
use crate::error::{Error, Result};
use zeroize::Zeroizing;
const MAX: usize = 18432;
#[derive(Default)]
pub struct Boundary {
    header: [u8; 5],
    used: usize,
    left: usize,
    kind: u8,
    disabled: bool,
}
impl Boundary {
    /// 返回本片段中第一个完整应用记录的尾部位置；不按 TCP 分段猜测边界。
    pub fn application_end(&mut self, data: &[u8]) -> Option<usize> {
        if self.disabled {
            return None;
        }
        let mut i = 0;
        let mut found = None;
        while i < data.len() {
            if self.left > 0 {
                let n = self.left.min(data.len() - i);
                self.left -= n;
                i += n;
                if self.left == 0 && self.kind == 23 && found.is_none() {
                    found = Some(i);
                }
                continue;
            }
            if self.used == 5 {
                self.used = 0;
            }
            let n = (5 - self.used).min(data.len() - i);
            self.header[self.used..self.used + n].copy_from_slice(&data[i..i + n]);
            self.used += n;
            i += n;
            if self.used == 5 {
                self.kind = self.header[0];
                self.left = usize::from(u16::from_be_bytes([self.header[3], self.header[4]]));
                if !matches!(self.kind, 20..=23)
                    || self.header[1] != 3
                    || self.header[2] > 3
                    || self.left == 0
                    || self.left > MAX
                {
                    self.disabled = true;
                    return None;
                }
            }
        }
        found
    }
}
#[derive(Default)]
pub struct ServerHello {
    records: Zeroizing<Vec<u8>>,
    handshake: Zeroizing<Vec<u8>>,
    done: bool,
}
impl ServerHello {
    pub fn feed(&mut self, data: &[u8]) -> Result<bool> {
        if self.done {
            return Ok(false);
        }
        if self.records.len() + data.len() > MAX + 16389 {
            return Err(Error::ResourceLimit);
        }
        self.records.extend(data);
        loop {
            if self.records.len() < 5 {
                return Ok(false);
            }
            let n = usize::from(u16::from_be_bytes([self.records[3], self.records[4]]));
            if self.records[0] != 22 || self.records[1] != 3 || n == 0 || n > MAX {
                self.done = true;
                self.records.clear();
                self.handshake.clear();
                return Ok(false);
            }
            if self.records.len() < 5 + n {
                return Ok(false);
            }
            if self.handshake.len() + n > 65536 {
                return Err(Error::ResourceLimit);
            }
            self.handshake.extend(&self.records[5..5 + n]);
            self.records.drain(..5 + n);
            if self.handshake.len() < 4 {
                continue;
            }
            let len = (usize::from(self.handshake[1]) << 16)
                | (usize::from(self.handshake[2]) << 8)
                | usize::from(self.handshake[3]);
            if self.handshake[0] != 2 || len > 65532 {
                self.done = true;
                return Ok(false);
            }
            if self.handshake.len() < 4 + len {
                continue;
            }
            self.done = true;
            let b = &self.handshake[4..4 + len];
            let result = check(b);
            self.records.clear();
            self.handshake.clear();
            return result;
        }
    }
}
fn check(b: &[u8]) -> Result<bool> {
    if b.len() < 40 || b[..2] != [3, 3] {
        return Ok(false);
    }
    let sid = usize::from(b[34]);
    let tail = b.get(35 + sid..).ok_or(Error::InvalidInput)?;
    if tail.len() < 5 || !matches!(&tail[..2], [0x13, 1] | [0x13, 2] | [0x13, 3]) || tail[2] != 0 {
        return Ok(false);
    }
    let length = usize::from(u16::from_be_bytes([tail[3], tail[4]]));
    if length != tail.len() - 5 {
        return Err(Error::InvalidInput);
    }
    let mut extensions = &tail[5..];
    let mut tls13 = false;
    let mut seen = std::collections::BTreeSet::new();
    while !extensions.is_empty() {
        if extensions.len() < 4 {
            return Err(Error::InvalidInput);
        }
        let tag = u16::from_be_bytes([extensions[0], extensions[1]]);
        if !seen.insert(tag) {
            return Err(Error::InvalidInput);
        }
        let n = usize::from(u16::from_be_bytes([extensions[2], extensions[3]]));
        let data = extensions.get(4..4 + n).ok_or(Error::InvalidInput)?;
        if tag == 43 {
            tls13 = data == [3, 4];
        }
        extensions = &extensions[4 + n..];
    }
    Ok(tls13)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_record_boundary() {
        let mut b = Boundary::default();
        assert_eq!(b.application_end(&[22, 3, 3, 0, 2, 1]), None);
        assert_eq!(b.application_end(&[2, 23, 3, 3, 0, 3, 1]), None);
        assert_eq!(b.application_end(&[2, 3, 99]), Some(2));
    }
    #[test]
    fn non_tls_does_not_enable_switch() {
        assert_eq!(Boundary::default().application_end(b"GET / HTTP/1.1"), None);
        assert!(!ServerHello::default().feed(b"HTTP/1.1 200 OK").unwrap());
    }
}
