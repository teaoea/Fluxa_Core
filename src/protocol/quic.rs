//! QUIC v1/v2 报文形状识别及连接内关联；不解密、不依赖端口、不跨流猜测。
const V1: u32 = 1;
const V2: u32 = 0x6b3343cf;
const MAX_DATAGRAM: usize = 65507;
#[derive(Default)]
pub struct Flow {
    identified: bool,
}
impl Flow {
    /// 短头包只能在已观察到长头握手的同一连接内识别；状态随连接回收。
    pub fn observe(&mut self, bytes: &[u8]) -> bool {
        if self.identified {
            return true;
        }
        self.identified = long_datagram(bytes);
        self.identified
    }
}
fn integer(bytes: &[u8], cursor: &mut usize) -> Option<usize> {
    let first = *bytes.get(*cursor)?;
    let width = 1usize << (first >> 6);
    let end = cursor.checked_add(width)?;
    let input = bytes.get(*cursor..end)?;
    let mut value = usize::from(first & 0x3f);
    for byte in &input[1..] {
        value = value.checked_mul(256)?.checked_add(usize::from(*byte))?;
    }
    *cursor = end;
    Some(value)
}
fn long_packet(bytes: &[u8]) -> Option<usize> {
    let first = *bytes.first()?;
    if first & 0xc0 != 0xc0 {
        return None;
    }
    let version = u32::from_be_bytes(bytes.get(1..5)?.try_into().ok()?);
    if ![V1, V2].contains(&version) {
        return None;
    }
    let mut cursor = 5;
    for _ in 0..2 {
        let length = usize::from(*bytes.get(cursor)?);
        if length > 20 {
            return None;
        }
        cursor = cursor.checked_add(1 + length)?;
        bytes.get(..cursor)?;
    }
    let kind = (first >> 4) & 3;
    let initial = if version == V1 { kind == 0 } else { kind == 1 };
    let retry = if version == V1 { kind == 3 } else { kind == 0 };
    if retry {
        return (bytes.len() >= cursor + 16).then_some(bytes.len());
    }
    if initial {
        let token = integer(bytes, &mut cursor)?;
        cursor = cursor.checked_add(token)?;
        bytes.get(..cursor)?;
    }
    let length = integer(bytes, &mut cursor)?;
    // 包号受首部保护，不能用低两位推断；至少一字节包号及 16 字节认证标签。
    if length < 17 {
        return None;
    }
    let end = cursor.checked_add(length)?;
    bytes.get(..end)?;
    Some(end)
}
fn long_datagram(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > MAX_DATAGRAM {
        return false;
    }
    // 合并报文或尾部填充不能撤销已识别的首包，避免追加字节绕过拦截。
    long_packet(bytes).is_some()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn initial(version: u32) -> Vec<u8> {
        let mut bytes = vec![if version == V1 { 0xc0 } else { 0xd0 }];
        bytes.extend(version.to_be_bytes());
        bytes.extend([1, 1, 1, 2, 0, 17]);
        bytes.extend([0; 17]);
        bytes
    }
    #[test]
    fn versions_lengths_association_and_bounded_input() {
        for version in [V1, V2] {
            let packet = initial(version);
            for length in 0..packet.len() {
                assert!(!Flow::default().observe(&packet[..length]));
            }
            let mut flow = Flow::default();
            assert!(!flow.observe(&[0x40; 30]));
            assert!(flow.observe(&packet));
            assert!(flow.observe(&[0x40; 30]));
            assert!(!Flow::default().observe(&[0x40; 30]));
            let mut joined = packet.clone();
            joined.extend(&packet);
            assert!(Flow::default().observe(&joined));
            let mut padded = packet.clone();
            padded.extend([0; 1200]);
            assert!(Flow::default().observe(&padded));
            let mut malformed = packet;
            malformed[5] = 21;
            assert!(!Flow::default().observe(&malformed));
        }
        for length in 0..=9000 {
            let _ = Flow::default().observe(&vec![0xff; length]);
        }
    }
}
