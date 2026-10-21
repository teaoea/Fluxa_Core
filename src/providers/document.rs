//! 远程正文只能声明条件，策略/来源/最终兜底不能由订阅提升或伪造。
use crate::{
    error::{Error, Result},
    routing::{Match, Policy, Rule, Transport},
};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    rules: Vec<serde_json::Value>,
}
pub(super) struct Parsed {
    pub rules: Vec<Rule>,
    pub ignored: usize,
    pub _memory: crate::budget::ByteLease,
}
/// YAML 结构错误整体失败；结构正确但非法/自带策略的行单独忽略，不影响其他合法条件。
pub(super) fn parse(
    input: &str,
    policy: Policy,
    outbound: Option<String>,
    geoip: bool,
    node_names: &[&str],
) -> Result<Parsed> {
    let memory = crate::routing::runtime::reserve_parse(input, super::BODY_LIMIT)?;
    let payload: Payload = crate::routing::runtime::yaml::decode(input)?;
    let mut rules = Vec::new();
    rules
        .try_reserve(payload.rules.len())
        .map_err(|_| Error::ResourceLimit)?;
    let mut ignored = 0;
    for row in payload.rules {
        let Some(line) = row.as_str() else {
            ignored += 1;
            continue;
        };
        let rule = condition(line, policy, outbound.clone(), geoip, node_names);
        match rule {
            Ok(Some(rule)) => rules.push(rule),
            Ok(None) | Err(Error::InvalidInput | Error::Unsupported) => ignored += 1,
            Err(error) => return Err(error),
        }
    }
    Ok(Parsed {
        rules,
        ignored,
        _memory: memory,
    })
}
fn condition(
    line: &str,
    policy: Policy,
    outbound: Option<String>,
    geoip: bool,
    nodes: &[&str],
) -> Result<Option<Rule>> {
    let mut parts = [""; 3];
    let mut count = 0;
    for field in line.split(',') {
        if count == 3 || field.is_empty() || field != field.trim() {
            return Ok(None);
        }
        parts[count] = field;
        count += 1;
    }
    if count < 2 {
        return Ok(None);
    }
    let mut protocol = None;
    let (kind, value) = match parts[0] {
        "domain" if count == 2 => (Match::Domain, parts[1].to_owned()),
        "domain-suffix" if count == 2 && policy != Policy::Reject => {
            (Match::DomainSuffix, parts[1].to_owned())
        }
        "domain-keyword" if count == 2 && policy != Policy::Reject => {
            if parts[1].len() > 63
                || !parts[1]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return Ok(None);
            }
            (Match::DomainKeyword, parts[1].to_owned())
        }
        "ip-cidr" if count == 2 => (Match::IpCidr, parts[1].to_owned()),
        "geoip" if count == 2 && geoip => (Match::GeoIp, parts[1].to_owned()),
        // 进程身份是本机条件，只允许 macOS 本地正式规则；远程行一律忽略，不能注入进程策略。
        "process-name" => return Ok(None),
        "port" if count == 2 => (Match::DestinationPort, parts[1].to_owned()),
        "port" if count == 3 => {
            if !parts[1].bytes().all(|b| b.is_ascii_digit())
                || matches!(parts[2], "direct" | "reject")
                || nodes.contains(&parts[2])
            {
                return Ok(None);
            }
            (Match::DomainPort, format!("{},{}", parts[1], parts[2]))
        }
        "protocol" if count == 2 && policy == Policy::Reject => {
            (Match::Transport, parts[1].to_owned())
        }
        "protocol" if count == 3 => {
            if matches!(parts[2], "direct" | "reject") || nodes.contains(&parts[2]) {
                return Ok(None);
            }
            match parts[1] {
                "tcp" => protocol = Some(Transport::Tcp),
                "udp" => protocol = Some(Transport::Udp),
                "quic" => {
                    return crate::routing::runtime::normalize_rule(
                        &Rule {
                            r#type: Match::DomainQuic,
                            value: parts[2].into(),
                            protocol: None,
                            policy,
                            outbound: outbound.clone(),
                        },
                        &outbound,
                    )
                    .map(Some);
                }
                _ => return Ok(None),
            }
            (Match::DomainSuffix, parts[2].to_owned())
        }
        _ => return Ok(None),
    };
    let rule = Rule {
        r#type: kind,
        value,
        protocol,
        policy,
        outbound: outbound.clone(),
    };
    crate::routing::runtime::normalize_rule(&rule, &outbound).map(Some)
}

#[cfg(test)]
#[path = "../../tests/test_providers_document.rs"]
mod test_providers_document;
