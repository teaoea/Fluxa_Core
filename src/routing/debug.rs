//! 临时调试规则仅通过本地 API 操作，不实现 Deserialize，也不进入配置或持久化。
//! 认证及授权属于宿主控制入口；此模块不创建网络或 IPC 服务。
use super::runtime::{Origin, RuleClass};
use super::{Decision, Match, Matcher, Policy, Reason, Rule, Target, Transport, matcher};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const MAX_DEBUG_RULES: usize = 256;
const MAX_DEBUG_SESSIONS: usize = 64;
pub const MAX_LIFETIME_SECONDS: u64 = 3600;
static SESSIONS: ByteBudget = ByteBudget::new(MAX_DEBUG_SESSIONS);
/// 生命周期必填，单位秒；Option 仅用于在 API 边界明确拒绝缺失值。
#[derive(Clone, Debug)]
pub struct DebugRule {
    pub rule: Rule,
    pub protocol: Option<Transport>,
    pub lifetime_seconds: Option<u64>,
}
struct Entry {
    matcher: Matcher,
    protocol: Option<Transport>,
    policy: Policy,
    outbound: Option<String>,
    deadline: Instant,
    expires_at: u64,
    sequence: usize,
}
struct Compiled {
    entries: RwLock<Vec<Entry>>,
    version: u64,
    _memory: ByteLease,
}
#[derive(Default)]
struct State {
    closed: bool,
    active: Option<Arc<Compiled>>,
}
pub struct Session {
    state: RwLock<State>,
    _lease: ByteLease,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub active: bool,
    pub count: usize,
    pub version: Option<u64>,
    pub next_expiration: Option<u64>,
}
fn discard_expired(entries: &mut Vec<Entry>, now: Instant) -> Result<()> {
    let unix_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Internal)?
        .as_secs();
    // 过期时间用于绝对截止，单调截止防止时钟回拨延长生命周期；任一到期就丢弃。
    entries.retain(|entry| entry.deadline > now && entry.expires_at > unix_now);
    Ok(())
}
impl Session {
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: RwLock::new(State::default()),
            _lease: SESSIONS.reserve(1)?,
        })
    }
    /// 所有规则成功后一次发布。宿主必须先完成调试权限检查，不能从配置调用。
    pub fn replace(&self, rules: Vec<DebugRule>, outbound: Option<String>) -> Result<Status> {
        self.replace_at(rules, outbound, Instant::now())
    }
    fn replace_at(
        &self,
        rules: Vec<DebugRule>,
        outbound: Option<String>,
        now: Instant,
    ) -> Result<Status> {
        if rules.len() > MAX_DEBUG_RULES {
            return Err(Error::ResourceLimit);
        }
        let memory = super::runtime::reserve_debug(rules.iter().map(|r| &r.rule))?;
        let unix_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Internal)?
            .as_secs();
        let mut entries = Vec::with_capacity(rules.len());
        for (sequence, row) in rules.into_iter().enumerate() {
            let seconds = row
                .lifetime_seconds
                .filter(|v| (1..=MAX_LIFETIME_SECONDS).contains(v))
                .ok_or(Error::DebugInvalidLifetime(row.lifetime_seconds))?;
            let expires_at = unix_now.checked_add(seconds).ok_or(Error::InvalidInput)?;
            let deadline = now
                .checked_add(Duration::from_secs(seconds))
                .ok_or(Error::InvalidInput)?;
            if row.protocol.is_some()
                && !matches!(
                    row.rule.r#type,
                    Match::Domain | Match::DomainSuffix | Match::DomainKeyword
                )
            {
                return Err(Error::InvalidInput);
            }
            if row.rule.r#type == Match::Transport && row.rule.value == "quic" {
                return Err(Error::Unsupported);
            }
            let rule = super::runtime::normalize_rule(&row.rule, &outbound)?;
            entries.push(Entry {
                matcher: matcher(rule.r#type, &rule.value)?,
                protocol: row.protocol,
                policy: rule.policy,
                outbound: rule.outbound,
                deadline,
                expires_at,
                sequence,
            });
        }
        let version = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let compiled = Arc::new(Compiled {
            entries: RwLock::new(entries),
            version,
            _memory: memory,
        });
        let mut state = self.state.write().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::DebugSessionClosed);
        }
        state.active = if compiled
            .entries
            .read()
            .map_err(|_| Error::Internal)?
            .is_empty()
        {
            None
        } else {
            Some(Arc::clone(&compiled))
        };
        Self::status_of(&state, now)
    }
    pub fn clear(&self) -> Result<()> {
        let mut state = self.state.write().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::DebugSessionClosed);
        }
        state.active = None;
        Ok(())
    }
    pub fn close(&self) -> Result<()> {
        let mut state = self.state.write().map_err(|_| Error::Internal)?;
        state.closed = true;
        state.active = None;
        Ok(())
    }
    pub fn status(&self) -> Result<Status> {
        let mut state = self.state.write().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::DebugSessionClosed);
        }
        let result = Self::status_of(&state, Instant::now())?;
        // 到期即不再匹配；下一次 API 访问释放会话持有的全过期方案。
        if !result.active {
            state.active = None;
        }
        Ok(result)
    }
    fn status_of(state: &State, now: Instant) -> Result<Status> {
        let Some(active) = &state.active else {
            return Ok(Status {
                active: false,
                count: 0,
                version: None,
                next_expiration: None,
            });
        };
        // 每次访问丢弃过期条目；锁损坏必须报错，不能伪装成未启用。
        let mut entries = active.entries.write().map_err(|_| Error::Internal)?;
        discard_expired(&mut entries, now)?;
        let count = entries.len();
        Ok(Status {
            active: count > 0,
            count,
            version: (count > 0).then_some(active.version),
            next_expiration: entries.iter().map(|e| e.expires_at).min(),
        })
    }
    pub(super) fn view(&self) -> Result<View> {
        let mut state = self.state.write().map_err(|_| Error::Internal)?;
        if !Self::status_of(&state, Instant::now())?.active {
            state.active = None;
        }
        Ok(View(if state.closed {
            None
        } else {
            state.active.clone()
        }))
    }
}
/// 视图不延长规则有效期，旧视图每次决策同样检查单调时钟截止时间。
pub(super) struct View(Option<Arc<Compiled>>);
impl View {
    fn applicable(entry: &Entry, target: &Target, now: Instant) -> bool {
        entry.deadline > now && entry.protocol.is_none_or(|p| p == target.transport)
    }
    fn domain_match(entry: &Entry, name: Option<&str>) -> bool {
        match &entry.matcher {
            Matcher::Domain(value, suffix) => name.is_some_and(|name| {
                name == value
                    || (*suffix && name.strip_suffix(value).is_some_and(|p| p.ends_with('.')))
            }),
            Matcher::Keyword(value) => name.is_some_and(|name| name.contains(value)),
            _ => false,
        }
    }
    pub(super) fn before_dns(&self, target: &Target) -> Result<super::DebugPreDecision> {
        let Some(active) = &self.0 else {
            return Ok(super::DebugPreDecision::Continue);
        };
        let now = Instant::now();
        let (name, address) = super::runtime::Snapshot::metadata(target)?;
        let mut entries = active.entries.write().map_err(|_| Error::Internal)?;
        discard_expired(&mut entries, now)?;
        let domain_hit = entries
            .iter()
            .any(|e| Self::applicable(e, target, now) && Self::domain_match(e, name.as_deref()));
        if !domain_hit
            && address.is_none()
            && entries.iter().any(|e| {
                Self::applicable(e, target, now) && matches!(e.matcher, Matcher::Network(_))
            })
        {
            return Ok(super::DebugPreDecision::WaitForDns);
        }
        drop(entries);
        Ok(match self.evaluate_at(target, now)? {
            Some(d) => super::DebugPreDecision::Decided(d),
            None => super::DebugPreDecision::Continue,
        })
    }
    pub(super) fn evaluate(&self, target: &Target) -> Result<Option<Decision>> {
        self.evaluate_at(target, Instant::now())
    }
    fn evaluate_at(&self, target: &Target, now: Instant) -> Result<Option<Decision>> {
        let Some(active) = &self.0 else {
            return Ok(None);
        };
        let (name, address) = super::runtime::Snapshot::metadata(target)?;
        let mut entries = active.entries.write().map_err(|_| Error::Internal)?;
        discard_expired(&mut entries, now)?;
        let domain_hit = entries
            .iter()
            .any(|e| Self::applicable(e, target, now) && Self::domain_match(e, name.as_deref()));
        let winner = entries
            .iter()
            .filter(|e| Self::applicable(e, target, now))
            .filter(|e| match &e.matcher {
                Matcher::Domain(..) | Matcher::Keyword(_) => Self::domain_match(e, name.as_deref()),
                Matcher::Network(n) => !domain_hit && address.is_some_and(|ip| n.includes(ip)),
                Matcher::Port(first, last) => (*first..=*last).contains(&target.port),
                Matcher::Transport(p) => *p == target.transport,
                Matcher::Quic => false,
            })
            .min_by_key(|e| {
                let class = match e.matcher {
                    Matcher::Network(_) => RuleClass::LocalIp,
                    Matcher::Domain(_, false) => RuleClass::LocalDomain,
                    Matcher::Domain(_, true) => RuleClass::LocalDomainSuffix,
                    Matcher::Keyword(_) => RuleClass::LocalKeyword,
                    _ => RuleClass::LocalPortTransport,
                };
                let order = [
                    RuleClass::LocalIp,
                    RuleClass::LocalDomain,
                    RuleClass::LocalDomainSuffix,
                    RuleClass::LocalKeyword,
                    RuleClass::LocalPortTransport,
                ];
                let rank = order.iter().position(|v| *v == class);
                (
                    e.policy != Policy::Reject,
                    e.protocol.is_none(),
                    rank,
                    e.sequence,
                )
            });
        Ok(winner.map(|e| Decision {
            policy: e.policy,
            outbound: e.outbound.clone(),
            rule_index: Some(e.sequence),
            reason: Reason::DebugRule,
            rule_version: Some(active.version),
            origin: Some(Origin {
                source: 0,
                sequence: e.sequence as u32,
            }),
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn individual_deadlines_expire_even_in_retained_views() {
        let now = Instant::now();
        let session = Session::new().unwrap();
        let rule = |lifetime, policy| DebugRule {
            rule: Rule {
                r#type: Match::Domain,
                value: "example.com".into(),
                policy,
                outbound: None,
            },
            protocol: None,
            lifetime_seconds: lifetime,
        };
        for missing in [None, Some(0), Some(MAX_LIFETIME_SECONDS + 1)] {
            assert!(
                session
                    .replace_at(vec![rule(missing, Policy::Direct)], None, now)
                    .is_err()
            );
        }
        session
            .replace_at(
                vec![
                    rule(Some(30), Policy::Reject),
                    rule(Some(60), Policy::Direct),
                ],
                None,
                now,
            )
            .unwrap();
        let view = session.view().unwrap();
        let target = Target {
            domain: Some("example.com".into()),
            address: None,
            port: 443,
            transport: Transport::Tcp,
        };
        assert_eq!(
            view.evaluate_at(&target, now).unwrap().unwrap().policy,
            Policy::Reject
        );
        assert_eq!(
            view.evaluate_at(&target, now + Duration::from_secs(30))
                .unwrap()
                .unwrap()
                .policy,
            Policy::Direct
        );
        assert!(
            view.evaluate_at(&target, now + Duration::from_secs(60))
                .unwrap()
                .is_none()
        );
    }
}
