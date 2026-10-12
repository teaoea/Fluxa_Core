//! 只保存内部生成产物；二进制索引不含主机指针，提交仍采用同目录临时文件与原子替换。
//! SHA-256 检测损坏，不充当认证；规则报告加载后仍仅结构验证，不能凭校验和升级信任。
use super::runtime::{Document, Snapshot};
use crate::{
    budget::ByteBudget,
    error::{Error, Result},
};
use ring::digest::{Context, SHA256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
const MAGIC: &[u8; 8] = b"FLXARULE";
const VERSION: u32 = 1;
const WORD_BYTES: usize = 4;
const VERSION_OFFSET: usize = MAGIC.len();
const DOCUMENT_LENGTH_OFFSET: usize = VERSION_OFFSET + WORD_BYTES;
const INDEX_LENGTH_OFFSET: usize = DOCUMENT_LENGTH_OFFSET + WORD_BYTES;
const RESERVED_OFFSET: usize = INDEX_LENGTH_OFFSET + WORD_BYTES;
const DIGEST_OFFSET: usize = RESERVED_OFFSET + WORD_BYTES;
const DIGEST_BYTES: usize = 32;
const HEADER_BYTES: usize = DIGEST_OFFSET + DIGEST_BYTES;
const IO_BUFFER_BYTES: usize = 8192;
const BUDGET_BASE_BYTES: usize = 4096;
const WRITE_RULE_BYTES: usize = 1024;
const LOAD_BUFFER_FACTOR: usize = 2;
const MAX_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 32 * 1024 * 1024;
// 文档解析与写入缓冲区额外预算；快照自身仍持有运行时预算。
static MEMORY: ByteBudget = ByteBudget::new(256 * 1024 * 1024);
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
fn open(path: &Path) -> Result<File> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(Error::InvalidInput);
    }
    if meta.len() > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    Ok(file)
}
pub(super) fn load(path: &Path) -> Result<Snapshot> {
    let mut file = open(path)?;
    let total = file.metadata()?.len() as usize;
    if total < HEADER_BYTES {
        return Err(Error::InvalidInput);
    }
    let _temporary = MEMORY.reserve(
        total
            .checked_mul(LOAD_BUFFER_FACTOR)
            .and_then(|n| n.checked_add(BUDGET_BASE_BYTES))
            .ok_or(Error::ResourceLimit)?,
    )?;
    let mut header = [0u8; HEADER_BYTES];
    file.read_exact(&mut header)?;
    if &header[..MAGIC.len()] != MAGIC || size(&header, VERSION_OFFSET) != VERSION as usize {
        return Err(Error::Unsupported);
    }
    let document_size = size(&header, DOCUMENT_LENGTH_OFFSET);
    let index_size = size(&header, INDEX_LENGTH_OFFSET);
    if size(&header, RESERVED_OFFSET) != 0
        || document_size > MAX_DOCUMENT_BYTES
        || HEADER_BYTES
            .checked_add(document_size)
            .and_then(|n| n.checked_add(index_size))
            != Some(total)
    {
        return Err(Error::InvalidInput);
    }
    let mut document = vec![0; document_size];
    let mut index = vec![0; index_size];
    file.read_exact(&mut document)?;
    file.read_exact(&mut index)?;
    let mut tail = [0];
    if file.read(&mut tail)? != 0 {
        return Err(Error::InvalidInput);
    }
    if checksum(&document, &index).as_ref() != &header[DIGEST_OFFSET..HEADER_BYTES] {
        return Err(Error::InvalidInput);
    }
    let document: Document = serde_json::from_slice(&document).map_err(|_| Error::InvalidInput)?;
    Snapshot::from_flat_document(document, index)
}
/// 临时文件由 Store 创建；返回前刷新并逐字节验证，不重编译第二份索引。
pub(super) fn write(path: &Path, snapshot: &Snapshot) -> Result<()> {
    let doc = snapshot.document();
    let _temporary = MEMORY.reserve(
        doc.report
            .original
            .checked_mul(WRITE_RULE_BYTES)
            .and_then(|n| n.checked_add(BUDGET_BASE_BYTES))
            .ok_or(Error::ResourceLimit)?,
    )?;
    let document = serde_json::to_vec(doc).map_err(|_| Error::InvalidInput)?;
    let index = snapshot.flat_bytes()?;
    let total = HEADER_BYTES
        .checked_add(document.len())
        .and_then(|n| n.checked_add(index.len()))
        .ok_or(Error::ResourceLimit)?;
    if document.len() > MAX_DOCUMENT_BYTES || total as u64 > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
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
    let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;
    file.write_all(&header)?;
    file.write_all(&document)?;
    file.write_all(index)?;
    file.sync_all()?;
    drop(file);
    // 固定大小栈缓冲流式比较，失败时旧文件和旧快照继续使用。
    let mut file = open(path)?;
    if file.metadata()?.len() != total as u64 {
        return Err(Error::InvalidInput);
    }
    let mut buffer = [0u8; IO_BUFFER_BYTES];
    for expected in [&header[..], &document[..], index] {
        for chunk in expected.chunks(buffer.len()) {
            file.read_exact(&mut buffer[..chunk.len()])?;
            if &buffer[..chunk.len()] != chunk {
                return Err(Error::InvalidInput);
            }
        }
    }
    Ok(())
}
