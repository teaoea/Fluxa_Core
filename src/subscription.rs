//! UTC 每日订阅更新计划；宿主提供时钟和持久状态，不在平台侧复制到期判断。
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
            let response = client
                .execute_limited(url.as_str(), 2 * 1024 * 1024)
                .await?;
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
    .map_err(|_| Error::Network)?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn check(time: Option<&str>, now: u64) -> Check {
        Check {
            schedule: Schedule {
                update_time: time.map(str::to_owned),
            },
            now,
            last_success: None,
            last_attempt: None,
            enabled: true,
            manual: false,
        }
    }
    #[test]
    fn utc_boundary_manual_and_daily_retry() {
        let mut c = check(Some("12:00"), DAY + 43199);
        c.last_success = Some(43200);
        assert!(!c.evaluate().unwrap().due);
        assert_eq!(c.evaluate().unwrap().next_update, Some(DAY + 43200));
        c.now += 1;
        assert!(c.evaluate().unwrap().due);
        c.last_attempt = Some(c.now);
        assert!(!c.evaluate().unwrap().due);
        c.now += DAY;
        assert!(c.evaluate().unwrap().due);
        c.last_success = Some(c.now);
        assert!(!c.evaluate().unwrap().due);
        c.manual = true;
        c.enabled = false;
        assert!(c.evaluate().unwrap().due);
        let mut absent = check(None, c.now);
        assert_eq!(
            absent.evaluate().unwrap(),
            Decision {
                due: false,
                next_update: None
            }
        );
        absent.manual = true;
        assert!(absent.evaluate().unwrap().due);
    }
    #[test]
    fn resume_catches_latest_slot_once_even_before_todays_slot() {
        let mut c = check(Some("12:00"), 10 * DAY + 3600);
        c.last_success = Some(DAY);
        assert!(c.evaluate().unwrap().due);
        assert_eq!(c.evaluate().unwrap().next_update, Some(10 * DAY + 43200));
        c.last_attempt = Some(c.now);
        assert!(!c.evaluate().unwrap().due);
        c.now = 10 * DAY + 43200;
        assert!(c.evaluate().unwrap().due);
    }
    #[test]
    fn manual_before_slot_does_not_suppress_later_slot() {
        let mut c = check(Some("12:00"), DAY + 43200);
        c.last_success = Some(DAY + 43199);
        assert!(c.evaluate().unwrap().due);
        c.last_success = Some(DAY + 43201);
        assert!(!c.evaluate().unwrap().due);
        c.enabled = false;
        c.now += DAY;
        assert_eq!(c.evaluate().unwrap().next_update, None);
    }
    #[test]
    fn failed_or_interrupted_attempt_retries_latest_slot_with_backoff() {
        let mut c = check(Some("12:00"), 10 * DAY + 50000);
        c.last_attempt = Some(c.now);
        assert!(!c.evaluate().unwrap().due);
        assert_eq!(c.evaluate().unwrap().next_update, Some(c.now + RETRY));
        c.now += RETRY;
        assert!(c.evaluate().unwrap().due);
        c.last_success = Some(c.now);
        c.now += RETRY;
        assert!(!c.evaluate().unwrap().due);
    }
    #[test]
    fn strict_input_and_midnight() {
        for value in [
            "", "1:00", "12:0", "24:00", "12:60", "12：00", " 12:00", "12:00Z", "-1:00", "aa:bb",
        ] {
            assert!(check(Some(value), DAY).evaluate().is_err());
        }
        for value in ["00:00", "23:59", "12:00"] {
            assert!(check(Some(value), DAY).evaluate().is_ok());
        }
        assert!(check(Some("00:00"), DAY).evaluate().unwrap().due);
        let mut c = check(Some("12:00"), MAX_TIME + 1);
        assert!(c.evaluate().is_err());
        c.now = 0;
        c.last_attempt = Some(MAX_TIME + 1);
        assert!(c.evaluate().is_err());
        assert!(serde_json::from_str::<Schedule>(r#"{"update-interval-hours":24}"#).is_err());
        assert!(
            serde_json::from_str::<Schedule>(r#"{"update-time":"12:00","update-time":"13:00"}"#)
                .is_err()
        );
    }
}
