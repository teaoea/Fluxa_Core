//! 平台结果的内核校验契约。生产尚无证据生产者，缺证据只能报告不可用。
use crate::{
    configuration::models::IpStrategy,
    error::{Error, Result},
    lifecycle::session::{Authority, Identity},
};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Serialize;
use std::{
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};
const MAX_EGRESS: usize = 32;
const VALIDITY: Duration = Duration::from_secs(30);

/// 只读平台类别；不接收字符串声明来认证平台。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Macos,
    Ios,
    Tvos,
    Android,
    Windows,
    Linux,
}
impl Platform {
    fn current() -> Result<Self> {
        match std::env::consts::OS {
            "macos" => Ok(Self::Macos),
            "ios" => Ok(Self::Ios),
            "tvos" => Ok(Self::Tvos),
            "android" => Ok(Self::Android),
            "windows" => Ok(Self::Windows),
            "linux" => Ok(Self::Linux),
            _ => Err(Error::Unsupported),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckState {
    NotRun,
    Passed,
    Unavailable,
    Rejected,
}
/// 细分诊断不是另一套错误码；Result 仍使用 error.rs 的现有权威错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    MissingAdapter,
    Cancelled,
    Consumed,
    Expired,
    WrongRequest,
    WrongPlatform,
    SourceRevoked,
    SourceFailure,
    InvalidObservation,
    PermissionDenied,
    PermissionUndetermined,
    MissingCapability,
    EgressUnresolved,
    MissingLoopGuard,
    InvalidRouteCoverage,
}
/// 可复制的摘要不包含挑战、对端身份、地址或租约，也不能提交回内核充当授权。
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Assessment {
    pub transaction: u64,
    pub platform: Platform,
    pub permission: CheckState,
    pub system_capability: CheckState,
    pub loop_protection: CheckState,
    pub reason: Option<Reason>,
}
impl Assessment {
    fn pending(transaction: u64, platform: Platform) -> Self {
        Self {
            transaction,
            platform,
            permission: CheckState::NotRun,
            system_capability: CheckState::NotRun,
            loop_protection: CheckState::NotRun,
            reason: None,
        }
    }
    fn unavailable(mut self, reason: Reason) -> Self {
        self.permission = CheckState::Unavailable;
        self.system_capability = CheckState::Unavailable;
        self.loop_protection = CheckState::Unavailable;
        self.reason = Some(reason);
        self
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Requirements {
    ipv4: bool,
    ipv6: bool,
    dns: bool,
}
/// 证据不实现Deserialize/Clone，无公开构造器；当前仅子模块测试能创建。
/// 将来仅可由验证真实OS能力通道的原生边界生成，不能添加普通JSON接收入口。
struct VerifiedEvidence {
    transaction: u64,
    challenge: [u8; 32],
    platform: Platform,
    observed: Instant,
    owner: Arc<Authority>,
    identity: Identity,
    permission: Permission,
    capabilities: Capabilities,
    egress_scope: u64,
    routes: [Option<RouteWitness>; MAX_EGRESS],
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Permission {
    Granted,
    Denied,
    Undetermined,
}
#[derive(Clone, Copy)]
struct Capabilities {
    ip_packets: bool,
    ipv4: bool,
    ipv6: bool,
    system_dns: bool,
}
#[derive(Clone, Copy)]
struct RouteWitness {
    destination: IpAddr,
    interface: u32,
    protected_scope: u64,
}
/// 固定容量、单次请求。当前调用链没有本地授权、系统修改或完整证据缓存。
pub(super) struct Request {
    transaction: u64,
    challenge: [u8; 32],
    platform: Platform,
    created: Instant,
    expires: Instant,
    requirements: Requirements,
    egress: Option<[Option<IpAddr>; MAX_EGRESS]>,
    consumed: bool,
}
impl Request {
    pub(super) fn new(
        transaction: u64,
        strategy: IpStrategy,
        dns: bool,
        egress: Option<&[IpAddr]>,
        now: Instant,
    ) -> Result<Self> {
        if transaction == 0 {
            return Err(Error::InvalidInput);
        }
        if egress.is_some_and(|v| v.len() > MAX_EGRESS) {
            return Err(Error::ResourceLimit);
        }
        let mut endpoints = egress.map(|_| [None; MAX_EGRESS]);
        if let (Some(input), Some(output)) = (egress, endpoints.as_mut()) {
            for (index, address) in input.iter().enumerate() {
                if address.is_unspecified()
                    || address.is_multicast()
                    || input[..index].contains(address)
                {
                    return Err(Error::InvalidInput);
                }
                output[index] = Some(*address);
            }
        }
        let mut challenge = [0u8; 32];
        SystemRandom::new()
            .fill(&mut challenge)
            .map_err(|_| Error::Internal)?;
        let requirements = Requirements {
            ipv4: !matches!(strategy, IpStrategy::OnlyIpv6),
            ipv6: !matches!(strategy, IpStrategy::OnlyIpv4),
            dns,
        };
        Ok(Self {
            transaction,
            challenge,
            platform: Platform::current()?,
            created: now,
            expires: now.checked_add(VALIDITY).ok_or(Error::InvalidInput)?,
            requirements,
            egress: endpoints,
            consumed: false,
        })
    }
    /// 无平台生产者时也通过同一拒绝路径；不把空的出站清单当作防回环证明。
    pub(super) fn unavailable(&mut self, now: Instant, cancelled: bool) -> Assessment {
        self.assess(None, now, cancelled)
    }
    fn assess(
        &mut self,
        evidence: Option<VerifiedEvidence>,
        now: Instant,
        cancelled: bool,
    ) -> Assessment {
        let summary = Assessment::pending(self.transaction, self.platform);
        if self.consumed {
            return summary.unavailable(Reason::Consumed);
        }
        self.consumed = true;
        if cancelled {
            return summary.unavailable(Reason::Cancelled);
        }
        if now < self.created || now >= self.expires {
            return summary.unavailable(Reason::Expired);
        }
        let Some(evidence) = evidence else {
            return summary.unavailable(Reason::MissingAdapter);
        };
        if evidence.transaction != self.transaction || evidence.challenge != self.challenge {
            return summary.unavailable(Reason::WrongRequest);
        }
        if evidence.platform != self.platform {
            return summary.unavailable(Reason::WrongPlatform);
        }
        if evidence.observed < self.created || evidence.observed > now {
            return summary.unavailable(Reason::InvalidObservation);
        }
        // 校验与源身份撤销共享权限锁；摘要从不恢复已撤销的能力。
        match evidence
            .owner
            .with_active(evidence.identity, || Ok(self.validate(&evidence, summary)))
        {
            Ok(result) => result,
            Err(Error::InvalidState) => summary.unavailable(Reason::SourceRevoked),
            Err(_) => summary.unavailable(Reason::SourceFailure),
        }
    }
    fn validate(&self, evidence: &VerifiedEvidence, mut result: Assessment) -> Assessment {
        if evidence.permission == Permission::Denied {
            result.permission = CheckState::Rejected;
            result.reason = Some(Reason::PermissionDenied);
            return result;
        }
        if evidence.permission == Permission::Undetermined {
            result.permission = CheckState::Unavailable;
            result.reason = Some(Reason::PermissionUndetermined);
            return result;
        }
        if evidence.permission != Permission::Granted {
            result.reason = Some(Reason::InvalidObservation);
            return result;
        }
        result.permission = CheckState::Passed;
        let caps = evidence.capabilities;
        if !caps.ip_packets
            || (self.requirements.ipv4 && !caps.ipv4)
            || (self.requirements.ipv6 && !caps.ipv6)
            || (self.requirements.dns && !caps.system_dns)
        {
            result.system_capability = CheckState::Rejected;
            result.reason = Some(Reason::MissingCapability);
            return result;
        }
        result.system_capability = CheckState::Passed;
        let Some(expected) = self.egress else {
            result.loop_protection = CheckState::Unavailable;
            result.reason = Some(Reason::EgressUnresolved);
            return result;
        };
        if evidence.egress_scope == 0 {
            result.loop_protection = CheckState::Rejected;
            result.reason = Some(Reason::MissingLoopGuard);
            return result;
        }
        let routes = &evidence.routes;
        let mut valid = true;
        for (index, witness) in routes.iter().enumerate() {
            if let Some(witness) = witness {
                valid &= witness.interface != 0
                    && witness.protected_scope == evidence.egress_scope
                    && expected.contains(&Some(witness.destination))
                    && !routes[..index]
                        .iter()
                        .flatten()
                        .any(|old| old.destination == witness.destination);
            }
        }
        for destination in expected.into_iter().flatten() {
            valid &= routes
                .iter()
                .flatten()
                .any(|w| w.destination == destination);
        }
        result.loop_protection = if valid {
            CheckState::Passed
        } else {
            CheckState::Rejected
        };
        result.reason = (!valid).then_some(Reason::InvalidRouteCoverage);
        result
    }
}
#[cfg(test)]
#[path = "../../tests/test_vpn_platform_check.rs"]
mod test_vpn_platform_check;
