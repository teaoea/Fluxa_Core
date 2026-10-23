//! 四阶段事务驱动。仅可信的内核 Rust 实现提供步骤，不接收配置/IPC 的成功标志。
//! 平台接线与必要网络检查未接入前，不从 FFI 暴露本驱动或宣称系统 VPN 可用。
use crate::error::{Error, Result};
// 系统接管槽由内核全局持有，前端对象释放不能遗失回滚失败的恢复入口。
static ACTIVE: Mutex<Option<Arc<Inner>>> = Mutex::new(None);
mod trusted {
    pub trait Sealed {}
}
use serde::Serialize;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Notify, watch};

/// 系统是否仍可能保留本事务的变更；未知不能冒充已恢复。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemState {
    Unchanged,
    MayBeChanged,
    Restored,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Preflight,
    Prepare,
    Commit,
    Verify,
    Running,
    RollingBack,
    Failed,
    RollbackFailed,
    Stopped,
}
/// 原因和清理错误分离；不包含配置、凭据或平台原状态。
#[derive(Clone, Debug)]
pub struct Status {
    pub transaction: u64,
    pub phase: Phase,
    pub system: SystemState,
    pub failure: Option<Error>,
    pub failure_phase: Option<Phase>,
    pub cleanup_failure: Option<Error>,
}
/// 阶段观察取消信号；期限由驱动监督，收到停止请求须返回确定结果。
/// 驱动不会直接丢弃含系统副作用的 Future，也不会将取消请求当成回滚完成。
#[derive(Clone)]
pub struct Cancellation {
    signal: watch::Receiver<bool>,
}
impl Cancellation {
    pub fn requested(&self) -> bool {
        *self.signal.borrow()
    }
    pub async fn notified(&mut self) {
        if self.requested() {
            return;
        }
        let _ = self.signal.changed().await;
    }
}
pub type Step<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
/// 此 trait 已密封，外部 Rust/FFI 调用方不能实现或注册驱动；当前仅测试实现。
/// 此接口只供可信 Rust 内核/平台组合实现，不是 App 的检查结果提交接口。
/// preflight/prepare 内核实现必须完成规范全部检查、证据绑定及授权复核。
/// commit 前登记逆向恢复动作，出错/取消仍须保留已发生副作用；rollback 必须幂等，
/// 只恢复本事务仍拥有的系统状态，并在确认释放完毕后才返回成功。
/// 所有方法均须有界且响应取消；不能用本 trait 的测试替身注册生产 FFI。
pub trait Steps: trusted::Sealed + Send + Sync + 'static {
    fn preflight(&self, transaction: u64, cancel: Cancellation) -> Step<'_>;
    fn prepare(&self, transaction: u64, cancel: Cancellation) -> Step<'_>;
    fn commit(&self, transaction: u64, cancel: Cancellation) -> Step<'_>;
    fn verify(&self, transaction: u64, cancel: Cancellation) -> Step<'_>;
    fn rollback(&self, transaction: u64) -> Step<'_>;
}
struct State {
    status: Status,
    cancelled: bool,
    finished: bool,
    recovering: bool,
}
struct Inner {
    state: Mutex<State>,
    cancel: watch::Sender<bool>,
    changed: Notify,
    steps: Mutex<Option<Arc<dyn Steps>>>,
}
/// 单个槽保持整个运行及恢复生命周期；重复启动不能替换仍活动或恢复失败的事务。
/// 只读状态和取消意图不授予外部调用者推进阶段或发布 Running 的能力。
pub struct Controller {
    owned: Mutex<Option<u64>>,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            owned: Mutex::new(None),
        }
    }
}
impl Controller {
    /// 每阶段最多120秒；期限触发取消后仍等待步骤确定停止，不能脱离回滚。
    /// 返回的是开始受理，不是成功接管；正常运行时调用 stop 并等待完成。
    pub fn begin(&self, steps: Arc<dyn Steps>, deadline: Duration) -> Result<u64> {
        if deadline.is_zero() || deadline > Duration::from_secs(120) {
            return Err(Error::InvalidInput);
        }
        let runtime = crate::lifecycle::runtime()?;
        let mut slot = ACTIVE.lock().map_err(|_| Error::Internal)?;
        if let Some(previous) = slot.as_ref() {
            let state = previous.state.lock().map_err(|_| Error::Internal)?;
            if !state.finished || state.status.phase == Phase::RollbackFailed {
                return Err(Error::InvalidState);
            }
        }
        let transaction = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let (cancel, _) = watch::channel(false);
        let inner = Arc::new(Inner {
            state: Mutex::new(State {
                status: Status {
                    transaction,
                    phase: Phase::Preflight,
                    system: SystemState::Unchanged,
                    failure: None,
                    failure_phase: None,
                    cleanup_failure: None,
                },
                cancelled: false,
                finished: false,
                recovering: false,
            }),
            cancel,
            changed: Notify::new(),
            steps: Mutex::new(Some(steps)),
        });
        *self.owned.lock().map_err(|_| Error::Internal)? = Some(transaction);
        *slot = Some(inner.clone());
        runtime.spawn(async move {
            inner.run(deadline).await;
        });
        Ok(transaction)
    }
    fn get(&self, transaction: u64) -> Result<Arc<Inner>> {
        let slot = ACTIVE.lock().map_err(|_| Error::Internal)?;
        let inner = slot.as_ref().ok_or(Error::InvalidState)?;
        if inner
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .status
            .transaction
            != transaction
        {
            return Err(Error::InvalidState);
        }
        Ok(inner.clone())
    }
    /// 控制入口重建后取得内核仍保留的事务摘要；不能据此跳过重新授权。
    pub fn current_status(&self) -> Result<Option<Status>> {
        let slot = ACTIVE.lock().map_err(|_| Error::Internal)?;
        slot.as_ref()
            .map(|inner| {
                Ok(inner
                    .state
                    .lock()
                    .map_err(|_| Error::Internal)?
                    .status
                    .clone())
            })
            .transpose()
    }
    pub fn status(&self, transaction: u64) -> Result<Status> {
        Ok(self
            .get(transaction)?
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .status
            .clone())
    }
    /// 取消与阶段准入使用同一锁。重复停止等待同一个清理结果，失败不会自动重试。
    pub async fn stop(&self, transaction: u64) -> Result<()> {
        let inner = self.get(transaction)?;
        inner.request_cancel()?;
        inner.wait_finished().await
    }
    /// 显式重试失败的清理；失败时槽及驱动保留，不冒称已恢复。
    pub async fn retry_cleanup(&self, transaction: u64) -> Result<()> {
        let runtime = crate::lifecycle::runtime()?;
        let inner = self.get(transaction)?;
        let launch = {
            let mut state = inner.state.lock().map_err(|_| Error::Internal)?;
            if state.recovering {
                false
            } else {
                if state.status.phase != Phase::RollbackFailed {
                    return Err(Error::InvalidState);
                }
                state.recovering = true;
                state.finished = false;
                state.status.phase = Phase::RollingBack;
                true
            }
        };
        if launch {
            let work = inner.clone();
            runtime.spawn(async move {
                work.cleanup().await;
            });
        }
        inner.wait_finished().await
    }
}
impl Inner {
    fn request_cancel(&self) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if !state.finished {
            state.cancelled = true;
            self.cancel.send_replace(true);
        }
        Ok(())
    }
    fn enter(&self, phase: Phase) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.cancelled {
            return Err(Error::Cancelled);
        }
        state.status.phase = phase;
        if phase == Phase::Commit {
            state.status.system = SystemState::MayBeChanged;
        }
        self.changed.notify_waiters();
        Ok(())
    }
    async fn run(self: Arc<Self>, deadline: Duration) {
        let result = self.stages(deadline).await;
        if let Err(error) = result {
            match self.state.lock() {
                Ok(mut state) => {
                    state.status.failure_phase = Some(state.status.phase);
                    state.status.failure = Some(error);
                }
                Err(_) => return,
            }
        } else {
            // 只有 Verify 返回成功且同一临界区仍未取消时才发布 Running。
            let running = self.enter(Phase::Running);
            if running.is_ok() {
                let mut signal = self.cancel.subscribe();
                if !*signal.borrow() {
                    let _ = signal.changed().await;
                }
            } else if let Ok(mut state) = self.state.lock() {
                state.status.failure_phase = Some(Phase::Verify);
                state.status.failure = running.err();
            }
        }
        self.cleanup().await;
    }
    fn steps(&self) -> Result<Arc<dyn Steps>> {
        self.steps
            .lock()
            .map_err(|_| Error::Internal)?
            .as_ref()
            .cloned()
            .ok_or(Error::InvalidState)
    }
    async fn stages(&self, deadline: Duration) -> Result<()> {
        for phase in [
            Phase::Preflight,
            Phase::Prepare,
            Phase::Commit,
            Phase::Verify,
        ] {
            self.enter(phase)?;
            let cancellation = Cancellation {
                signal: self.cancel.subscribe(),
            };
            let id = self
                .state
                .lock()
                .map_err(|_| Error::Internal)?
                .status
                .transaction;
            let steps = self.steps()?;
            // JoinError 使步骤意外 panic 也进入确定的故障/清理路径，而非遗失全局事务。
            let mut step = tokio::spawn(async move {
                match phase {
                    Phase::Preflight => steps.preflight(id, cancellation).await,
                    Phase::Prepare => steps.prepare(id, cancellation).await,
                    Phase::Commit => steps.commit(id, cancellation).await,
                    Phase::Verify => steps.verify(id, cancellation).await,
                    _ => Err(Error::Internal),
                }
            });
            let expires = tokio::time::Instant::now() + deadline;
            let result = tokio::select! {
                // 不丢弃可能已执行一半系统修改的步骤；先请求其停止，再等待完成。
                result = &mut step => result.map_err(|_| Error::Internal)?,
                _ = tokio::time::sleep_until(expires) => {
                    self.request_cancel()?;
                    let completed = step.await.map_err(|_| Error::Internal)?;
                    completed.and(Err(Error::NetworkTimeout))
                }
            };
            result?;
            if tokio::time::Instant::now() >= expires {
                self.request_cancel()?;
                return Err(Error::NetworkTimeout);
            }
        }
        Ok(())
    }
    async fn cleanup(self: &Arc<Self>) {
        let id = match self.state.lock() {
            Ok(mut state) => {
                state.status.phase = Phase::RollingBack;
                state.recovering = true;
                state.status.transaction
            }
            Err(_) => return,
        };
        // 不因前一阶段失败就跳过清理；即使 Preflight 失败也可能有探测资源待释放。
        let work = self.clone();
        let result = tokio::spawn(async move {
            let steps = work.steps()?;
            steps.rollback(id).await?;
            drop(steps);
            // 成功清理后只保留摘要，不让全局状态延长步骤资源的生命。
            // 先移出锁再析构；析构也受任务监管，panic 不能伪装成清理完成。
            let released = work.steps.lock().map_err(|_| Error::Internal)?.take();
            drop(released);
            Ok(())
        })
        .await
        .unwrap_or(Err(Error::Internal));
        if let Ok(mut state) = self.state.lock() {
            state.status.cleanup_failure = result.err();
            state.status.phase = if result.is_err() {
                Phase::RollbackFailed
            } else if state.status.failure.is_some() {
                Phase::Failed
            } else {
                Phase::Stopped
            };
            state.status.system = if result.is_err() {
                SystemState::Unknown
            } else if state.status.system == SystemState::Unchanged {
                SystemState::Unchanged
            } else {
                SystemState::Restored
            };
            state.finished = true;
            state.recovering = false;
            self.changed.notify_waiters();
        }
    }
    async fn wait_finished(&self) -> Result<()> {
        loop {
            let wake = self.changed.notified();
            tokio::pin!(wake);
            wake.as_mut().enable();
            {
                let state = self.state.lock().map_err(|_| Error::Internal)?;
                if state.finished {
                    return state.status.cleanup_failure.map_or(Ok(()), Err);
                }
            }
            wake.await;
        }
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        if let Ok(owned) = self.owned.lock()
            && let Some(id) = *owned
            && let Ok(inner) = self.get(id)
        {
            let _ = crate::error::diagnostics::observe(
                inner.request_cancel(),
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
    }
}
#[cfg(test)]
#[path = "../../tests/test_vpn_transaction.rs"]
mod test_vpn_transaction;
