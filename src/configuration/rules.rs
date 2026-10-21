//! 主配置标量规则：校验与转换共用固定字段解析，不在宿主另建匹配器。
use crate::{
    error::{Error, Result},
    network::{Network, normalize_host},
};
use std::collections::HashSet;

pub(super) fn policy(value: &str, nodes: &HashSet<&str>) -> Result<()> {
    if matches!(value, "direct" | "reject") || nodes.contains(value) {
        Ok(())
    } else {
        Err(Error::InvalidInput)
    }
}
pub(super) fn domain(value: &str) -> Result<()> {
    // 组合条件只接受基域名；后续匹配使用标签边界的 suffix 语义，不能使用字符串尾缀猜测。
    if normalize_host(value)?.parse::<std::net::IpAddr>().is_ok() {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
pub(super) fn validate_at(
    rules: &[String],
    nodes: &HashSet<&str>,
    current: &mut usize,
) -> Result<()> {
    if rules.is_empty() {
        return Err(Error::InvalidInput);
    }
    if rules.len() > crate::routing::runtime::MAX_RULES {
        return Err(Error::ResourceLimit);
    }
    let mut final_seen = false;
    for (index, rule) in rules.iter().enumerate() {
        *current = index;
        let (parts, count) = fields(rule)?;
        policy(parts[count - 1], nodes)?;
        match parts[0] {
            "final" if count == 2 && index + 1 == rules.len() && !final_seen => {
                final_seen = true;
            }
            "domain" | "domain-suffix" if count == 3 => domain(parts[1])?,
            "domain-keyword" if count == 3 => {
                if parts[1].len() > 63
                    || !parts[1]
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                {
                    return Err(Error::InvalidInput);
                }
            }
            "process-name" if count == 3 => {
                if parts[1].len() > crate::routing::MAX_PROCESS_NAME_BYTES
                    || parts[1]
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '/' | '\\' | '*'))
                {
                    return Err(Error::InvalidInput);
                }
            }
            "ip-cidr" if count == 3 => {
                Network::parse(parts[1])?;
            }
            "geoip" if count == 3 => {
                // 禁用时仍校验规则语法；是否加入有效来源由编译阶段决定，不能将无效配置变成成功。
                if parts[1].len() != 2 || !parts[1].bytes().all(|b| b.is_ascii_uppercase()) {
                    return Err(Error::InvalidInput);
                }
            }
            "port" if count == 3 || count == 4 => {
                if !parts[1].bytes().all(|b| b.is_ascii_digit()) {
                    return Err(Error::InvalidInput);
                }
                let port: u16 = parts[1].parse().map_err(|_| Error::InvalidInput)?;
                if port == 0 {
                    return Err(Error::InvalidInput);
                }
                if count == 4 {
                    domain(parts[2])?;
                }
            }
            "protocol" if count == 3 || count == 4 => {
                if !matches!(parts[1], "tcp" | "udp" | "quic") {
                    return Err(Error::InvalidInput);
                }
                // 全域协议仅允许拦截，不能借此把所有域名改成直连或指定代理。
                if count == 3 && parts[2] != "reject" {
                    return Err(Error::InvalidInput);
                }
                if count == 4 {
                    domain(parts[2])?;
                }
            }
            _ => return Err(Error::InvalidInput),
        }
    }
    if final_seen {
        Ok(())
    } else {
        Err(Error::InvalidInput)
    }
}
// 固定栈上槽位；校验和编译使用相同分词规则，不为每条规则创建临时 Vec。
fn fields(rule: &str) -> Result<([&str; 4], usize)> {
    let mut parts = [""; 4];
    let mut count = 0;
    for part in rule.split(',') {
        if count == parts.len() || part.is_empty() || part != part.trim() {
            return Err(Error::InvalidInput);
        }
        parts[count] = part;
        count += 1;
    }
    if count < 2 {
        return Err(Error::InvalidInput);
    }
    Ok((parts, count))
}
fn action(value: &str) -> (crate::routing::Policy, Option<String>) {
    use crate::routing::Policy;
    match value {
        "direct" => (Policy::Direct, None),
        "reject" => (Policy::Reject, None),
        node => (Policy::Proxy, Some(node.to_owned())),
    }
}
/// 只编译已校验的本地标量，不下载提供者、不启动解析器，也不发布运行状态。
/// final 转入快照兜底；禁用的 GeoIP 行在生成来源前剔除。
pub(super) fn compile(
    raw: &super::models::Raw,
    version: u64,
) -> Result<crate::routing::runtime::Snapshot> {
    compile_ready(raw, version, None)
}
pub(super) fn compile_ready(
    raw: &super::models::Raw,
    version: u64,
    geoip: Option<std::sync::Arc<crate::geoip::Database>>,
) -> Result<crate::routing::runtime::Snapshot> {
    compile_sources(raw, version, geoip, Vec::new())
}
pub(super) fn compile_sources(
    raw: &super::models::Raw,
    version: u64,
    geoip: Option<std::sync::Arc<crate::geoip::Database>>,
    sources: Vec<crate::routing::runtime::Source>,
) -> Result<crate::routing::runtime::Snapshot> {
    compile_sources_inner(raw, version, geoip, sources, true)
}
/// 初次数据库下载的控制上下文没有国家库；仅该显式自举阶段跳过国家条件。
pub(super) fn compile_sources_inner(
    raw: &super::models::Raw,
    version: u64,
    geoip: Option<std::sync::Arc<crate::geoip::Database>>,
    mut sources: Vec<crate::routing::runtime::Source>,
    require_geoip: bool,
) -> Result<crate::routing::runtime::Snapshot> {
    use crate::routing::{
        Config, Match, Mode, Rule, Transport,
        runtime::{Layer, Snapshot, Source},
    };
    let mut rules = Vec::new();
    // Document 的驻留租约覆盖输入和此次有界转换；Snapshot 另行预留完整编译预算。
    rules
        .try_reserve_exact(raw.rules.len())
        .map_err(|_| Error::ResourceLimit)?;
    let mut settings = Config {
        mode: match raw.mode {
            super::Mode::Rule => Mode::Rules,
            super::Mode::Direct => Mode::Direct,
            super::Mode::Global => Mode::Global,
        },
        ..Config::default()
    };
    for scalar in &raw.rules {
        let (p, count) = fields(scalar)?;
        let (policy, outbound) = action(p[count - 1]);
        let mut protocol = None;
        let (kind, value) = match p[0] {
            "final" => {
                settings.fallback = policy;
                settings.outbound = outbound;
                continue;
            }
            // iOS/iPadOS/tvOS 按约定不使用进程规则；macOS 通过 OS 套接字归属匹配，其他未接入平台明确拒绝编译。
            "process-name" if cfg!(any(target_os = "ios", target_os = "tvos")) => continue,
            "process-name" if !cfg!(target_os = "macos") => return Err(Error::Unsupported),
            "process-name" => (Match::ProcessName, p[1].to_owned()),
            "geoip" if !raw.geoip.enabled || (!require_geoip && geoip.is_none()) => continue,
            // GeoIP Ready 及数据库查询由后续编排阶段提供；不得把启用但未准备的规则静默丢弃。
            "geoip" if geoip.is_none() => return Err(Error::GeoipInitializationFailed),
            "geoip" => (Match::GeoIp, p[1].to_owned()),
            "domain" => (Match::Domain, p[1].to_owned()),
            "domain-suffix" => (Match::DomainSuffix, p[1].to_owned()),
            "domain-keyword" => (Match::DomainKeyword, p[1].to_owned()),
            "ip-cidr" => (Match::IpCidr, p[1].to_owned()),
            "port" if count == 4 => (Match::DomainPort, format!("{},{}", p[1], p[2])),
            "port" => (Match::DestinationPort, p[1].to_owned()),
            "protocol" if count == 4 => {
                protocol = Some(match p[1] {
                    "tcp" => Transport::Tcp,
                    "udp" => Transport::Udp,
                    // 独立条件在取得真实 UDP 首包后参与查询，不扩展 Transport 为伪传输层。
                    "quic" => {
                        rules.push(Rule {
                            protocol: None,
                            r#type: Match::DomainQuic,
                            value: p[2].to_owned(),
                            policy,
                            outbound,
                        });
                        continue;
                    }
                    _ => return Err(Error::InvalidInput),
                });
                (Match::DomainSuffix, p[2].to_owned())
            }
            "protocol" => (Match::Transport, p[1].to_owned()),
            _ => return Err(Error::InvalidInput),
        };
        rules.push(Rule {
            protocol,
            r#type: kind,
            value,
            policy,
            outbound,
        });
    }
    if sources
        .iter()
        .any(|s| s.layer != Layer::Remote || !s.enabled)
    {
        return Err(Error::InvalidInput);
    }
    sources.insert(
        0,
        Source {
            id: "local".into(),
            layer: Layer::Local,
            enabled: true,
            rules,
        },
    );
    let snapshot = Snapshot::generate(settings, sources, version)?;
    Ok(match geoip {
        Some(database) => snapshot.bind_geoip(database),
        None => snapshot,
    })
}
#[cfg(test)]
#[path = "../../tests/test_configuration_rules.rs"]
mod test_configuration_rules;
