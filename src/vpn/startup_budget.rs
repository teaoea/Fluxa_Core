//! 启动预算的统一峰值核算；未具备完整运行计划和可信系统余量时明确拒绝。
use crate::error::{Error, Result};
use serde::Serialize;
use std::time::{Duration, Instant};

/// 只读预算诊断，不是配额租约或启动授权。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    IncompletePlan,
    MissingSystemObservation,
    InvalidObservation,
    ExpiredObservation,
    ArithmeticOverflow,
    ProcessMemoryLimit,
    ProcessDescriptorLimit,
    SystemMemoryHeadroom,
    SystemDescriptorHeadroom,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Usage {
    pub memory_bytes: u64,
    pub file_descriptors: u64,
}
impl Usage {
    fn add(self, other: Self) -> Option<Self> {
        Some(Self {
            memory_bytes: self.memory_bytes.checked_add(other.memory_bytes)?,
            file_descriptors: self.file_descriptors.checked_add(other.file_descriptors)?,
        })
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Assessment {
    pub transaction: u64,
    pub peak: Option<Usage>,
    pub additional: Option<Usage>,
    pub reason: Option<Reason>,
    /// 仅表示本次预算算术及观测满足要求，绝不等价于 VPN 接管许可。
    pub budget_satisfied: bool,
}
/// 必须是内核完整运行计划：None表示未完成核算，Some(0)才表示已确认无需资源。
/// 四部分互斥计费；retained包含仍实际存活的旧快照，不以逻辑关闭扣除。
/// buffers/temporary的内存及FD不得再次计入candidate或retained。
pub(super) struct Plan {
    pub(super) retained: Option<Usage>,
    pub(super) candidate: Option<Usage>,
    pub(super) buffers: Option<Usage>,
    pub(super) temporary: Option<Usage>,
    pub(super) safety_margin: Usage,
    pub(super) process_limit: Usage,
}
impl Plan {
    pub(super) fn incomplete() -> Self {
        Self {
            retained: None,
            candidate: None,
            buffers: None,
            temporary: None,
            safety_margin: Usage {
                memory_bytes: 0,
                file_descriptors: 0,
            },
            process_limit: Usage {
                memory_bytes: 0,
                file_descriptors: 0,
            },
        }
    }
    fn totals(&self) -> std::result::Result<(Usage, Usage), Reason> {
        let (Some(retained), Some(candidate), Some(buffers), Some(temporary)) =
            (self.retained, self.candidate, self.buffers, self.temporary)
        else {
            return Err(Reason::IncompletePlan);
        };
        let additional = candidate
            .add(buffers)
            .and_then(|v| v.add(temporary))
            .and_then(|v| v.add(self.safety_margin))
            .ok_or(Reason::ArithmeticOverflow)?;
        let peak = retained.add(additional).ok_or(Reason::ArithmeticOverflow)?;
        Ok((peak, additional))
    }
}
/// 无Deserialize、Clone或公开构造器。生产尚无可信OS余量采集器；不能由JSON/配置填写。
/// available是当前可追加余量，已扣除系统/进程现有占用，不能拿总容量冒充空闲。
pub(super) struct VerifiedHeadroom {
    transaction: u64,
    observed: Instant,
    available: Usage,
}
/// 预算检查只对同一事务的一次计划有效；不提供可重放的成功票据。
pub(super) struct Request {
    transaction: u64,
    created: Instant,
    consumed: bool,
    plan: Plan,
}
impl Request {
    pub(super) fn new(transaction: u64, plan: Plan, now: Instant) -> Result<Self> {
        if transaction == 0 {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            transaction,
            created: now,
            consumed: false,
            plan,
        })
    }
    pub(super) fn assess(
        &mut self,
        observation: Option<VerifiedHeadroom>,
        now: Instant,
        cancelled: bool,
    ) -> Result<Assessment> {
        if self.consumed {
            return Err(Error::InvalidState);
        }
        self.consumed = true;
        let mut result = Assessment {
            transaction: self.transaction,
            peak: None,
            additional: None,
            reason: None,
            budget_satisfied: false,
        };
        let evaluate = || -> std::result::Result<(Usage, Usage), Reason> {
            if cancelled {
                return Err(Reason::Cancelled);
            }
            if now < self.created {
                return Err(Reason::InvalidObservation);
            }
            let (peak, additional) = self.plan.totals()?;
            if peak.memory_bytes > self.plan.process_limit.memory_bytes {
                return Err(Reason::ProcessMemoryLimit);
            }
            if peak.file_descriptors > self.plan.process_limit.file_descriptors {
                return Err(Reason::ProcessDescriptorLimit);
            }
            let observation = observation.ok_or(Reason::MissingSystemObservation)?;
            if observation.transaction != self.transaction
                || observation.observed < self.created
                || observation.observed > now
            {
                return Err(Reason::InvalidObservation);
            }
            if now.duration_since(observation.observed) >= Duration::from_secs(5) {
                return Err(Reason::ExpiredObservation);
            }
            if additional.memory_bytes > observation.available.memory_bytes {
                return Err(Reason::SystemMemoryHeadroom);
            }
            if additional.file_descriptors > observation.available.file_descriptors {
                return Err(Reason::SystemDescriptorHeadroom);
            }
            Ok((peak, additional))
        };
        // 只在完整计划成功计算后发布总额；失败原因不含路径、配置正文和节点凭据。
        if let Ok((peak, additional)) = self.plan.totals() {
            result.peak = Some(peak);
            result.additional = Some(additional);
        }
        match evaluate() {
            Ok(_) => result.budget_satisfied = true,
            Err(reason) => result.reason = Some(reason),
        }
        Ok(result)
    }
}
#[cfg(test)]
#[path = "../../tests/test_vpn_startup_budget.rs"]
mod test_vpn_startup_budget;
