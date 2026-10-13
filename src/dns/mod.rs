//! DNS 客户端共用层：固定上游、加密传输、DNSSEC、正负 TTL 缓存及资源预算。
//! Hickory 负责标准报文/密码学验证；不读取系统 DNS 或 hosts，不隐式降级。
mod config;
pub mod fake_ip;
pub mod observation;
pub mod service;
mod wire;
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
pub use config::{Config, Server};
use hickory_resolver::{
    TokioResolver,
    net::runtime::TokioRuntimeProvider,
    net::{DnsError, NetError},
    proto::{
        dnssec::Proof,
        rr::{RData, RecordType},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    str::FromStr,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
const MAX_CACHE: usize = 128;
const MAX_RESULT_BYTES: usize = 16 * 1024;
static CACHE_BUDGET: ByteBudget = ByteBudget::new(16 * 1024 * 1024);
static CLIENTS: Semaphore = Semaphore::const_new(16);
static QUERIES: Semaphore = Semaphore::const_new(16);

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub name: String,
    pub query_type: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Record {
    pub name: String,
    pub record_type: String,
    pub ttl: u32,
    pub data: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    pub name: String,
    pub query_type: String,
    pub records: Vec<Record>,
    pub addresses: Vec<String>,
    pub ttl: u32,
    pub rcode: u16,
    pub validation: String,
    pub from_cache: bool,
    pub duration_ms: u64,
    #[serde(skip)]
    wire: Vec<u8>,
}
struct Cached {
    answer: Answer,
    expires: Instant,
    _lease: ByteLease,
}
/// 所有上游显式配置；实例最多 16 个，每次查询有总期限，缓存不提高 TTL。
pub struct Resolver {
    inner: Vec<TokioResolver>,
    config: Config,
    fake_ip: Option<fake_ip::Table>,
    cache: Mutex<BTreeMap<(String, String), Cached>>,
    observation: std::sync::Arc<Mutex<Option<observation::Observation>>>,
    _permit: tokio::sync::SemaphorePermit<'static>,
}
impl Resolver {
    pub fn new(config: Config) -> Result<Self> {
        Self::build(config, None, None)
    }
    pub fn with_tls_config(config: Config, tls: rustls::ClientConfig) -> Result<Self> {
        Self::build(config, Some(tls), None)
    }
    /// 显式私有 DNSSEC 根信任锚，仅影响此解析器；普通客户端使用内置根锚。
    pub fn with_trust_anchors(
        config: Config,
        anchors: hickory_resolver::proto::dnssec::TrustAnchors,
    ) -> Result<Self> {
        Self::build(config, None, Some(std::sync::Arc::new(anchors)))
    }
    fn build(
        config: Config,
        tls: Option<rustls::ClientConfig>,
        anchors: Option<std::sync::Arc<hickory_resolver::proto::dnssec::TrustAnchors>>,
    ) -> Result<Self> {
        config.validate()?;
        let permit = CLIENTS.try_acquire().map_err(|_| Error::ResourceLimit)?;
        let mut inner = Vec::with_capacity(config.servers.len());
        for server in &config.servers {
            let single = Config {
                servers: vec![server.clone()],
                timeout_ms: config.timeout_ms,
                dnssec: config.dnssec,
                fake_ip: None,
                service: None,
            };
            let (servers, mut options) = single.build()?;
            options.timeout =
                Duration::from_millis((config.timeout_ms / config.servers.len() as u64).max(50));
            let mut builder =
                TokioResolver::builder_with_config(servers, TokioRuntimeProvider::default())
                    .with_options(options);
            if let Some(mut tls) = tls.clone() {
                tls.alpn_protocols.clear();
                builder = builder.with_tls_config(tls);
            }
            if let Some(anchors) = &anchors {
                builder = builder.with_trust_anchor(anchors.clone());
            }
            inner.push(builder.build().map_err(|_| Error::InvalidInput)?);
        }
        let fake_ip = config
            .fake_ip
            .clone()
            .map(fake_ip::Table::new)
            .transpose()?;
        Ok(Self {
            inner,
            observation: observation::shared(&config)?,
            fake_ip,
            config,
            cache: Mutex::new(BTreeMap::new()),
            _permit: permit,
        })
    }
    /// 只读最近一次默认服务器解析；读取不会查询 DNS。缓存命中为 0 ms，合成映射不更新。
    pub fn last_query(&self) -> Result<Option<observation::Observation>> {
        Ok(self
            .observation
            .lock()
            .map_err(|_| Error::Internal)?
            .clone())
    }
    fn observe(&self, duration_ms: u64, from_cache: bool, successful: bool) -> Result<()> {
        *self.observation.lock().map_err(|_| Error::Internal)? = Some(observation::Observation {
            duration_ms,
            from_cache,
            successful,
        });
        Ok(())
    }
    pub async fn serve(self: std::sync::Arc<Self>) -> Result<service::Service> {
        let config = self.config.service.clone().ok_or(Error::InvalidState)?;
        service::Service::start(config, self).await
    }
    pub fn reverse_fake_ip(&self, address: &str) -> Result<Option<String>> {
        self.fake_ip
            .as_ref()
            .ok_or(Error::Unsupported)?
            .reverse(address)
    }
    pub fn reset_fake_ip(&self) -> Result<()> {
        self.fake_ip.as_ref().ok_or(Error::Unsupported)?.reset()
    }
    pub fn clear(&self) -> Result<()> {
        self.cache.lock().map_err(|_| Error::Internal)?.clear();
        Ok(())
    }
    /// TUN 原始包无宿主代次；显式绑定后合成池同进程不复用旧地址。
    pub(crate) fn bind_tun_fake_ip(&self) -> Result<()> {
        if let Some(table) = &self.fake_ip {
            table.bind_tun()?;
        }
        Ok(())
    }
    pub fn fake_ip_generation(&self) -> Option<u64> {
        self.fake_ip.as_ref().map(fake_ip::Table::generation)
    }
    pub fn is_fake_ip(&self, address: std::net::IpAddr) -> bool {
        self.fake_ip
            .as_ref()
            .is_some_and(|table| table.contains(address))
    }
    pub fn restore_fake_ip(&self, address: &str, generation: u64) -> Result<String> {
        let table = self.fake_ip.as_ref().ok_or(Error::Unsupported)?;
        if generation != table.generation() {
            return Err(Error::InvalidState);
        }
        table.reverse(address)?.ok_or(Error::InvalidState)
    }
    pub async fn resolve(&self, query: Query) -> Result<Answer> {
        self.resolve_inner(query, true).await
    }
    /// 真实解析复用上游、缓存和 DNSSEC；只跳过合成映射，不跳过验证。
    pub async fn resolve_real(&self, query: Query) -> Result<Answer> {
        self.resolve_inner(query, false).await
    }
    /// 只生成本地映射，排除项直接拒绝，不向上游查询。
    pub fn allocate_fake_ip(&self, query: Query) -> Result<Answer> {
        let (name, kind) = validated_query(query)?;
        if !matches!(kind, RecordType::A | RecordType::AAAA) {
            return Err(Error::Unsupported);
        }
        let _permit = QUERIES.try_acquire().map_err(|_| Error::ResourceLimit)?;
        self.synthetic_answer(&name, kind)?
            .ok_or(Error::FakeIpUnavailable)
    }
    fn synthetic_answer(&self, name: &str, kind: RecordType) -> Result<Option<Answer>> {
        if let Some(table) = &self.fake_ip
            && matches!(kind, RecordType::A | RecordType::AAAA)
            && let Some((address, from_cache)) = table.allocate(name, kind == RecordType::AAAA)?
        {
            return Ok(Some(Answer {
                wire: wire::synthetic(name, &address, kind)?,
                name: name.to_string(),
                query_type: kind.to_string(),
                records: vec![Record {
                    name: format!("{name}."),
                    record_type: kind.to_string(),
                    ttl: 60,
                    data: address.clone(),
                }],
                addresses: vec![address],
                ttl: 60,
                rcode: 0,
                validation: "synthetic".into(),
                from_cache,
                duration_ms: 0,
            }));
        }
        Ok(None)
    }
    async fn resolve_inner(&self, query: Query, synthetic: bool) -> Result<Answer> {
        let (name, kind) = validated_query(query)?;
        let _permit = QUERIES.try_acquire().map_err(|_| Error::ResourceLimit)?;
        if synthetic && let Some(answer) = self.synthetic_answer(&name, kind)? {
            return Ok(answer);
        }
        let key = (name.clone(), kind.to_string());
        let started = Instant::now();
        {
            let mut cache = self.cache.lock().map_err(|_| Error::Internal)?;
            cache.retain(|_, v| v.expires > started);
            if let Some(entry) = cache.get(&key) {
                let mut answer = entry.answer.clone();
                answer.from_cache = true;
                answer.duration_ms = 0;
                answer.ttl = entry.expires.saturating_duration_since(started).as_secs() as u32;
                for record in &mut answer.records {
                    record.ttl = record.ttl.min(answer.ttl);
                }
                self.observe(0, true, answer.rcode == 0 || answer.rcode == 3)?;
                return Ok(answer);
            }
        }
        let fqdn = if name == "." {
            name.clone()
        } else {
            format!("{name}.")
        };
        let attempt =
            Duration::from_millis((self.config.timeout_ms / self.inner.len() as u64).max(50));
        let lookup = tokio::time::timeout(Duration::from_millis(self.config.timeout_ms), async {
            let mut last = Err(NetError::Timeout);
            for (index, resolver) in self.inner.iter().enumerate() {
                let attempt_started = Instant::now();
                last = tokio::time::timeout(attempt, resolver.lookup(fqdn.clone(), kind))
                    .await
                    .unwrap_or(Err(NetError::Timeout));
                if index == 0 {
                    // 同步写入一条小统计，不创建后台任务；失败保留失败状态，不伪造零延迟成功。
                    self.observe(
                        attempt_started.elapsed().as_millis() as u64,
                        false,
                        last.is_ok()
                            || matches!(&last, Err(NetError::Dns(DnsError::NoRecordsFound(_)))),
                    )
                    .map_err(|_| NetError::Timeout)?;
                }
                if last.is_ok() || matches!(&last, Err(NetError::Dns(DnsError::NoRecordsFound(_))))
                {
                    break;
                }
                // DNSSEC 错误不以其他上游结果掩盖，防止验证失败被当成普通重试。
                if self.config.dnssec && !matches!(&last, Err(NetError::Timeout)) {
                    break;
                }
            }
            last
        })
        .await
        .map_err(|_| {
            let _ = self.observe(started.elapsed().as_millis() as u64, false, false);
            Error::DnsTimeout
        })?;
        let mut answer = Answer {
            name,
            query_type: kind.to_string(),
            records: vec![],
            addresses: vec![],
            ttl: 0,
            rcode: 0,
            validation: if self.config.dnssec {
                "insecure"
            } else {
                "disabled"
            }
            .into(),
            from_cache: false,
            duration_ms: started.elapsed().as_millis() as u64,
            wire: vec![],
        };
        let mut cache_deadline = None;
        match lookup {
            Ok(lookup) => {
                answer.wire = wire::encode(lookup.message())?;
                if lookup.answers().len() > 128 {
                    return Err(Error::ResourceLimit);
                }
                let mut size = 0;
                let mut secure = true;
                let now = Instant::now();
                let deadline = lookup.valid_until().min(now + Duration::from_secs(3600));
                answer.ttl = deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .div_ceil(1000) as u32;
                cache_deadline = Some(deadline);
                for record in lookup.answers() {
                    let data = &record.data;
                    let text = data.to_string();
                    let owner = record.name.to_ascii();
                    size += text.len() + owner.len();
                    if size > MAX_RESULT_BYTES {
                        return Err(Error::ResourceLimit);
                    }
                    if matches!(data, RData::A(_) | RData::AAAA(_)) {
                        answer.addresses.push(text.clone());
                    }
                    secure &= record.proof == Proof::Secure;
                    answer.records.push(Record {
                        name: owner,
                        record_type: record.record_type().to_string(),
                        ttl: record.ttl,
                        data: text,
                    });
                }
                if self.config.dnssec && secure && !answer.records.is_empty() {
                    answer.validation = "secure".into();
                }
            }
            Err(NetError::Dns(DnsError::NoRecordsFound(no_records))) => {
                let mut message = hickory_resolver::proto::op::Message::query();
                message.metadata.response_code = no_records.response_code;
                if let Some(authorities) = &no_records.authorities {
                    if authorities.len() > 256 {
                        return Err(Error::ResourceLimit);
                    }
                    message.authorities = authorities.to_vec();
                }
                if !message
                    .authorities
                    .iter()
                    .any(|r| r.record_type() == RecordType::SOA)
                    && let Some(soa) = &no_records.soa
                {
                    message.add_authority(hickory_resolver::proto::rr::Record::from_rdata(
                        soa.name.clone(),
                        soa.ttl,
                        RData::SOA(soa.data.clone()),
                    ));
                }
                answer.wire = wire::encode(&message)?;
                answer.rcode = u16::from(no_records.response_code);
                if answer.rcode != 0 && answer.rcode != 3 {
                    return Err(Error::DnsResponse);
                }
                answer.ttl = no_records.negative_ttl.unwrap_or(0).min(300);
                if self.config.dnssec {
                    let secure = no_records.authorities.as_ref().is_some_and(|records| {
                        !records.is_empty() && records.iter().all(|r| r.proof == Proof::Secure)
                    });
                    answer.validation = if secure { "secure" } else { "insecure" }.into();
                    // 仅按已验证且裁剪过 TTL 的 SOA/NSEC 缓存否定结果。
                    if secure && let Some(soa) = &no_records.soa {
                        let proof_ttl = no_records
                            .authorities
                            .as_ref()
                            .and_then(|records| records.iter().map(|r| r.ttl).min())
                            .unwrap_or(0);
                        answer.ttl = soa.ttl.min(soa.data.minimum).min(proof_ttl).min(300);
                    }
                }
            }
            Err(NetError::Timeout) => return Err(Error::DnsTimeout),
            Err(_) if self.config.dnssec => return Err(Error::DnsValidation),
            Err(_) => return Err(Error::DnsResponse),
        }
        if answer.ttl > 0 {
            let mut cache = self.cache.lock().map_err(|_| Error::Internal)?;
            let now = Instant::now();
            cache.retain(|_, v| v.expires > now);
            if cache.len() >= MAX_CACHE
                && let Some(old) = cache
                    .iter()
                    .min_by_key(|(_, v)| v.expires)
                    .map(|(k, _)| k.clone())
            {
                cache.remove(&old);
            }
            // 返回文本上限 16 KiB；固定 128 KiB 覆盖最多 64 KiB 报文、展示记录和容器开销。
            if let Ok(lease) = CACHE_BUDGET.reserve(128 * 1024) {
                cache.insert(
                    key,
                    Cached {
                        expires: cache_deadline
                            .unwrap_or(now + Duration::from_secs(u64::from(answer.ttl))),
                        answer: answer.clone(),
                        _lease: lease,
                    },
                );
            }
        }
        Ok(answer)
    }
}
pub(super) fn normalize(name: &str) -> Result<String> {
    if name == "." {
        return Ok(".".into());
    }
    let name = name.strip_suffix('.').unwrap_or(name);
    if name.is_empty()
        || name.len() > 253
        || !name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
    {
        return Err(Error::InvalidInput);
    }
    Ok(name.to_ascii_lowercase())
}

fn validated_query(query: Query) -> Result<(String, RecordType)> {
    let name = normalize(&query.name)?;
    if query.query_type.len() > 16 {
        return Err(Error::InvalidInput);
    }
    let kind = RecordType::from_str(&query.query_type.to_ascii_uppercase())
        .map_err(|_| Error::Unsupported)?;
    if matches!(
        kind,
        RecordType::ANY
            | RecordType::AXFR
            | RecordType::IXFR
            | RecordType::OPT
            | RecordType::Unknown(_)
    ) {
        return Err(Error::Unsupported);
    }
    Ok((name, kind))
}
