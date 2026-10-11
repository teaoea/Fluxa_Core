//! SQLite 只保存生成结果，匹配仍使用不可变索引；用户 YAML 与订阅原文不进入提交路径。
use super::runtime::{Document, EffectiveRule, MAX_RULES, Snapshot};
use crate::{
    budget::ByteBudget,
    error::{Error, Result},
};
use rusqlite::{Connection, OpenFlags, limits::Limit, params};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::Path};
const MAX_BYTES: u64 = 32 * 1024 * 1024;
// 持久化额外内存全局有界；SQLite 页缓存限制为 2 MiB，不与匹配缓存混用。
static MEMORY: ByteBudget = ByteBudget::new(128 * 1024 * 1024);
const META: &str = "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)";
const RULES: &str =
    "CREATE TABLE effective_rules (ordinal INTEGER PRIMARY KEY, record TEXT NOT NULL)";
fn sql<T>(value: rusqlite::Result<T>) -> Result<T> {
    value.map_err(|error| match error.sqlite_error_code() {
        Some(
            rusqlite::ErrorCode::OutOfMemory
            | rusqlite::ErrorCode::DiskFull
            | rusqlite::ErrorCode::TooBig,
        ) => Error::ResourceLimit,
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            Error::InvalidState
        }
        _ => Error::InvalidInput,
    })
}
fn encode<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| Error::InvalidInput)
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| Error::InvalidInput)
}
fn connection(path: &Path, write: bool) -> Result<Connection> {
    let flags = if write {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let db = sql(Connection::open_with_flags(
        path,
        flags | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    ))?;
    sql(db.set_limit(Limit::SQLITE_LIMIT_LENGTH, 32 * 1024 * 1024))?;
    sql(db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 4096))?;
    sql(db.set_limit(Limit::SQLITE_LIMIT_COLUMN, 16))?;
    sql(
        db.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA cache_size=-2048; PRAGMA mmap_size=0;")
    )?;
    Ok(db)
}
fn checked_connection(path: &Path) -> Result<Connection> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(Error::InvalidInput);
    }
    if meta.len() > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let db = connection(path, false)?;
    let id: u32 = sql(db.query_row("PRAGMA application_id", [], |r| r.get(0)))?;
    let version: u32 = sql(db.query_row("PRAGMA user_version", [], |r| r.get(0)))?;
    if id != 0x464c5841 || version != 1 {
        return Err(Error::Unsupported);
    }
    // 拒绝视图、触发器、虚拟表和未知表，避免不可信数据库执行任意查询表达式。
    let mut statement = sql(
        db.prepare("SELECT sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name")
    )?;
    let schema: Vec<String> = sql(statement.query_map([], |r| r.get(0)))?
        .collect::<rusqlite::Result<_>>()
        .map_err(|_| Error::InvalidInput)?;
    if schema != [RULES, META] {
        return Err(Error::InvalidInput);
    }
    drop(statement);
    Ok(db)
}
fn metadata<T: DeserializeOwned>(db: &Connection, key: &str) -> Result<T> {
    let value: String = sql(
        db.query_row("SELECT value FROM metadata WHERE key=?1", [key], |r| {
            r.get(0)
        }),
    )?;
    decode(&value)
}
fn document(db: &Connection) -> Result<Document> {
    let count: u32 = sql(db.query_row("SELECT count(*) FROM effective_rules", [], |r| r.get(0)))?;
    if count as usize > MAX_RULES {
        return Err(Error::ResourceLimit);
    }
    let mut statement =
        sql(db.prepare("SELECT ordinal, record FROM effective_rules ORDER BY ordinal"))?;
    let mut rows = sql(statement.query([]))?;
    let mut rules = Vec::with_capacity(count as usize);
    let mut origins = 0usize;
    while let Some(row) = sql(rows.next())? {
        let ordinal: u32 = sql(row.get(0))?;
        if ordinal as usize != rules.len() {
            return Err(Error::InvalidInput);
        }
        let rule = decode::<EffectiveRule>(&sql(row.get::<_, String>(1))?)?;
        origins = origins
            .checked_add(1 + rule.additional_origins.len())
            .ok_or(Error::ResourceLimit)?;
        if origins > MAX_RULES {
            return Err(Error::ResourceLimit);
        }
        rules.push(rule);
    }
    let count: u32 = sql(db.query_row("SELECT count(*) FROM metadata", [], |r| r.get(0)))?;
    if count != 6 || metadata::<String>(db, "storage")? != "sqlite-v1" {
        return Err(Error::InvalidInput);
    }
    Ok(Document {
        format_version: metadata(db, "format_version")?,
        version: metadata(db, "version")?,
        settings: metadata(db, "settings")?,
        sources: metadata(db, "sources")?,
        report: metadata(db, "report")?,
        rules,
    })
}
pub(super) fn load(path: &Path) -> Result<Snapshot> {
    let size = fs::metadata(path)?.len();
    if size > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let _temporary = MEMORY.reserve(size as usize * 4 + 4096)?;
    Snapshot::from_document(document(&checked_connection(path)?)?)
}
/// 写入临时数据库并完成验证，返回前关闭全部连接，不保留 WAL 或 journal 副文件。
pub(super) fn write(path: &Path, snapshot: &Snapshot) -> Result<()> {
    let _temporary = MEMORY.reserve(snapshot.document().report.original * 1024 + 4096)?;
    let mut db = connection(path, true)?;
    sql(db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA page_size=4096; PRAGMA max_page_count=8192; PRAGMA application_id=1179408449; PRAGMA user_version=1;"))?;
    let transaction = sql(db.transaction())?;
    sql(transaction.execute_batch(META))?;
    sql(transaction.execute_batch(RULES))?;
    let doc = snapshot.document();
    for (key, value) in [
        ("format_version", encode(&doc.format_version)?),
        ("version", encode(&doc.version)?),
        ("settings", encode(&doc.settings)?),
        ("sources", encode(&doc.sources)?),
        ("report", encode(&doc.report)?),
        ("storage", encode(&"sqlite-v1")?),
    ] {
        sql(transaction.execute("INSERT INTO metadata VALUES (?1,?2)", params![key, value]))?;
    }
    {
        let mut insert = sql(transaction.prepare("INSERT INTO effective_rules VALUES (?1,?2)"))?;
        for (ordinal, rule) in doc.rules.iter().enumerate() {
            sql(insert.execute(params![ordinal as u32, encode(rule)?]))?;
        }
    }
    sql(transaction.commit())?;
    drop(db);
    let db = checked_connection(path)?;
    // 流式逐行比较，不复制整份规则或重新编译第二份索引。
    if metadata::<u32>(&db, "format_version")? != doc.format_version
        || metadata::<u64>(&db, "version")? != doc.version
        || metadata::<super::Config>(&db, "settings")? != doc.settings
        || metadata::<Vec<super::runtime::SourceIdentity>>(&db, "sources")? != doc.sources
        || metadata::<super::runtime::Report>(&db, "report")? != doc.report
    {
        return Err(Error::InvalidInput);
    }
    let mut statement =
        sql(db.prepare("SELECT ordinal,record FROM effective_rules ORDER BY ordinal"))?;
    let mut rows = sql(statement.query([]))?;
    for (ordinal, expected) in doc.rules.iter().enumerate() {
        let row = sql(rows.next())?.ok_or(Error::InvalidInput)?;
        if sql(row.get::<_, u32>(0))? as usize != ordinal
            || decode::<EffectiveRule>(&sql(row.get::<_, String>(1))?)? != *expected
        {
            return Err(Error::InvalidInput);
        }
    }
    if sql(rows.next())?.is_some() {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
