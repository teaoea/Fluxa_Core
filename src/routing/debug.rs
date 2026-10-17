//! 临时调试规则仅通过本地 API 操作，不实现 Deserialize，也不进入配置或持久化。
//! 认证、授权与审计由专用本地 IPC 完成，禁止普通业务接口创建调试会话。
use super::runtime::{Origin, TypePriority};
use super::{Decision, Match, Matcher, Policy, Reason, Rule, Target, Transport, matcher};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
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
    revoked: AtomicBool,
    revocation: tokio::sync::watch::Sender<bool>,
    entries: RwLock<Vec<Entry>>,
    version: u64,
    _memory: ByteLease,
}
#[derive(Default)]
struct State {
    closed: bool,
    active: Option<Arc<Compiled>>,
}
pub(crate) struct Session {
    id: u64,
    state: RwLock<State>,
    _lease: ByteLease,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    pub(crate) fn identity(&self) -> u64 {
        self.id
    }

    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            id: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            state: RwLock::new(State::default()),
            _lease: SESSIONS.reserve(1)?,
        })
    }
    /// 专用 IPC 逐操作授权成功后才能调用；全部规则成功后一次发布。
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
            // Debug 的协议字段只由专用 IPC 层提供，禁止嵌套第二个条件伪造优先级。
            if row.rule.protocol.is_some() {
                return Err(Error::InvalidInput);
            }
            if row.protocol.is_some()
                && !matches!(
                    row.rule.r#type,
                    Match::Domain | Match::DomainSuffix | Match::DomainKeyword
                )
            {
                return Err(Error::InvalidInput);
            }
            if row.rule.r#type == Match::ProcessName
                || row.rule.r#type == Match::GeoIp
                || row.rule.r#type == Match::DomainQuic
                || (row.rule.r#type == Match::Transport && row.rule.value == "quic")
            {
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
            revoked: AtomicBool::new(false),
            revocation: tokio::sync::watch::channel(false).0,
            entries: RwLock::new(entries),
            version,
            _memory: memory,
        });
        let mut state = self.state.write().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::DebugSessionClosed);
        }
        if let Some(old) = &state.active {
            old.revoked.store(true, Ordering::Release);
            old.revocation.send_replace(true);
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
        if let Some(old) = state.active.take() {
            old.revoked.store(true, Ordering::Release);
            old.revocation.send_replace(true);
        }
        Ok(())
    }
    pub fn close(&self) -> Result<()> {
        let mut state = self.state.write().map_err(|_| Error::Internal)?;
        state.closed = true;
        if let Some(old) = state.active.take() {
            old.revoked.store(true, Ordering::Release);
            old.revocation.send_replace(true);
        }
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
#[derive(Clone)]
pub(super) struct View(Option<Arc<Compiled>>);
impl View {
    pub(super) fn revoked(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|active| active.revoked.load(Ordering::Acquire))
    }
    pub(super) async fn changed(&self) -> Result<()> {
        let Some(active) = &self.0 else {
            return std::future::pending().await;
        };
        let mut revoked = active.revocation.subscribe();
        if *revoked.borrow() {
            return Ok(());
        }
        let deadline = {
            let entries = active.entries.read().map_err(|_| Error::Internal)?;
            entries.iter().map(|entry| entry.deadline).min()
        };
        let next = deadline
            .map(|deadline| deadline.min(Instant::now() + Duration::from_secs(1)))
            .unwrap_or_else(Instant::now);
        tokio::select! {biased; _=revoked.changed()=>(),_=tokio::time::sleep_until(tokio::time::Instant::from_std(next))=>()}
        Ok(())
    }

    fn applicable(entry: &Entry, target: &Target, now: Instant) -> bool {
        entry.deadline > now
            && entry.protocol.is_none_or(|p| p == target.transport)
            && !matches!(&entry.matcher, Matcher::DomainPort(_, port) if *port != target.port)
    }
    fn domain_match(entry: &Entry, name: Option<&str>) -> bool {
        match &entry.matcher {
            Matcher::DomainPort(value, _) => name.is_some_and(|name| {
                name == value || name.strip_suffix(value).is_some_and(|p| p.ends_with('.'))
            }),
            Matcher::Domain(value, suffix) => name.is_some_and(|name| {
                name == value
                    || (*suffix && name.strip_suffix(value).is_some_and(|p| p.ends_with('.')))
            }),
            Matcher::Keyword(value) => name
                .and_then(super::keyword_subject)
                .is_some_and(|label| label.contains(value)),
            _ => false,
        }
    }
    pub(super) fn before_dns(&self, target: &Target) -> Result<super::DebugPreDecision> {
        let Some(active) = &self.0 else {
            return Ok(super::DebugPreDecision::Continue);
        };
        if active.revoked.load(Ordering::Acquire) {
            return Ok(super::DebugPreDecision::Continue);
        }
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
        if active.revoked.load(Ordering::Acquire) {
            return Ok(None);
        }
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
                Matcher::Domain(..) | Matcher::Keyword(_) | Matcher::DomainPort(..) => {
                    Self::domain_match(e, name.as_deref())
                }
                Matcher::Network(n) => !domain_hit && address.is_some_and(|ip| n.includes(ip)),
                Matcher::Port(first, last) => (*first..=*last).contains(&target.port),
                Matcher::Transport(p) => *p == target.transport,
                Matcher::Quic
                | Matcher::DomainQuic(_)
                | Matcher::GeoIp(_)
                | Matcher::ProcessName(_) => false,
            })
            .min_by_key(|e| {
                let priority = if e.protocol.is_some() {
                    TypePriority::Protocol
                } else {
                    match e.matcher {
                        Matcher::Transport(_) | Matcher::Quic | Matcher::DomainQuic(_) => {
                            TypePriority::Protocol
                        }
                        Matcher::Port(..) | Matcher::DomainPort(..) => TypePriority::Port,
                        Matcher::Domain(_, false) => TypePriority::Domain,
                        Matcher::Domain(_, true) => TypePriority::Suffix,
                        Matcher::Keyword(_) => TypePriority::Keyword,
                        Matcher::Network(_) => TypePriority::Ip,
                        Matcher::GeoIp(_) => TypePriority::GeoIp,
                        Matcher::ProcessName(_) => TypePriority::Process,
                    }
                };
                // Overlay 已先于正式规则求值；内部不再给 REJECT 另开越级通道。
                (priority, e.sequence)
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

/// 独立 Unix IPC 控制入口；不注册普通业务 JSON 操作。
#[cfg(unix)]
pub mod ipc;

#[cfg(test)]
#[path = "../../tests/test_routing_debug.rs"]
mod test_routing_debug;
