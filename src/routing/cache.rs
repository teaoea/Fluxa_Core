//! 只读加载宿主保存的二进制索引；导出仅返回内存编码，不创建或替换文件。
//! SHA-256 检测损坏，不充当认证；规则报告加载后仍仅结构验证，不能凭校验和升级信任。
use super::runtime::{Document, Snapshot};
use crate::error::{Error, Result};
use ring::digest::{Context, SHA256};
use std::path::Path;
const MAGIC: &[u8; 8] = b"FLXARULE";
// v6 为 QUIC 组合条件增加独立森林条件根；同时隔离进程/国家精确键，候选槽位数量不变；v5 及更早布局须重建。
// 旧布局/旧语义必须从原始来源重建；禁止把旧缓存当作当前规则接受。
const VERSION: u32 = 6;
const WORD_BYTES: usize = 4;
const VERSION_OFFSET: usize = MAGIC.len();
const DOCUMENT_LENGTH_OFFSET: usize = VERSION_OFFSET + WORD_BYTES;
const INDEX_LENGTH_OFFSET: usize = DOCUMENT_LENGTH_OFFSET + WORD_BYTES;
const RESERVED_OFFSET: usize = INDEX_LENGTH_OFFSET + WORD_BYTES;
const DIGEST_OFFSET: usize = RESERVED_OFFSET + WORD_BYTES;
const DIGEST_BYTES: usize = 32;
const HEADER_BYTES: usize = DIGEST_OFFSET + DIGEST_BYTES;
const BUDGET_BASE_BYTES: usize = 4096;
const WRITE_RULE_BYTES: usize = 1024;
const LOAD_BUFFER_FACTOR: usize = 2;
const MAX_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 32 * 1024 * 1024;
// 文档解析与写入缓冲区额外预算；快照自身仍持有运行时预算。
// 编译、平铺、缓存 I/O 与存量快照使用同一 512 MiB 准入池。
use super::runtime::MEMORY;
fn size(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(
        bytes[offset..offset + WORD_BYTES]
            .try_into()
            .expect("头部已校验长度"),
    ) as usize
}
fn checksum(document: &[u8], index: &[u8]) -> ring::digest::Digest {
    let mut hash = Context::new(&SHA256);
    hash.update(document);
    hash.update(index);
    hash.finish()
}
pub(super) fn load(path: &Path) -> Result<Snapshot> {
    let file = crate::storage::ReadOnlyFile::open(path)?;
    let total = file.size()?;
    if total as u64 > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    if total < HEADER_BYTES {
        return Err(Error::InvalidInput);
    }
    let _temporary = MEMORY.reserve(
        total
            .checked_mul(LOAD_BUFFER_FACTOR)
            .and_then(|n| n.checked_add(BUDGET_BASE_BYTES))
            .ok_or(Error::ResourceLimit)?,
    )?;
    let bytes = file.read_binary(total)?;
    let header = &bytes[..HEADER_BYTES];
    if &header[..MAGIC.len()] != MAGIC || size(header, VERSION_OFFSET) != VERSION as usize {
        return Err(Error::Unsupported);
    }
    let document_size = size(header, DOCUMENT_LENGTH_OFFSET);
    let index_size = size(header, INDEX_LENGTH_OFFSET);
    if size(header, RESERVED_OFFSET) != 0
        || document_size > MAX_DOCUMENT_BYTES
        || HEADER_BYTES
            .checked_add(document_size)
            .and_then(|n| n.checked_add(index_size))
            != Some(total)
    {
        return Err(Error::InvalidInput);
    }
    let document = &bytes[HEADER_BYTES..HEADER_BYTES + document_size];
    let index = &bytes[HEADER_BYTES + document_size..];
    if checksum(document, index).as_ref() != &header[DIGEST_OFFSET..HEADER_BYTES] {
        return Err(Error::InvalidInput);
    }
    let document: Document = serde_json::from_slice(document).map_err(|_| Error::InvalidInput)?;
    Snapshot::from_flat_document(document, index.to_vec())
}
/// 只返回内存编码；是否保存、保存路径及原子替换由宿主处理。
pub struct Encoded {
    bytes: Vec<u8>,
    _memory: crate::budget::ByteLease,
}
impl AsRef<[u8]> for Encoded {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
pub(super) fn encode(snapshot: &Snapshot) -> Result<Encoded> {
    let doc = snapshot.document();
    let temporary = MEMORY.reserve(
        doc.report
            .original
            .checked_mul(WRITE_RULE_BYTES)
            .and_then(|v| v.checked_add(BUDGET_BASE_BYTES))
            .ok_or(Error::ResourceLimit)?,
    )?;
    let document = serde_json::to_vec(doc).map_err(|_| Error::InvalidInput)?;
    let index = snapshot.flat_bytes()?;
    let total = HEADER_BYTES
        .checked_add(document.len())
        .and_then(|v| v.checked_add(index.len()))
        .ok_or(Error::ResourceLimit)?;
    if document.len() > MAX_DOCUMENT_BYTES || total as u64 > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let memory = MEMORY.reserve(total)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| Error::ResourceLimit)?;
    let mut header = [0u8; HEADER_BYTES];
    header[..MAGIC.len()].copy_from_slice(MAGIC);
    header[VERSION_OFFSET..DOCUMENT_LENGTH_OFFSET].copy_from_slice(&VERSION.to_le_bytes());
    header[DOCUMENT_LENGTH_OFFSET..INDEX_LENGTH_OFFSET].copy_from_slice(
        &u32::try_from(document.len())
            .map_err(|_| Error::ResourceLimit)?
            .to_le_bytes(),
    );
    header[INDEX_LENGTH_OFFSET..RESERVED_OFFSET].copy_from_slice(
        &u32::try_from(index.len())
            .map_err(|_| Error::ResourceLimit)?
            .to_le_bytes(),
    );
    header[DIGEST_OFFSET..HEADER_BYTES].copy_from_slice(checksum(&document, index).as_ref());
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&document);
    bytes.extend_from_slice(index);
    drop(temporary);
    Ok(Encoded {
        bytes,
        _memory: memory,
    })
}
