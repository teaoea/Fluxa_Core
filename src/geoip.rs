//! 只读 MMDB 离线查询；不联网，不映射外部内存，不信任数据库或输入地址。
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use maxminddb::{Reader, path};
use serde::Serialize;
use std::{fs::File, io::Read, net::IpAddr, path::Path};
const MAX_DATABASE: usize = 256 * 1024 * 1024;
static DATABASE_BUDGET: ByteBudget = ByteBudget::new(512 * 1024 * 1024);
pub struct Database {
    reader: Reader<Vec<u8>>,
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
        let length = usize::try_from(file.metadata()?.len()).map_err(|_| Error::ResourceLimit)?;
        if length == 0 || length > MAX_DATABASE {
            return Err(Error::ResourceLimit);
        }
        let lease = DATABASE_BUDGET.reserve(length)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Error::ResourceLimit)?;
        bytes.resize(length, 0);
        file.read_exact(&mut bytes)?;
        if bytes.len() != length {
            return Err(Error::InvalidInput);
        }
        let reader = Reader::from_source(bytes).map_err(|_| Error::InvalidInput)?;
        Ok(Self {
            reader,
            _lease: lease,
        })
    }
    /// 只解码所需短字符串，避免把完整记录或坐标复制到宿主。
    pub fn lookup(&self, address: &str, language: &str) -> Result<Option<Location>> {
        if address.len() > 64 || language.len() > 16 {
            return Err(Error::InvalidInput);
        }
        let ip: IpAddr = address.parse().map_err(|_| Error::InvalidInput)?;
        let record = self.reader.lookup(ip).map_err(|_| Error::InvalidInput)?;
        if !record.has_data() {
            return Ok(None);
        }
        let language = if language.starts_with("zh") {
            "zh-CN"
        } else {
            "en"
        };
        let field = |parts: &[maxminddb::PathElement<'_>]| -> Result<String> {
            let value: Option<&str> = record.decode_path(parts).map_err(|_| Error::InvalidInput)?;
            Ok(value.unwrap_or("").chars().take(100).collect())
        };
        let country_code = field(&path!["country", "iso_code"])?;
        if country_code.len() != 2 || !country_code.bytes().all(|value| value.is_ascii_uppercase())
        {
            return Err(Error::InvalidInput);
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_database_is_rejected() {
        let path = std::env::temp_dir().join(format!("fluxa-geoip-invalid-{}", std::process::id()));
        std::fs::write(&path, b"not an mmdb").unwrap();
        assert!(Database::open(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
