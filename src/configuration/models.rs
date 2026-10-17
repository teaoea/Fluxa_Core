//! 主配置的严格字段模型；不接受旧别名，不授予网络状态或资源控制权。
use serde::Deserialize;
use std::collections::BTreeMap;

/// 主配置运行模式；与旧路由的内部 Rules 拼写分离。
#[derive(Clone, Copy, Debug, Default, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Rule,
    Global,
    Direct,
}
/// 日志选择是精确类别，不是按严重程度向上筛选；Info 表示完整记录。
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFilter {
    #[default]
    Silent,
    Info,
    Debug,
    Warn,
    Error,
}
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum IpStrategy {
    #[default]
    PreferIpv4,
    PreferIpv6,
    OnlyIpv4,
    OnlyIpv6,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Raw {
    #[serde(default)]
    pub log_level: LogFilter,
    #[serde(default)]
    pub system_proxy: SystemProxy,
    #[serde(default)]
    pub lan_share: LanShare,
    #[serde(default)]
    pub proxy_groups: Vec<ReservedProxyProvider>,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub geoip: Geoip,
    #[serde(default)]
    pub ip_strategy: IpStrategy,
    #[serde(default)]
    pub proxies: Vec<Proxy>,
    // 两种提供者尚不接入执行；只接受已知的保留形状，不能以任意 JSON 绕过字段校验。
    #[serde(default)]
    pub proxy_providers: Vec<ReservedProxyProvider>,
    #[serde(default)]
    pub plugin_providers: Vec<PluginProvider>,
    #[serde(default)]
    pub rule_providers: Vec<RuleProvider>,
    pub rules: Vec<String>,
    #[serde(default)]
    pub outbound_limits: crate::outbound::Limits,
    #[serde(default)]
    pub dns: Dns,
    #[serde(default)]
    pub tun: Tun,
    #[serde(default)]
    pub mitm: Mitm,
    #[serde(default)]
    pub rewrites: Vec<crate::rewrite::Rewrite>,
}
/// 系统代理与局域网分别使用严格对象，不接受原先分散的顶层端口/认证字段。
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SystemProxy {
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub bypass: Vec<String>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LanShare {
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub password: String,
}
impl Drop for LanShare {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.account.zeroize();
        self.password.zeroize();
    }
}
// 不允许在尚未规定 proxy-providers 业务格式时猜测并接受活动配置。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReservedProxyProvider {}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Geoip {
    #[serde(default)]
    pub enabled: bool,
    pub url: Option<String>,
    pub update_time: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Proxy {
    pub name: String,
    pub r#type: String,
    pub server: String,
    pub port: u16,
    pub uuid: String,
    pub flow: String,
    pub security: String,
    pub sni: String,
    pub pbk: String,
    pub sid: String,
    #[serde(default = "fingerprint")]
    pub fp: String,
}
fn fingerprint() -> String {
    "chrome133".into()
}
impl Drop for Proxy {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.uuid.zeroize();
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct RuleProvider {
    pub name: String,
    pub url: String,
    pub policy: String,
    pub enable: bool,
    pub update_time: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct PluginProvider {
    pub name: String,
    pub category: String,
    pub url: String,
    pub enabled: bool,
    pub update_time: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Dns {
    #[serde(default)]
    pub enable: bool,
    #[serde(rename = "bootstrap")]
    pub default_nameserver: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "server")]
    pub nameserver: Vec<String>,
    #[serde(default = "dns_timeout")]
    pub timeout: i64,
    #[serde(default)]
    pub dnssec: bool,
    pub fake_ip: Option<FakeIp>,
    #[serde(default)]
    pub hosts: BTreeMap<String, String>,
}
pub(super) const fn dns_timeout() -> i64 {
    3
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct FakeIp {
    pub ipv4_range: String,
    pub ipv6_range: String,
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    #[serde(rename = "bypass")]
    pub exclude: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Tun {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "tun_name")]
    pub name: String,
    #[serde(default = "mtu")]
    pub mtu: u16,
    #[serde(default, rename = "bypass")]
    pub exclude_routes: Vec<String>,
}
fn tun_name() -> String {
    "utun".into()
}
const fn mtu() -> u16 {
    1500
}
impl Default for Tun {
    fn default() -> Self {
        Self {
            enable: false,
            name: tun_name(),
            mtu: mtu(),
            exclude_routes: Vec::new(),
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mitm {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "skip-server-cert-verify")]
    pub skip_server_cert_verify: bool,
    #[serde(default, rename = "capture-body")]
    pub capture_body: bool,
    #[serde(default)]
    pub hosts: Vec<String>,
    #[serde(default, rename = "bypass")]
    pub exclude_hosts: Vec<String>,
}
#[cfg(test)]
#[path = "../../tests/test_configuration_models.rs"]
mod test_configuration_models;
