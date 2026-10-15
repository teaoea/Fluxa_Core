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
#[path = "../../tests/test_protocol_quic.rs"]
mod test_protocol_quic;
