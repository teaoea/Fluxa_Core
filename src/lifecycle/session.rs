//! 会话权限与 Rust 对象所有权隔离：权限由显式状态裁决，不以引用计数判断。
use crate::error::{Error, Result};
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, watch};

/// 身份和代次均为内核分配的非复用令牌，不表示优先级或对象地址。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub session: u64,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Active,
    Closing,
    Closed,
    Failed,
}
struct State {
    phase: Phase,
    operations: usize,
    completion: Option<Result<()>>,
}
pub(crate) struct Authority {
    identity: Identity,
    state: Mutex<State>,
    canceled: watch::Sender<bool>,
    changed: Notify,
}
/// 活动操作计数记录正在执行的内核操作；Arc 只保证控制对象的内存安全。
pub(crate) struct Operation {
    owner: Arc<Authority>,
}
impl Authority {
    pub(crate) fn new() -> Result<Arc<Self>> {
        let identity = Identity {
            session: super::identifier().ok_or(Error::ResourceLimit)?,
            generation: super::identifier().ok_or(Error::ResourceLimit)?,
        };
        let (canceled, _) = watch::channel(false);
        Ok(Arc::new(Self {
            identity,
            state: Mutex::new(State {
                phase: Phase::Active,
                operations: 0,
                completion: None,
            }),
            canceled,
            changed: Notify::new(),
        }))
    }
    pub(crate) fn identity(&self) -> Identity {
        self.identity
    }
    pub(crate) fn phase(&self) -> Result<Phase> {
        Ok(self.state.lock().map_err(|_| Error::Internal)?.phase)
    }
    pub(crate) fn check(&self, identity: Identity) -> Result<()> {
        let state = self.state.lock().map_err(|_| Error::Internal)?;
        if self.identity != identity || state.phase != Phase::Active {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    /// 在同一权限临界区登记资源提交；撤销不能穿透检查与提交之间。
    pub(crate) fn with_active<T>(
        &self,
        identity: Identity,
        commit: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let state = self.state.lock().map_err(|_| Error::Internal)?;
        if self.identity != identity || state.phase != Phase::Active {
            return Err(Error::InvalidState);
        }
        commit()
    }
    /// 检查身份、代次与登记操作属于同一临界区，撤销后不能再获取操作权。
    pub(crate) fn enter(self: &Arc<Self>, identity: Identity) -> Result<Operation> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if self.identity != identity || state.phase != Phase::Active {
            return Err(Error::InvalidState);
        }
        state.operations = state
            .operations
            .checked_add(1)
            .ok_or(Error::ResourceLimit)?;
        Ok(Operation {
            owner: self.clone(),
        })
    }
    /// 返回 true 的唯一调用负责启动内核清理；重复调用只等待同一结果。
    pub(crate) fn revoke(&self) -> Result<bool> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.phase != Phase::Active {
            return Ok(false);
        }
        state.phase = Phase::Closing;
        self.canceled.send_replace(true);
        self.changed.notify_waiters();
        Ok(true)
    }
    /// 将有界外部提交与撤销置于同一准入锁：提交失败时权限不变，成功后才撤销新操作。
    pub(crate) fn revoke_after(&self, commit: impl FnOnce() -> Result<()>) -> Result<bool> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.phase != Phase::Active {
            return Err(Error::InvalidState);
        }
        commit()?;
        state.phase = Phase::Closing;
        self.canceled.send_replace(true);
        self.changed.notify_waiters();
        Ok(true)
    }
    pub(crate) async fn canceled(&self) {
        let mut signal = self.canceled.subscribe();
        if *signal.borrow() {
            return;
        }
        let _ = signal.changed().await;
    }
    pub(crate) async fn wait_idle(&self) -> Result<()> {
        loop {
            let wake = self.changed.notified();
            tokio::pin!(wake);
            wake.as_mut().enable();
            if self.state.lock().map_err(|_| Error::Internal)?.operations == 0 {
                return Ok(());
            }
            wake.await;
        }
    }
    /// 仅在真实清理完成后调用；失败不伪装为 Closed，也不能跳过在途操作。
    pub(crate) fn complete(&self, result: Result<()>) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.phase != Phase::Closing || (result.is_ok() && state.operations != 0) {
            return Err(Error::InvalidState);
        }
        state.phase = if result.is_ok() {
            Phase::Closed
        } else {
            Phase::Failed
        };
        state.completion = Some(result);
        self.changed.notify_waiters();
        Ok(())
    }
    pub(crate) async fn wait_closed(&self) -> Result<()> {
        loop {
            let wake = self.changed.notified();
            tokio::pin!(wake);
            wake.as_mut().enable();
            if let Some(result) = self.state.lock().map_err(|_| Error::Internal)?.completion {
                return result;
            }
            wake.await;
        }
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        match self.owner.state.lock() {
            Ok(mut state) => {
                state.operations -= 1;
                self.owner.changed.notify_waiters();
            }
            Err(_) => crate::error::diagnostics::record(
                Error::Internal,
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            ),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/test_lifecycle_session.rs"]
mod test_lifecycle_session;
