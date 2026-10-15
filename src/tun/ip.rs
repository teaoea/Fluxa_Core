//! 有界 IP 重组和扩展头规范化；完成校验后才交给 TCP/UDP，不产生逐包任务。
use super::{MAX_MTU, MIN_MTU, Packet, checksum, word};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
const MAX_ENTRIES: usize = 32;
const LIFETIME: Duration = Duration::from_secs(60);
const ENTRY_BYTES: usize = MAX_MTU * 2 + 512;
static MEMORY: ByteBudget = ByteBudget::new(2 * 1024 * 1024);
#[derive(Clone, Eq, PartialEq, Hash)]
struct Key {
    header: [u8; 40],
    identifier: u32,
    generation: Option<u64>,
}
struct Storage {
    bytes: Zeroizing<Vec<u8>>,
    present: Zeroizing<Vec<u8>>,
    _lease: ByteLease,
}
struct Entry {
    header: [u8; 40],
    prefix: [u8; 32],
    extension_count: usize,
    extension_bytes: usize,
    until: Instant,
    storage: Option<Storage>,
    end: Option<usize>,
    maximum: usize,
    received: usize,
    first: bool,
}
struct Description<'a> {
    header: [u8; 40],
    header_size: usize,
    data: &'a [u8],
    fragment: Option<(u32, usize, bool)>,
    extensions: bool,
    prefix: [u8; 32],
    extension_count: usize,
    extension_bytes: usize,
}
pub(super) enum Input<'a> {
    Borrowed(&'a [u8]),
    Owned(Packet),
}
impl Input<'_> {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Owned(packet) => &packet.bytes,
        }
    }
}
/// 由被登记的任务按值持有；即使任务尚未首次轮询或被 abort，析构也清理真实缓冲。
pub(super) struct Cleanup(pub std::sync::Arc<std::sync::Mutex<Reassembly>>);
impl Cleanup {
    pub fn expire(&self, now: Instant) -> Result<()> {
        self.0.lock().map_err(|_| Error::Internal)?.expire(now);
        Ok(())
    }
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        match self.0.lock() {
            Ok(mut table) => table.clear(),
            Err(poisoned) => {
                poisoned.into_inner().clear();
                crate::error::diagnostics::record(
                    Error::Internal,
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            }
        }
    }
}
#[derive(Default)]
pub(super) struct Reassembly {
    entries: HashMap<Key, Entry>,
    revision: u64,
}
impl Reassembly {
    pub fn revision(&mut self, revision: u64) {
        if self.revision != revision {
            self.clear();
            self.revision = revision;
        }
    }
    pub fn expire(&mut self, now: Instant) {
        self.entries.retain(|_, entry| now < entry.until);
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn submit<'a>(
        &mut self,
        bytes: &'a [u8],
        mtu: usize,
        generation: Option<u64>,
        now: Instant,
    ) -> Result<Option<Input<'a>>> {
        if !(MIN_MTU..=MAX_MTU).contains(&mtu) {
            return Err(Error::InvalidInput);
        }
        if bytes.len() > mtu {
            return Err(Error::ResourceLimit);
        }
        self.expire(now);
        let description = describe(bytes)?;
        let Some((identifier, offset, more)) = description.fragment else {
            return if description.extensions {
                Ok(Some(Input::Owned(canonical(
                    &description.header,
                    description.header_size,
                    description.data,
                )?)))
            } else {
                Ok(Some(Input::Borrowed(bytes)))
            };
        };
        // 原子 IPv6 分片必须独立处理，不能污染同标识的普通分片重组。
        if description.header_size == 40 && offset == 0 && !more {
            return Ok(Some(Input::Owned(finish(
                &description.header,
                40,
                description.data,
                description.extension_count,
                description.extension_bytes,
            )?)));
        }
        let end = offset
            .checked_add(description.data.len())
            .ok_or(Error::InvalidInput)?;
        let mut identity = [0; 40];
        identity[0] = description.header[0] & 0xf0;
        if description.header_size == 20 {
            identity[9] = description.header[9];
            identity[12..20].copy_from_slice(&description.header[12..20]);
        } else {
            identity[8..40].copy_from_slice(&description.header[8..40]);
        }
        let key = Key {
            header: identity,
            identifier,
            generation,
        };
        if end > MAX_MTU - description.header_size
            || description.data.is_empty()
            || (more && description.data.len() % 8 != 0)
        {
            self.poison(&key);
            return Err(if end > MAX_MTU - description.header_size {
                Error::ResourceLimit
            } else {
                Error::InvalidInput
            });
        }
        // RFC 7112：首片必须包含完整扩展链和完整传输头，不能在首片隐藏路由条件。
        if offset == 0
            && let Err(error) = first_header(&description)
        {
            self.poison(&key);
            return Err(error);
        }
        if !self.entries.contains_key(&key) {
            if self.entries.len() >= MAX_ENTRIES {
                return Err(Error::ResourceLimit);
            }
            let lease = MEMORY.reserve(ENTRY_BYTES)?;
            self.entries
                .try_reserve(1)
                .map_err(|_| Error::ResourceLimit)?;
            let allocate = || -> Result<Zeroizing<Vec<u8>>> {
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(MAX_MTU)
                    .map_err(|_| Error::ResourceLimit)?;
                bytes.resize(MAX_MTU, 0);
                Ok(Zeroizing::new(bytes))
            };
            self.entries.insert(
                key.clone(),
                Entry {
                    header: description.header,
                    prefix: description.prefix,
                    extension_count: description.extension_count,
                    extension_bytes: description.extension_bytes,
                    until: now + LIFETIME,
                    storage: Some(Storage {
                        bytes: allocate()?,
                        present: allocate()?,
                        _lease: lease,
                    }),
                    end: None,
                    maximum: 0,
                    received: 0,
                    first: false,
                },
            );
        }
        let entry = self.entries.get_mut(&key).ok_or(Error::Internal)?;
        if entry.header != description.header
            || entry.prefix != description.prefix
            || entry.extension_count != description.extension_count
            || entry.extension_bytes != description.extension_bytes
        {
            entry.storage = None;
            return Err(Error::InvalidInput);
        }
        let storage = entry.storage.as_mut().ok_or(Error::InvalidInput)?;
        // 重叠一律废弃整个数据报，保留到期墓碑，防止攻击者重新拼装同一标识。
        if storage.present[offset..end].iter().any(|value| *value != 0)
            || entry
                .end
                .is_some_and(|last| end > last || (!more && end != last))
            || (!more && (entry.maximum > end || entry.end.is_some()))
        {
            entry.storage = None;
            return Err(Error::InvalidInput);
        }
        storage.bytes[offset..end].copy_from_slice(description.data);
        storage.present[offset..end].fill(1);
        entry.received += description.data.len();
        entry.maximum = entry.maximum.max(end);
        entry.first |= offset == 0;
        if !more {
            entry.end = Some(end);
        }
        if !entry.first || entry.end != Some(entry.received) {
            return Ok(None);
        }
        let entry = self.entries.remove(&key).ok_or(Error::Internal)?;
        let storage = entry.storage.ok_or(Error::Internal)?;
        Ok(Some(Input::Owned(finish(
            &entry.header,
            description.header_size,
            &storage.bytes[..entry.received],
            entry.extension_count,
            entry.extension_bytes,
        )?)))
    }
    fn poison(&mut self, key: &Key) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.storage = None;
        }
    }
}
fn canonical(header: &[u8; 40], header_size: usize, data: &[u8]) -> Result<Packet> {
    let size = header_size
        .checked_add(data.len())
        .ok_or(Error::ResourceLimit)?;
    if size > MAX_MTU {
        return Err(Error::ResourceLimit);
    }
    Packet::build(size, |bytes| {
        bytes[..header_size].copy_from_slice(&header[..header_size]);
        bytes[header_size..].copy_from_slice(data);
        if header_size == 20 {
            bytes[2..4].copy_from_slice(&(size as u16).to_be_bytes());
            bytes[6..8].fill(0);
            bytes[10..12].fill(0);
            let sum = checksum(&bytes[..20], 0);
            bytes[10..12].copy_from_slice(&sum.to_be_bytes());
        } else {
            bytes[4..6].copy_from_slice(&(data.len() as u16).to_be_bytes());
        }
    })
}
fn finish(
    header: &[u8; 40],
    header_size: usize,
    data: &[u8],
    extension_count: usize,
    extension_bytes: usize,
) -> Result<Packet> {
    let packet = canonical(header, header_size, data)?;
    let description = describe(&packet.bytes)?;
    if description.fragment.is_some() {
        return Err(Error::InvalidInput);
    }
    if extension_count + description.extension_count > 8
        || extension_bytes + description.extension_bytes > 256
    {
        return Err(Error::ResourceLimit);
    }
    canonical(
        &description.header,
        description.header_size,
        description.data,
    )
}
fn first_header(description: &Description<'_>) -> Result<()> {
    let packet = canonical(
        &description.header,
        description.header_size,
        description.data,
    )?;
    let next = describe(&packet.bytes)?;
    if next.fragment.is_some() {
        return Err(Error::InvalidInput);
    }
    if description.extension_count + next.extension_count > 8
        || description.extension_bytes + next.extension_bytes > 256
    {
        return Err(Error::ResourceLimit);
    }
    let minimum = match next.header[if next.header_size == 20 { 9 } else { 6 }] {
        6 => 20,
        17 => 8,
        _ => return Err(Error::Unsupported),
    };
    if next.data.len() < minimum
        || (minimum == 20
            && (usize::from(next.data[12] >> 4) * 4 > next.data.len() || next.data[12] >> 4 < 5))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn describe(bytes: &[u8]) -> Result<Description<'_>> {
    let mut header = [0; 40];
    match bytes.first().ok_or(Error::InvalidInput)? >> 4 {
        4 => {
            if bytes.len() < 20
                || bytes[0] & 15 < 5
                || word(bytes, 2) as usize != bytes.len()
                || bytes[8] == 0
            {
                return Err(Error::InvalidInput);
            }
            let length = usize::from(bytes[0] & 15) * 4;
            if length > bytes.len()
                || checksum(&bytes[..length], 0) != 0
                || word(bytes, 6) & 0x8000 != 0
            {
                return Err(Error::InvalidInput);
            }
            if length != 20 {
                return Err(Error::Unsupported);
            }
            header[..20].copy_from_slice(&bytes[..20]);
            header[2..4].fill(0);
            header[6..8].fill(0);
            header[10..12].fill(0);
            let flags = word(bytes, 6);
            if flags & 0x4000 != 0 && flags & 0x3fff != 0 {
                return Err(Error::InvalidInput);
            }
            let fragment = (flags & 0x3fff != 0).then_some((
                u32::from(word(bytes, 4)),
                usize::from(flags & 0x1fff) * 8,
                flags & 0x2000 != 0,
            ));
            Ok(Description {
                header,
                header_size: 20,
                data: &bytes[20..],
                fragment,
                extensions: false,
                prefix: [0; 32],
                extension_count: 0,
                extension_bytes: 0,
            })
        }
        6 => {
            if bytes.len() < 40 || usize::from(word(bytes, 4)) + 40 != bytes.len() || bytes[7] == 0
            {
                return Err(Error::InvalidInput);
            }
            header.copy_from_slice(&bytes[..40]);
            header[4..6].fill(0);
            let mut next = bytes[6];
            let mut offset = 40;
            let mut count = 0;
            let mut hop = false;
            let mut routing = false;
            let mut destinations = 0;
            loop {
                if matches!(next, 6 | 17) {
                    header[6] = next;
                    return Ok(Description {
                        header,
                        header_size: 40,
                        data: &bytes[offset..],
                        fragment: None,
                        extensions: count != 0,
                        prefix: [0; 32],
                        extension_count: count,
                        extension_bytes: offset - 40,
                    });
                }
                count += 1;
                if count > 8 || offset - 40 > 256 {
                    return Err(Error::ResourceLimit);
                }
                if next == 44 {
                    if bytes.len() - offset < 8
                        || bytes[offset + 1] != 0
                        || word(bytes, offset + 2) & 6 != 0
                    {
                        return Err(Error::InvalidInput);
                    }
                    if bytes[offset] == 0 {
                        return Err(Error::InvalidInput);
                    }
                    let bits = word(bytes, offset + 2);
                    let identifier = u32::from_be_bytes(
                        bytes[offset + 4..offset + 8]
                            .try_into()
                            .map_err(|_| Error::InvalidInput)?,
                    );
                    header[6] = bytes[offset];
                    return Ok(Description {
                        header,
                        header_size: 40,
                        data: &bytes[offset + 8..],
                        fragment: Some((identifier, usize::from(bits & 0xfff8), bits & 1 != 0)),
                        extensions: true,
                        prefix: ring::digest::digest(&ring::digest::SHA256, &bytes[40..offset])
                            .as_ref()
                            .try_into()
                            .map_err(|_| Error::Internal)?,
                        extension_count: count,
                        extension_bytes: offset - 40 + 8,
                    });
                }
                if !matches!(next, 0 | 43 | 60) {
                    return Err(Error::Unsupported);
                }
                if bytes.len() - offset < 8 {
                    return Err(Error::InvalidInput);
                }
                let length = (usize::from(bytes[offset + 1]) + 1) * 8;
                if length > bytes.len() - offset || offset - 40 + length > 256 {
                    return Err(Error::InvalidInput);
                }
                match next {
                    0 => {
                        if hop || offset != 40 {
                            return Err(Error::InvalidInput);
                        }
                        hop = true;
                        options(&bytes[offset + 2..offset + length])?;
                    }
                    43 => {
                        if routing || bytes[offset + 2] == 0 || bytes[offset + 3] != 0 {
                            return Err(Error::Unsupported);
                        }
                        routing = true;
                    }
                    60 => {
                        destinations += 1;
                        if destinations > 2 {
                            return Err(Error::InvalidInput);
                        }
                        options(&bytes[offset + 2..offset + length])?;
                    }
                    _ => return Err(Error::Internal),
                }
                next = bytes[offset];
                offset += length;
            }
        }
        _ => Err(Error::Unsupported),
    }
}
fn options(mut bytes: &[u8]) -> Result<()> {
    while let Some(&kind) = bytes.first() {
        if kind == 0 {
            bytes = &bytes[1..];
            continue;
        }
        let size = usize::from(*bytes.get(1).ok_or(Error::InvalidInput)?) + 2;
        if size > bytes.len() {
            return Err(Error::InvalidInput);
        }
        if kind == 1 {
            if bytes[2..size].iter().any(|value| *value != 0) {
                return Err(Error::InvalidInput);
            }
        } else if kind == 5 {
            if size != 4 {
                return Err(Error::InvalidInput);
            }
        } else if kind >> 6 != 0 {
            return Err(Error::Unsupported);
        }
        bytes = &bytes[size..];
    }
    Ok(())
}
#[cfg(test)]
#[path = "../../tests/test_tun_ip.rs"]
mod test_tun_ip;
