//! 主配置的内核校验入口：先预算、再严格解析、最后校验交叉引用。
//! 成功只表示配置合法，不表示资源已启动、节点可用或 VPN 已接管。
pub(crate) mod models;
mod rules;
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use models::*;
pub use models::{LogFilter, Mode};
use serde::Serialize;
use std::{collections::HashSet, net::IpAddr};

/// 主配置不含远程正文；独立于大规模规则生成输入，限制解析常驻内存。
pub const MAX_CONFIG_BYTES: usize = 4 * 1024 * 1024;
const CONFIG_MEMORY_LIMIT: usize = 256 * 1024 * 1024;
const PARSE_FACTOR: usize = 64;
const PARSE_BASE: usize = 64 * 1024;
const MAX_NODES: usize = 256;
const MAX_PROVIDERS: usize = 64;
const MAX_DNS_SERVERS: usize = 8;
const MIN_LISTEN_PORT: u16 = 1054;
const MAX_ADDRESS_FILTERS: usize = 128;
// SOCKS5 用户名/密码长度字段只有一个字节，HTTP 和 SOCKS 共用同一份凭据。
const MAX_AUTH_BYTES: usize = u8::MAX as usize;
static MEMORY: ByteBudget = ByteBudget::new(CONFIG_MEMORY_LIMIT);

/// 不可变的已校验配置；无 Deserialize/Clone，不能绕过校验或无预算复制整份配置。
/// 持有数据多久，就保留对应内存租约；本对象没有启动/停止或绑定资源的权限。
pub struct Document {
    raw: Raw,
    _memory: ByteLease,
}
/// 可公开返回的有限摘要，不包含节点凭据、URL、主机列表或可变管理器。
#[derive(Debug, Serialize)]
pub struct Summary {
    pub mode: Mode,
    pub system_proxy_port: u16,
    pub lan_share_port: u16,
    pub node_count: usize,
    pub rule_count: usize,
    pub rule_provider_count: usize,
    pub dns_enabled: bool,
    pub dns_timeout_seconds: u64,
    pub geoip_enabled: bool,
    pub tun_requested: bool,
}
impl Document {
    /// YAML/JSON 都通过同一严格事件解析器，拒绝未知/重复字段、标签、别名及过深嵌套。
    /// 未取得驻留预算时不会开始反序列化；失败不修改任何有效网络配置。
    pub fn from_yaml(input: &str) -> Result<Self> {
        Self::parse_with_budget(input, &MEMORY)
    }
    // 同一解析实现内部接收预算；生产只使用全局预算，测试不与其他模块争抢计数。
    fn parse_with_budget(input: &str, memory: &'static ByteBudget) -> Result<Self> {
        if input.len() > MAX_CONFIG_BYTES {
            return Err(Error::ResourceLimit);
        }
        let bytes = input
            .len()
            .checked_mul(PARSE_FACTOR)
            .and_then(|n| n.checked_add(PARSE_BASE))
            .ok_or(Error::ResourceLimit)?;
        let lease = memory.reserve(bytes)?;
        let mut raw: Raw = crate::routing::runtime::yaml::decode(input)?;
        validate(&mut raw)?;
        Ok(Self {
            raw,
            _memory: lease,
        })
    }
    /// 只读返回有效 GeoIP 来源；禁用时即使保留 URL，也不能被下游误当作启用。
    /// 借用随已校验配置存活，不发起下载，不产生 Ready 状态。
    pub fn geoip_url(&self) -> Option<&str> {
        if self.raw.geoip.enabled {
            self.raw.geoip.url.as_deref()
        } else {
            None
        }
    }
    /// 编译本地规则，返回不可变候选快照；不会替换正在运行的配置。
    /// 规则提供者由后续内核编排合并；调用者不得把本地候选当作完整配置启动成功。
    /// 例如 `Document::from_yaml("rules: ['final,direct']")?.compile_local_rules(1)?`。
    pub fn compile_local_rules(self, version: u64) -> Result<crate::routing::runtime::Snapshot> {
        rules::compile(&self.raw, version)
    }
    /// 编译时验证启用状态、显式 URL 与 Ready 服务属于同一来源，并绑定该不可变数据库。
    /// 例如已经成功 refresh 的 service 才能用于启用了 geoip.url 的主配置；来源不符返回 2408。
    pub fn compile_local_rules_with_geoip(
        self,
        version: u64,
        service: &crate::geoip::Service,
    ) -> Result<crate::routing::runtime::Snapshot> {
        let database = if self.raw.geoip.enabled {
            Some(
                service.database_for_url(
                    self.raw
                        .geoip
                        .url
                        .as_deref()
                        .ok_or(Error::GeoipInvalidUrl)?,
                )?,
            )
        } else {
            None
        };
        rules::compile_ready(&self.raw, version, database)
    }
    /// 只读的认证选择，不返回账号密码；未配置时允许无认证，不代表监听已运行。
    pub fn lan_authentication_enabled(&self) -> bool {
        !self.raw.lan_share.account.is_empty()
    }
    /// 只计算配置允许的域名匹配范围；bypass 永远优先，不能启用全量解密。
    /// 例如排除 *.chatgpt.com 时，chatgpt.com 和各级子域名都不解密。
    /// 此结果不代表 CA 已受信任，不替代解密启动的授权和生命周期检查。
    pub fn mitm_matches(&self, host: &str) -> Result<bool> {
        let host = crate::network::normalize_host(host)?;
        // 配置对象没有调试授权；即使将来增加运行期调试范围，也不能从这里启用全量解密。
        Ok(mitm_scope(&self.raw.mitm, &host))
    }
    pub fn summary(&self) -> Summary {
        Summary {
            mode: self.raw.mode,
            system_proxy_port: self.raw.system_proxy.port,
            lan_share_port: self.raw.lan_share.port,
            node_count: self.raw.proxies.len(),
            rule_count: self.raw.rules.len(),
            rule_provider_count: self.raw.rule_providers.len(),
            dns_enabled: self.raw.dns.enable,
            dns_timeout_seconds: if self.raw.dns.enable {
                self.raw.dns.timeout as u64
            } else {
                0
            },
            geoip_enabled: self.raw.geoip.enabled,
            tun_requested: self.raw.tun.enable,
        }
    }
}
fn validate(raw: &mut Raw) -> Result<()> {
    for port in [raw.system_proxy.port, raw.lan_share.port] {
        if port != 0 && port < MIN_LISTEN_PORT {
            return Err(Error::InvalidInput);
        }
    }
    if raw.system_proxy.port != 0 && raw.system_proxy.port == raw.lan_share.port {
        return Err(Error::InvalidInput);
    }
    if raw.proxies.len() > MAX_NODES
        || raw.rule_providers.len() > MAX_PROVIDERS
        || raw.plugin_providers.len() > MAX_PROVIDERS
    {
        return Err(Error::ResourceLimit);
    }
    if !raw.proxy_providers.is_empty() || !raw.proxy_groups.is_empty() {
        return Err(Error::Unsupported);
    }
    let mut nodes = HashSet::new();
    for proxy in &raw.proxies {
        name(&proxy.name)?;
        if matches!(proxy.name.as_str(), "direct" | "reject") || !nodes.insert(proxy.name.as_str())
        {
            return Err(Error::InvalidInput);
        }
        validate_proxy(proxy)?;
    }
    rules::validate(&raw.rules, &nodes)?;
    let mut providers = HashSet::new();
    for provider in &raw.rule_providers {
        provider_file_name(&provider.name)?;
        // 缓存文件跨默认大小写不敏感文件系统使用，拒绝可能指向同一文件的名称。
        if !providers.insert(provider.name.to_ascii_lowercase()) {
            return Err(Error::InvalidInput);
        }
        https_url(&provider.url)?;
        rules::policy(&provider.policy, &nodes)?;
        daily_time(provider.update_time.as_deref())?;
        // enable 仅控制后续下载/参与合并，不能豁免元数据合法性。
        let _enabled = provider.enable;
    }
    for provider in &raw.plugin_providers {
        name(&provider.name)?;
        name(&provider.category)?;
        https_url(&provider.url)?;
        daily_time(provider.update_time.as_deref())?;
        if provider.enabled {
            return Err(Error::Unsupported);
        }
    }
    if raw.geoip.enabled {
        crate::geoip::Config {
            geoip_url: Some(raw.geoip.url.clone().ok_or(Error::GeoipInvalidUrl)?),
        }
        .validate()?;
        daily_time(raw.geoip.update_time.as_deref())?;
    }
    validate_dns(&mut raw.dns, &nodes)?;
    raw.outbound_limits.validate()?;
    validate_auth(&raw.lan_share.account, &raw.lan_share.password)?;
    validate_patterns(&raw.system_proxy.bypass, true)?;
    if raw.tun.exclude_routes.len() > MAX_ADDRESS_FILTERS {
        return Err(Error::ResourceLimit);
    }
    for route in &raw.tun.exclude_routes {
        crate::network::Network::parse(route)?;
    }
    if raw.tun.enable {
        if !(1280..=65535).contains(&raw.tun.mtu) {
            return Err(Error::InvalidInput);
        }
        if !cfg!(target_vendor = "apple")
            && (raw.tun.name.is_empty()
                || raw.tun.name.len() > 15
                || raw.tun.name.starts_with(|c: char| c.is_ascii_digit())
                || !raw
                    .tun
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')))
        {
            return Err(Error::InvalidInput);
        }
    }
    if raw.mitm.enabled && raw.mitm.hosts.is_empty() {
        return Err(Error::InvalidInput);
    }
    // 正式配置不包含全量解密字段；hosts 始终按普通域名/通配符范围校验。
    // 专用调试全量解密必须由独立授权路径控制。
    validate_patterns(&raw.mitm.hosts, false)?;
    validate_patterns(&raw.mitm.exclude_hosts, false)?;
    if raw.mitm.hosts.len() > MAX_ADDRESS_FILTERS || raw.rewrites.len() > 1024 {
        return Err(Error::ResourceLimit);
    }
    // 记录配置选择，实际 TLS/正文捕获管线在后续完整配置事务接入，不能据此声明运行成功。
    let _mitm_execution = (raw.mitm.skip_server_cert_verify, raw.mitm.capture_body);
    for rewrite in &raw.rewrites {
        rewrite.validate()?;
    }
    // 类型本身穷尽限制了值；后续网络准备读取这些字段，校验成功不等于已经应用。
    let _selection = (raw.log_level, raw.ip_strategy, raw.dns.dnssec);
    Ok(())
}
fn name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value != value.trim()
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, ',' | '/' | '\\'))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
pub(crate) fn provider_file_name(value: &str) -> Result<()> {
    name(value)?;
    if matches!(value, "." | "..")
        || value.ends_with('.')
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn https_url(value: &str) -> Result<()> {
    if value.len() > 1024 || value.bytes().any(|b| b <= 32 || b == 127) {
        return Err(Error::InvalidInput);
    }
    let url = url::Url::parse(value).map_err(|_| Error::InvalidInput)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port() == Some(0)
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn daily_time(value: Option<&str>) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || ![bytes[0], bytes[1], bytes[3], bytes[4]]
            .iter()
            .all(u8::is_ascii_digit)
        || &value[..2] > "23"
        || &value[3..] > "59"
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn proxy_profile(proxy: &Proxy) -> Result<crate::protocol::config::Profile> {
    if proxy.r#type != "vless" || proxy.flow != "xtls-rprx-vision" || proxy.security != "reality" {
        return Err(Error::Unsupported);
    }
    let host = crate::network::normalize_host(&proxy.server)?;
    if proxy.port == 0 {
        return Err(Error::InvalidInput);
    }
    // 复用真实协议的凭据/指纹验证；URL 编码阻止参数注入，不实现第二套 REALITY 校验器。
    let host = if host.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("[{host}]")
    } else {
        host
    };
    let mut uri = url::Url::parse(&format!("vless://{host}:{}", proxy.port))
        .map_err(|_| Error::InvalidInput)?;
    uri.set_username(&proxy.uuid)
        .map_err(|_| Error::InvalidInput)?;
    uri.query_pairs_mut().extend_pairs([
        ("security", proxy.security.as_str()),
        ("flow", proxy.flow.as_str()),
        ("sni", proxy.sni.as_str()),
        ("pbk", proxy.pbk.as_str()),
        ("sid", proxy.sid.as_str()),
        ("fp", proxy.fp.as_str()),
    ]);
    let profile = crate::protocol::config::Profile {
        id: proxy.name.clone(),
        uri: uri.to_string(),
    };
    Ok(profile)
}
fn validate_proxy(proxy: &Proxy) -> Result<()> {
    proxy_profile(proxy)?.validate()?;
    Ok(())
}
fn validate_dns(dns: &mut Dns, nodes: &HashSet<&str>) -> Result<()> {
    // 关闭后不检查上游、hosts 和 Fake-IP 的业务值，也不创建任何解析器或合成地址池。
    if !dns.enable {
        return Ok(());
    }
    if dns.nameserver.is_empty() || dns.nameserver.len() > MAX_DNS_SERVERS {
        return Err(Error::InvalidInput);
    }
    if !(1..=30).contains(&dns.timeout) {
        dns.timeout = dns_timeout();
    }
    if let Some(servers) = &dns.default_nameserver {
        if servers.is_empty() || servers.len() > MAX_DNS_SERVERS {
            return Err(Error::InvalidInput);
        }
        for value in servers {
            numeric_dns(value)?;
        }
    }
    for value in &dns.nameserver {
        // system 只能由用户在 server 列表中显式声明，保留原位置作为普通上游。
        // 不为未声明的列表补入它，不把失败转换为隐式系统回退；bootstrap 仍只允许数字地址。
        // 此处仅校验配置，顺序查询和系统解析适配由后续 DNS 编排实现。
        if value == "system" {
            continue;
        }
        let (endpoint, route) = value.split_once(',').unwrap_or((value, "direct"));
        if route != "direct" && !nodes.contains(route) {
            return Err(Error::InvalidInput);
        }
        if endpoint.parse::<IpAddr>().is_ok() || endpoint.parse::<std::net::SocketAddr>().is_ok() {
            numeric_dns(endpoint)?;
            continue;
        }
        if endpoint.starts_with("tls://") {
            let url = url::Url::parse(endpoint).map_err(|_| Error::InvalidInput)?;
            crate::network::normalize_host(
                url.host_str()
                    .ok_or(Error::InvalidInput)?
                    .trim_matches(['[', ']']),
            )?;
            if url.port() == Some(0) {
                return Err(Error::InvalidInput);
            }
            if !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || !matches!(url.path(), "" | "/")
            {
                return Err(Error::InvalidInput);
            }
        } else {
            https_url(endpoint)?;
        }
        if route != "direct" && !nodes.contains(route) {
            return Err(Error::InvalidInput);
        }
    }
    for (host, address) in &dns.hosts {
        rules::domain(host)?;
        address.parse::<IpAddr>().map_err(|_| Error::InvalidInput)?;
    }
    if let Some(fake) = &dns.fake_ip
        && fake.enable
    {
        crate::dns::fake_ip::Config::automatic(&fake.ipv4_range, &fake.ipv6_range, &fake.exclude)?;
    }
    Ok(())
}
fn numeric_dns(value: &str) -> Result<()> {
    if value
        .parse::<std::net::SocketAddr>()
        .is_ok_and(|a| a.port() == 0)
    {
        return Err(Error::InvalidInput);
    }
    let address: IpAddr = value
        .parse()
        .or_else(|_| value.parse::<std::net::SocketAddr>().map(|a| a.ip()))
        .map_err(|_| Error::InvalidInput)?;
    if address.is_unspecified()
        || address.is_multicast()
        || address == IpAddr::V4(std::net::Ipv4Addr::BROADCAST)
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
#[cfg(test)]
#[path = "../tests/test_configuration.rs"]
mod test_configuration;

fn validate_auth(account: &str, password: &str) -> Result<()> {
    if account.is_empty() != password.is_empty()
        || [account, password]
            .iter()
            .any(|s| s.len() > MAX_AUTH_BYTES || s.chars().any(char::is_control))
        || account.contains(':')
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn validate_patterns(values: &[String], allow_networks: bool) -> Result<()> {
    if values.len() > MAX_ADDRESS_FILTERS {
        return Err(Error::ResourceLimit);
    }
    for value in values {
        // MITM 两份清单中的单独 * 按用户约定无效，但不使其他合法条目失效。
        // 不对系统代理绕过清单扩充同义全局语法。
        if !allow_networks && value == "*" {
            continue;
        }
        if allow_networks
            && (value.parse::<IpAddr>().is_ok() || crate::network::Network::parse(value).is_ok())
        {
            continue;
        }
        let host = value.strip_prefix("*.").unwrap_or(value);
        if host.contains('*') || host.is_empty() || host.len() > 253 {
            return Err(Error::InvalidInput);
        }
        rules::domain(host)?;
    }
    Ok(())
}
fn host_pattern_matches(pattern: &str, host: &str) -> bool {
    // 单独 * 既不能解密全部域名，也不能绕过全部域名。
    if pattern == "*" {
        return false;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        let Ok(suffix) = crate::network::normalize_host(suffix) else {
            return false;
        };
        // 纯公共/私有后缀（com、co.uk、github.io 等）没有可注册域名，不得当作全量替代。
        // 与关键词共用固定 PSL 数据；无效范围只是不命中，不扩大到所有域名。
        if psl::domain_str(&suffix).is_none() {
            return false;
        }
        // 有效的 *. 基域名包含域名本身和各级子域名，但必须保留标签边界。
        host == suffix
            || host
                .strip_suffix(&suffix)
                .is_some_and(|prefix| prefix.len() > 1 && prefix.ends_with('.'))
    } else {
        crate::network::normalize_host(pattern).is_ok_and(|pattern| pattern == host)
    }
}

// 纯配置范围只有普通 hosts，排除列表总优先；不提供全量开关或调试授权。
fn mitm_scope(config: &Mitm, host: &str) -> bool {
    config.enabled
        && !config
            .exclude_hosts
            .iter()
            .any(|p| host_pattern_matches(p, host))
        && config.hosts.iter().any(|p| host_pattern_matches(p, host))
}

/// 已提交配置的只读状态；Committed 仅表示配置发布，不表示 VPN Running 或节点可用。
/// 无主配置时 configuration 为 None（初始状态或被旧分模块命令修改）。
#[derive(Serialize)]
pub struct ConfigurationStatus {
    pub revision: u64,
    /// 主配置启用状态；下载失败由调用 Err/控制状态 error_info 表达，不能冒充 Disabled。
    pub geoip_status: crate::geoip::Status,
    pub configuration: Option<Summary>,
}

// 候选只在内核调用栈内存活；不向宿主暴露提交令牌、快照或活动资源控制权。
pub(crate) struct Prepared {
    pub document: std::sync::Arc<Document>,
    pub resolver: Option<std::sync::Arc<crate::dns::Resolver>>,
    pub geoip: Option<std::sync::Arc<crate::geoip::Service>>,
    pub router: std::sync::Arc<crate::routing::Router>,
    pub outbounds: std::sync::Arc<
        std::collections::HashMap<String, std::sync::Arc<crate::protocol::config::Node>>,
    >,
}
impl Document {
    /// 只读使用已验证限额，不向宿主开放内部配置。
    pub(crate) fn raw_limits(&self) -> crate::outbound::Limits {
        self.raw.outbound_limits
    }

    /// 完整准备当前可执行的本地配置；未接入活动能力整体拒绝，不部分应用。
    /// 不下载、不绑定、不修改当前状态；失败时输入租约和候选索引自动释放。
    pub(crate) fn prepare(self) -> Result<Prepared> {
        if self.needs_runtime() || self.raw.rule_providers.iter().any(|p| p.enable) {
            return Err(Error::Unsupported);
        }
        std::sync::Arc::new(self).prepare_shared(Vec::new())
    }
    pub(crate) fn needs_runtime(&self) -> bool {
        self.raw.dns.enable
            || self.raw.geoip.enabled
            || !matches!(self.raw.ip_strategy, IpStrategy::PreferIpv4)
    }
    pub(crate) fn dns_settings(&self) -> &models::Dns {
        &self.raw.dns
    }
    pub(crate) fn geoip_time(&self) -> Option<&str> {
        self.raw.geoip.update_time.as_deref()
    }
    pub(crate) fn ip_strategy(&self) -> models::IpStrategy {
        self.raw.ip_strategy
    }
    pub(crate) fn provider_settings(&self) -> &[models::RuleProvider] {
        &self.raw.rule_providers
    }
    pub(crate) fn provider_nodes(&self) -> Vec<&str> {
        self.raw.proxies.iter().map(|p| p.name.as_str()).collect()
    }
    /// 内核持有经提供者正文验证的 Remote 来源，缺失/额外来源不允许部分提交。
    pub(crate) fn prepare_shared(
        self: std::sync::Arc<Self>,
        sources: Vec<crate::routing::runtime::Source>,
    ) -> Result<Prepared> {
        self.prepare_inner(sources, false, None)
    }
    pub(crate) fn prepare_download(
        self: std::sync::Arc<Self>,
        database: Option<std::sync::Arc<crate::geoip::Database>>,
    ) -> Result<Prepared> {
        self.prepare_inner(Vec::new(), true, database)
    }
    pub(crate) fn provider_preflight(&self) -> Result<()> {
        let raw = &self.raw;
        if raw.tun.enable
            || raw.mitm.enabled
            || raw.system_proxy.port != 0
            || raw.lan_share.port != 0
            || !raw.rewrites.is_empty()
            || !matches!(raw.log_level, LogFilter::Silent)
        {
            return Err(Error::Unsupported);
        }
        // 来源总数包含本地来源，过量请求必须在下载前受控拒绝。
        if raw.rule_providers.iter().filter(|p| p.enable).count()
            >= crate::routing::runtime::MAX_SOURCES
        {
            return Err(Error::ResourceLimit);
        }
        Ok(())
    }
    pub(crate) fn prepare_ready(
        self: std::sync::Arc<Self>,
        sources: Vec<crate::routing::runtime::Source>,
        resolver: Option<std::sync::Arc<crate::dns::Resolver>>,
        geoip: Option<std::sync::Arc<crate::geoip::Service>>,
    ) -> Result<Prepared> {
        let database = geoip
            .as_ref()
            .map(|s| s.database_for_url(self.geoip_url().ok_or(Error::GeoipInitializationFailed)?))
            .transpose()?;
        let mut prepared = self.prepare_inner(sources, false, database)?;
        prepared.resolver = resolver;
        prepared.geoip = geoip;
        Ok(prepared)
    }
    fn prepare_inner(
        self: std::sync::Arc<Self>,
        sources: Vec<crate::routing::runtime::Source>,
        download: bool,
        database: Option<std::sync::Arc<crate::geoip::Database>>,
    ) -> Result<Prepared> {
        let raw = &self.raw;
        self.provider_preflight()?;
        if !download
            && raw
                .rule_providers
                .iter()
                .filter(|provider| provider.enable)
                .count()
                != sources.len()
        {
            return Err(Error::InvalidInput);
        }
        let mut outbounds = std::collections::HashMap::new();
        outbounds
            .try_reserve(raw.proxies.len())
            .map_err(|_| Error::ResourceLimit)?;
        for proxy in &raw.proxies {
            outbounds.insert(
                proxy.name.clone(),
                std::sync::Arc::new(proxy_profile(proxy)?.validate()?),
            );
        }
        let version = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let router =
            std::sync::Arc::new(crate::routing::Router::from_runtime(std::sync::Arc::new(
                rules::compile_sources_inner(raw, version, database, sources, !download)?
                    .into_flat()?,
            ))?);
        Ok(Prepared {
            resolver: None,
            geoip: None,
            document: self,
            router,
            outbounds: std::sync::Arc::new(outbounds),
        })
    }
}
