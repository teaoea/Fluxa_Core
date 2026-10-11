//! 有界 Vision 分帧状态机；读取不依赖 TCP 分段或外层 TLS 记录边界。
use crate::error::{Error, Result};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;
const LIMIT: usize = 65535;
#[derive(Default)]
pub struct Decoder {
    input: Zeroizing<Vec<u8>>,
    first: bool,
    stopped: bool,
    pub direct: bool,
}
impl Decoder {
    pub fn new() -> Self {
        Self {
            first: true,
            ..Self::default()
        }
    }
    pub fn feed(&mut self, data: &[u8], uuid: &[u8; 16]) -> Result<Vec<u8>> {
        if self.stopped {
            return Ok(data.to_vec());
        }
        if self
            .input
            .len()
            .checked_add(data.len())
            .is_none_or(|n| n > 2 * LIMIT + 21)
        {
            return Err(Error::ResourceLimit);
        }
        self.input.extend(data);
        let mut out = Vec::new();
        let mut consumed = 0;
        if self.first {
            if self.input.len() < 16 {
                return Ok(out);
            }
            if self.input[..16] != *uuid {
                return Err(Error::InvalidInput);
            }
            consumed = 16;
            self.first = false;
        }
        loop {
            let remaining = &self.input[consumed..];
            if remaining.len() < 5 {
                break;
            }
            let command = remaining[0];
            if command > 2 {
                return Err(Error::InvalidInput);
            }
            let body = usize::from(u16::from_be_bytes([remaining[1], remaining[2]]));
            let padding = usize::from(u16::from_be_bytes([remaining[3], remaining[4]]));
            let total = 5 + body + padding;
            if remaining.len() < total {
                break;
            }
            out.extend(&remaining[5..5 + body]);
            consumed += total;
            if command != 0 {
                self.stopped = true;
                self.direct = command == 2;
                out.extend(&self.input[consumed..]);
                consumed = self.input.len();
                break;
            }
        }
        self.input.drain(..consumed);
        Ok(out)
    }
    pub fn finish(&self) -> Result<()> {
        if self.input.is_empty() {
            Ok(())
        } else {
            Err(Error::InvalidInput)
        }
    }
}
pub fn frame(uuid: Option<&[u8; 16]>, content: &[u8], command: u8) -> Result<Vec<u8>> {
    if content.len() > 1792 || command > 2 {
        return Err(Error::InvalidInput);
    }
    let mut random = [0];
    SystemRandom::new()
        .fill(&mut random)
        .map_err(|_| Error::Internal)?;
    let padding =
        usize::from(random[0]).min(2048 - content.len() - if uuid.is_some() { 21 } else { 5 });
    let mut out = Vec::with_capacity(21 + content.len() + padding);
    if let Some(uuid) = uuid {
        out.extend(uuid);
    }
    out.push(command);
    out.extend((content.len() as u16).to_be_bytes());
    out.extend((padding as u16).to_be_bytes());
    out.extend(content);
    out.resize(out.len() + padding, 0);
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_fragment_boundary_and_stop() {
        let uuid = [7; 16];
        let mut wire = frame(Some(&uuid), b"first", 0).unwrap();
        wire.extend(frame(None, b"last", 2).unwrap());
        for split in 0..=wire.len() {
            let mut d = Decoder::new();
            let mut out = d.feed(&wire[..split], &uuid).unwrap();
            out.extend(d.feed(&wire[split..], &uuid).unwrap());
            assert_eq!(out, b"firstlast");
            assert!(d.direct);
            d.finish().unwrap();
        }
    }
    #[test]
    fn bad_uuid_command_truncation_and_quota() {
        let uuid = [7; 16];
        let wire = frame(Some(&uuid), b"first", 0).unwrap();
        assert!(Decoder::new().feed(&wire, &[8; 16]).is_err());
        let mut d = Decoder::new();
        d.feed(&wire[..17], &uuid).unwrap();
        assert!(d.finish().is_err());
        assert!(
            Decoder::new()
                .feed(&vec![0; 2 * LIMIT + 22], &uuid)
                .is_err()
        );
        let mut bad = uuid.to_vec();
        bad.extend([3, 0, 0, 0, 0]);
        assert!(Decoder::new().feed(&bad, &uuid).is_err());
    }
}
