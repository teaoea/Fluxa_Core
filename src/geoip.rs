//! 显式配置来源的有界 MMDB 下载与离线查询；无默认地址，未配置是 Disabled 状态。
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, ErrorCode, ErrorInfo, Result},
};
use maxminddb::{Reader, path};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    net::IpAddr,
    path::Path,
    sync::{Arc, Mutex},
};
const MAX_DATABASE: usize = crate::http::MAX_DATABASE_BODY;
const MMDB_MARKER: &[u8] = b"\xab\xcd\xefMaxMind.com";
static DATABASE_BUDGET: ByteBudget = ByteBudget::new(512 * 1024 * 1024);
/// 配置缺失/null 表示关闭；显式空串是无效输入，没有默认来源。
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub geoip_url: Option<String>,
}
impl Config {
    /// 仅解析主配置提取后的 GeoIP 片段；未知字段和重复键严格拒绝。
    pub fn from_yaml(input: &str) -> Result<Self> {
        if input.len() > 4096 {
            return Err(Error::GeoipResourceLimit);
        }
        let value: Self = crate::routing::runtime::yaml::decode(input)?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        let Some(value) = &self.geoip_url else {
            return Ok(());
        };
        if value.is_empty() || value.len() > 1024 || value.bytes().any(|b| b <= 32 || b == 127) {
            return Err(Error::GeoipInvalidUrl);
        }
        let url = url::Url::parse(value).map_err(|_| Error::GeoipInvalidUrl)?;
        if url.scheme() != "https" {
            return Err(Error::GeoipUrlScheme);
        }
        if url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.port() == Some(0)
        {
            return Err(Error::GeoipInvalidUrl);
        }
        crate::network::normalize_host(url.host_str().ok_or(Error::GeoipInvalidUrl)?)
            .map_err(|_| Error::GeoipInvalidUrl)?;
        Ok(())
    }
}
/// 可选功能未配置是 WARN 状态；仍使用中央代码与空参数 Schema。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Warning {
    pub level: &'static str,
    #[serde(flatten)]
    pub error: ErrorInfo,
}
impl Warning {
    pub fn disabled() -> Self {
        crate::error::diagnostics::record_code(
            ErrorCode::GeoipNotConfigured,
            crate::error::diagnostics::DiagnosticSite::GeoipDownload,
        );
        Self {
            level: "warn",
            error: ErrorInfo::new(ErrorCode::GeoipNotConfigured),
        }
    }
}
/// 成功和关闭状态有机器代码，不通过 Err 或 error_info 返回。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Status {
    pub state: &'static str,
    pub level: &'static str,
    pub code: ErrorCode,
    pub params: crate::error::ErrorParams,
}
impl Status {
    pub fn disabled() -> Self {
        Self {
            state: "disabled",
            level: "warn",
            code: ErrorCode::GeoipNotConfigured,
            params: Default::default(),
        }
    }
    pub fn ready() -> Self {
        Self {
            state: "ready",
            level: "info",
            code: ErrorCode::GeoipReady,
            params: Default::default(),
        }
    }
    pub fn no_match() -> Self {
        Self {
            state: "no-match",
            level: "info",
            code: ErrorCode::GeoipNoMatch,
            params: Default::default(),
        }
    }
}
pub struct Lookup {
    pub status: Status,
    pub location: Option<Location>,
}
/// 来源不可变，配置变化创建新服务；旧下载不更新新服务，失败显式返回。
#[derive(Default)]
pub struct Service {
    config: Config,
    database: Mutex<Option<Arc<Database>>>,
}
impl Service {
    pub fn new(config: Config) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            database: Mutex::new(None),
        })
    }
    /// 仅内核事务比较来源身份，不能用来自其他 URL 的旧库掩盖初始化失败。
    pub(crate) fn source_url(&self) -> Option<&str> {
        self.config.geoip_url.as_deref()
    }
    pub fn is_disabled(&self) -> bool {
        self.config.geoip_url.is_none()
    }
    pub fn warning(&self) -> Option<Warning> {
        self.is_disabled().then(Warning::disabled)
    }
    /// 无配置直接 Disabled；成功才原子替换库，失败保留库但仍返回 Err。
    pub async fn refresh(&self, client: &crate::https::HttpsClient) -> Result<Status> {
        let Some(url) = &self.config.geoip_url else {
            return Ok(Status::disabled());
        };
        let mut lease = DATABASE_BUDGET
            .reserve(MAX_DATABASE)
            .map_err(resource_error)?;
        let response = client.download_database(url).await.map_err(access_error)?;
        if response.headers.iter().any(|h| {
            h.name.eq_ignore_ascii_case("content-encoding")
                && !h.value.eq_ignore_ascii_case("identity")
        }) {
            return Err(Error::GeoipInvalidDatabase);
        }
        lease.shrink_to(response.body.capacity());
        let replacement = Database::from_bytes(response.body, lease)?;
        *self
            .database
            .lock()
            .map_err(|_| Error::GeoipInitializationFailed)? = Some(Arc::new(replacement));
        crate::error::diagnostics::record_code(
            ErrorCode::GeoipReady,
            crate::error::diagnostics::DiagnosticSite::GeoipDownload,
        );
        Ok(Status::ready())
    }
    /// 主配置候选必须绑定同一显式来源的 Ready 数据库，不能借用其他 URL 的旧库。
    pub(crate) fn database_for_url(&self, url: &str) -> Result<Arc<Database>> {
        if self.config.geoip_url.as_deref() != Some(url) {
            return Err(Error::GeoipInitializationFailed);
        }
        self.database()
    }
    pub fn database(&self) -> Result<Arc<Database>> {
        self.database
            .lock()
            .map_err(|_| Error::GeoipInitializationFailed)?
            .as_ref()
            .map(Arc::clone)
            .ok_or(Error::GeoipInitializationFailed)
    }
    pub fn lookup(&self, address: &str, language: &str) -> Result<Lookup> {
        if self.is_disabled() {
            return Ok(Lookup {
                status: Status::disabled(),
                location: None,
            });
        }
        let location = self.database()?.lookup(address, language)?;
        Ok(Lookup {
            status: if location.is_some() {
                Status::ready()
            } else {
                Status::no_match()
            },
            location,
        })
    }
    pub fn close(&self) -> Result<()> {
        *self
            .database
            .lock()
            .map_err(|_| Error::GeoipInitializationFailed)? = None;
        Ok(())
    }
}
/// 转换前在有界诊断环保留底层分类；不会公开 URL、凭据或目标 IP。
fn access_error(error: Error) -> Error {
    crate::error::diagnostics::record(
        error,
        crate::error::diagnostics::DiagnosticSite::GeoipDownload,
    );
    match error {
        Error::NetworkTimeout => Error::GeoipDownloadTimeout,
        Error::ResourceLimit | Error::ResourceBudgetExceeded { .. } => Error::GeoipResourceLimit,
        Error::GeoipIntegrityFailed => error,
        _ => Error::GeoipDownloadFailed,
    }
}
/// 域名查询的 DNS 或运行状态故障保留原分类，公开为实际 GeoIP 查询执行失败。
pub(crate) fn query_error(error: Error) -> Error {
    crate::error::diagnostics::record(error, crate::error::diagnostics::DiagnosticSite::GeoipQuery);
    Error::GeoipQueryFailed
}
fn resource_error(error: Error) -> Error {
    crate::error::diagnostics::record(
        error,
        crate::error::diagnostics::DiagnosticSite::GeoipDownload,
    );
    Error::GeoipResourceLimit
}
#[derive(Clone)]
struct SharedBytes(Arc<Vec<u8>>);
impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
pub struct Database {
    reader: Reader<SharedBytes>,
    source: SharedBytes,
    _lease: ByteLease,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Location {
    pub ip: String,
    pub country_code: String,
    pub city: String,
    pub region: String,
}
impl Database {
    /// 配额在读文件之前保留；新库失败时调用者继续保留旧库。
    pub fn open(path: &Path) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Error::InvalidInput);
        }
        let mut file = File::open(path)?;
        let length =
            usize::try_from(file.metadata()?.len()).map_err(|_| Error::GeoipResourceLimit)?;
        if length == 0 {
            return Err(Error::GeoipInvalidDatabase);
        }
        if length > MAX_DATABASE {
            return Err(Error::GeoipResourceLimit);
        }
        let lease = DATABASE_BUDGET.reserve(length).map_err(resource_error)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Error::GeoipResourceLimit)?;
        bytes.resize(length, 0);
        file.read_exact(&mut bytes).map_err(|error| {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                Error::GeoipIntegrityFailed
            } else {
                Error::from(error)
            }
        })?;
        if bytes.len() != length {
            return Err(Error::InvalidInput);
        }
        Self::from_bytes(bytes, lease)
    }
    /// 格式识别与结构验证分别返回 2405/2406；不声称替代外部校验和或签名。
    fn from_bytes(bytes: Vec<u8>, lease: ByteLease) -> Result<Self> {
        if bytes.is_empty()
            || !bytes
                .windows(MMDB_MARKER.len())
                .rev()
                .take(128 * 1024)
                .any(|window| window == MMDB_MARKER)
        {
            return Err(Error::GeoipInvalidDatabase);
        }
        // 借用解析布局后校验 16 字节分隔区，不复制正文；临时元数据先释放，再转移所有权。
        let layout =
            Reader::from_source(bytes.as_slice()).map_err(|_| Error::GeoipInvalidDatabase)?;
        let start = usize::try_from(layout.metadata().node_count)
            .map_err(|_| Error::GeoipResourceLimit)?
            .checked_mul(usize::from(layout.metadata().record_size) / 4)
            .ok_or(Error::GeoipResourceLimit)?;
        let end = start.checked_add(16).ok_or(Error::GeoipResourceLimit)?;
        if !bytes
            .get(start..end)
            .is_some_and(|separator| separator.iter().all(|byte| *byte == 0))
        {
            return Err(Error::GeoipIntegrityFailed);
        }
        drop(layout);
        let source = SharedBytes(Arc::new(bytes));
        let reader =
            Reader::from_source(source.clone()).map_err(|_| Error::GeoipInvalidDatabase)?;
        verify_records(&reader)?;
        Ok(Self {
            reader,
            source,
            _lease: lease,
        })
    }
    /// 原子缓存写入只借用已校验正文，不复制整库；数据库预算覆盖真实共享缓冲区。
    pub(crate) fn bytes(&self) -> &[u8] {
        self.source.as_ref()
    }
    /// 规则查询只借用两字节国家代码；无记录返回 None，数据库错误不可变成未命中。
    pub(crate) fn country_code(&self, ip: IpAddr) -> Result<Option<&str>> {
        let record = self
            .reader
            .lookup(ip)
            .map_err(|_| Error::GeoipQueryFailed)?;
        if !record.has_data() {
            return Ok(None);
        }
        let code: Option<&str> = record
            .decode_path(&path!["country", "iso_code"])
            .map_err(|_| Error::GeoipQueryFailed)?;
        let Some(code) = code else {
            return Err(Error::GeoipQueryFailed);
        };
        if code.len() != 2 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(Error::GeoipQueryFailed);
        }
        Ok(Some(code))
    }
    /// 只解码所需短字符串，避免把完整记录或坐标复制到宿主。
    pub fn lookup(&self, address: &str, language: &str) -> Result<Option<Location>> {
        if address.len() > 64 || language.len() > 16 {
            return Err(Error::InvalidInput);
        }
        let ip: IpAddr = address.parse().map_err(|_| Error::InvalidInput)?;
        let record = self
            .reader
            .lookup(ip)
            .map_err(|_| Error::GeoipQueryFailed)?;
        if !record.has_data() {
            return Ok(None);
        }
        let language = if language.starts_with("zh") {
            "zh-CN"
        } else {
            "en"
        };
        let field = |parts: &[maxminddb::PathElement<'_>]| -> Result<String> {
            let value: Option<&str> = record
                .decode_path(parts)
                .map_err(|_| Error::GeoipQueryFailed)?;
            Ok(value.unwrap_or("").chars().take(100).collect())
        };
        let country_code = field(&path!["country", "iso_code"])?;
        if country_code.len() != 2 || !country_code.bytes().all(|value| value.is_ascii_uppercase())
        {
            return Err(Error::GeoipQueryFailed);
        }
        let localized = |local, english| -> Result<String> {
            let value = field(local)?;
            if value.is_empty() {
                field(english)
            } else {
                Ok(value)
            }
        };
        let city_local = path!["city", "names", language];
        let city_english = path!["city", "names", "en"];
        let city = localized(&city_local, &city_english)?;
        let region_local = path!["subdivisions", 0, "names", language];
        let region_english = path!["subdivisions", 0, "names", "en"];
        let region = localized(&region_local, &region_english)?;
        Ok(Some(Location {
            ip: ip.to_string(),
            country_code,
            city,
            region,
        }))
    }
}
/// 免费 IP 回显服务仅获知请求来源；不发送节点地址、凭据或地理查询。
pub async fn current_address(client: &crate::https::HttpsClient, ipv6: bool) -> Result<String> {
    let endpoint = if ipv6 {
        "https://api6.ipify.org/"
    } else {
        "https://api.ipify.org/"
    };
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.execute_limited(endpoint, 64),
    )
    .await
    .map_err(|_| Error::Internal)??;
    if response.status != 200 {
        return Err(Error::Network);
    }
    let value = std::str::from_utf8(&response.body)
        .map_err(|_| Error::InvalidInput)?
        .trim();
    let address: IpAddr = value.parse().map_err(|_| Error::InvalidInput)?;
    if address.is_ipv6() != ipv6 || address.is_unspecified() || address.is_loopback() {
        return Err(Error::InvalidInput);
    }
    Ok(address.to_string())
}

/// 校验工作单位包括值和字符串字节；重复引用仍计数，防止小文件引发超量扫描。
const MAX_VERIFICATION_WORK: usize = 64 * 1024 * 1024;
#[derive(Default)]
struct VerifiedValue {
    work: usize,
}
impl<'de> Deserialize<'de> for VerifiedValue {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = VerifiedValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("有效 MMDB 值")
            }
            fn visit_bool<E: serde::de::Error>(
                self,
                _: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue { work: 1 })
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue { work: 1 })
            }
            // MMDB 扩展类型 10 支持 uint128，不应把合法附加字段当作损坏。
            fn visit_u128<E: serde::de::Error>(
                self,
                _: u128,
            ) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue { work: 1 })
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue { work: 1 })
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue { work: 1 })
            }
            fn visit_f32<E: serde::de::Error>(self, _: f32) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue { work: 1 })
            }
            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                value: &'de str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue {
                    work: value.len().saturating_add(1),
                })
            }
            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue {
                    work: value.len().saturating_add(1),
                })
            }
            fn visit_borrowed_bytes<E: serde::de::Error>(
                self,
                value: &'de [u8],
            ) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue {
                    work: value.len().saturating_add(1),
                })
            }
            fn visit_bytes<E: serde::de::Error>(
                self,
                value: &[u8],
            ) -> std::result::Result<Self::Value, E> {
                Ok(VerifiedValue {
                    work: value.len().saturating_add(1),
                })
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut work: usize = 1;
                while let Some(value) = sequence.next_element::<VerifiedValue>()? {
                    work = work
                        .checked_add(value.work)
                        .ok_or_else(|| serde::de::Error::custom("MMDB 校验工作量超限"))?;
                }
                Ok(VerifiedValue { work })
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut work: usize = 1;
                while let Some((key, value)) = map.next_entry::<VerifiedValue, VerifiedValue>()? {
                    work = work
                        .checked_add(key.work)
                        .and_then(|v| v.checked_add(value.work))
                        .ok_or_else(|| serde::de::Error::custom("MMDB 校验工作量超限"))?;
                }
                Ok(VerifiedValue { work })
            }
        }
        decoder.deserialize_any(Visitor)
    }
}
/// 上游迭代器保留有界 IP 路径，记录解码不建立 Vec/HashSet/JSON 树；每个引用均验证。
fn verify_records<S: AsRef<[u8]>>(reader: &Reader<S>) -> Result<()> {
    let maximum = usize::try_from(reader.metadata().node_count)
        .map_err(|_| Error::GeoipResourceLimit)?
        .checked_mul(3)
        .ok_or(Error::GeoipResourceLimit)?;
    let records = reader
        .networks(maxminddb::WithinOptions::default().include_networks_without_data())
        .map_err(|_| Error::GeoipIntegrityFailed)?;
    let mut work: usize = 0;
    for (index, record) in records.enumerate() {
        if index >= maximum || index >= MAX_VERIFICATION_WORK {
            return Err(Error::GeoipIntegrityFailed);
        }
        let record = record.map_err(|_| Error::GeoipIntegrityFailed)?;
        let value = record
            .decode::<VerifiedValue>()
            .map_err(|error| match error {
                maxminddb::MaxMindDbError::ResourceLimit { .. } => Error::GeoipResourceLimit,
                _ => Error::GeoipIntegrityFailed,
            })?;
        work = work
            .checked_add(value.map_or(1, |value| value.work))
            .filter(|v| *v <= MAX_VERIFICATION_WORK)
            .ok_or(Error::GeoipResourceLimit)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/test_geoip.rs"]
mod test_geoip;
