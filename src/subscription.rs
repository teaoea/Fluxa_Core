//! 独立旧 UTC 计划 API 与共用 HTTPS 下载；新主配置本地日历由 providers 自主运行。
//! Check 仅计算传入状态，不是 rule-providers 的活动调度器。
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
const DAY: u64 = 86400;
const RETRY: u64 = 60;
// 限定公历 1970–9999 年，防止跨语言时间转换或下一天运算溢出。
const MAX_TIME: u64 = 253402300799;
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    #[serde(rename = "update-time", default)]
    pub update_time: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub schedule: Schedule,
    pub now: u64,
    pub last_success: Option<u64>,
    pub last_attempt: Option<u64>,
    pub enabled: bool,
    pub manual: bool,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Decision {
    pub due: bool,
    pub next_update: Option<u64>,
}
impl Schedule {
    fn seconds(&self) -> Result<Option<u64>> {
        let Some(value) = &self.update_time else {
            return Ok(None);
        };
        let bytes = value.as_bytes();
        if bytes.len() != 5
            || bytes[2] != b':'
            || ![0, 1, 3, 4].iter().all(|i| bytes[*i].is_ascii_digit())
        {
            return Err(Error::InvalidInput);
        }
        let hour = u64::from(bytes[0] - b'0') * 10 + u64::from(bytes[1] - b'0');
        let minute = u64::from(bytes[3] - b'0') * 10 + u64::from(bytes[4] - b'0');
        if hour > 23 || minute > 59 {
            return Err(Error::InvalidInput);
        }
        Ok(Some(hour * 3600 + minute * 60))
    }
    pub fn validate(&self) -> Result<()> {
        self.seconds().map(|_| ())
    }
}
impl Check {
    pub fn evaluate(&self) -> Result<Decision> {
        let seconds = self.schedule.seconds()?;
        if [Some(self.now), self.last_success, self.last_attempt]
            .into_iter()
            .flatten()
            .any(|t| t > MAX_TIME)
        {
            return Err(Error::InvalidInput);
        }
        // 手动操作不受定时字段、订阅启停限制；它不会自动启用订阅。
        if self.manual {
            return Ok(Decision {
                due: true,
                next_update: None,
            });
        }
        let Some(seconds) = seconds.filter(|_| self.enabled) else {
            return Ok(Decision {
                due: false,
                next_update: None,
            });
        };
        let today = self.now / DAY * DAY + seconds;
        let latest = if self.now >= today {
            Some(today)
        } else {
            today.checked_sub(DAY)
        };
        let pending = latest.is_some_and(|slot| self.last_success.is_none_or(|time| time < slot));
        // 尝试只限速，不代表成功；断网或中途退出后仍可补最近一次到期任务。
        let retry = self.last_attempt.map(|time| time.saturating_add(RETRY));
        let due = pending && retry.is_none_or(|time| self.now >= time);
        let mut next = if self.now < today {
            today
        } else {
            today.checked_add(DAY).ok_or(Error::ResourceLimit)?
        };
        if pending
            && !due
            && let Some(retry) = retry
        {
            next = next.min(retry);
        }
        Ok(Decision {
            due,
            next_update: (next <= MAX_TIME).then_some(next),
        })
    }
}
/// 下载仅接受无凭据 HTTPS；重定向仍执行同样校验，不携带 cookie 或认证头。
pub async fn fetch(
    client: &crate::https::HttpsClient,
    address: &str,
) -> Result<crate::https::Response> {
    fetch_limited(client, address, 8 * 1024 * 1024).await
}
pub(crate) async fn fetch_limited(
    client: &crate::https::HttpsClient,
    address: &str,
    maximum: usize,
) -> Result<crate::https::Response> {
    if maximum == 0 || maximum > 8 * 1024 * 1024 {
        return Err(Error::InvalidInput);
    }
    if address.len() > 4096 {
        return Err(Error::ResourceLimit);
    }
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        let mut url = url::Url::parse(address).map_err(|_| Error::InvalidInput)?;
        for attempt in 0..=5 {
            if url.as_str().len() > 4096 {
                return Err(Error::ResourceLimit);
            }
            if url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return Err(Error::InvalidInput);
            }
            let response = client.execute_limited(url.as_str(), maximum).await?;
            if response.status == 200 {
                std::str::from_utf8(&response.body).map_err(|_| Error::InvalidInput)?;
                return Ok(response);
            }
            if !matches!(response.status, 301 | 302 | 303 | 307 | 308) {
                return Err(Error::Network);
            }
            if attempt == 5 {
                return Err(Error::ResourceLimit);
            }
            let mut locations = response
                .headers
                .iter()
                .filter(|h| h.name.eq_ignore_ascii_case("location"));
            let value = locations.next().ok_or(Error::InvalidInput)?;
            if locations.next().is_some() {
                return Err(Error::InvalidInput);
            }
            url = url.join(&value.value).map_err(|_| Error::InvalidInput)?;
        }
        Err(Error::ResourceLimit)
    })
    .await
    .map_err(|_| Error::NetworkTimeout)?
}
/// Clash classical YAML：动作由订阅配置提供；不接受列表内第三列策略、no-resolve 或未知类型。
pub fn parse_rules(
    input: &str,
    policy: crate::routing::Policy,
    outbound: Option<String>,
) -> Result<Vec<crate::routing::Rule>> {
    let _temporary =
        crate::routing::runtime::reserve_parse(input, crate::routing::runtime::MAX_PAYLOAD_BYTES)?;
    if let Some(id) = &outbound {
        crate::routing::identifier(id)?;
        if policy != crate::routing::Policy::Proxy {
            return Err(Error::InvalidInput);
        }
    }
    use crate::routing::{Match, Rule};
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Payload {
        payload: Vec<String>,
    }
    let payload: Payload = crate::routing::runtime::yaml::decode(input)?;
    if payload.payload.len() > crate::routing::runtime::MAX_RULES {
        return Err(Error::ResourceLimit);
    }
    let mut rules = Vec::with_capacity(payload.payload.len());
    for line in payload.payload {
        let mut fields = line.split(',');
        let kind = fields.next().ok_or(Error::InvalidInput)?;
        let value = fields.next().ok_or(Error::InvalidInput)?;
        if fields.next().is_some() || kind.trim() != kind || value.trim() != value {
            return Err(Error::InvalidInput);
        }
        let kind = match kind {
            "DOMAIN" => Match::Domain,
            "DOMAIN-SUFFIX" => Match::DomainSuffix,
            "DOMAIN-KEYWORD" => Match::DomainKeyword,
            "IP-CIDR" => Match::IpCidr,
            _ => return Err(Error::Unsupported),
        };
        // 与主配置、生成、索引使用同一规范化和匹配条件校验。
        let mut rule = Rule {
            protocol: None,
            r#type: kind,
            value: value.into(),
            policy,
            outbound: outbound.clone(),
        };
        if policy == crate::routing::Policy::Proxy && outbound.is_none() {
            // 默认出站直到合并主配置时才确定，不在下载时绑定当时选中的节点。
            rule.value = crate::routing::runtime::normalize_condition(&rule)?;
        } else {
            rule = crate::routing::runtime::normalize_rule(&rule, &outbound)?;
        }
        rules.push(rule);
    }
    Ok(rules)
}

#[cfg(test)]
#[path = "../tests/test_subscription.rs"]
mod test_subscription;
