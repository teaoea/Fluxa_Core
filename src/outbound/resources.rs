//! 有界连接状态，不记录正文；回收任务只持有弱引用，析构不会留下后台所有者。
use super::{Connection, Manager, State};
use crate::{
    error::{Error, Result},
    routing::Transport,
};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex, Weak},
    time::Instant,
};
use tokio::{
    task::JoinHandle,
    time::{Duration, sleep},
};
pub(super) struct Usage {
    pub last_activity: Instant,
    pub uploaded: u64,
    pub downloaded: u64,
}
impl Default for Usage {
    fn default() -> Self {
        Self {
            last_activity: Instant::now(),
            uploaded: 0,
            downloaded: 0,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct SessionStatus {
    pub id: u64,
    pub address: String,
    pub transport: Transport,
    pub revision: u64,
    pub uploaded: u64,
    pub downloaded: u64,
    pub idle_ms: u64,
    pub idle_timeout_ms: u64,
}
#[derive(Debug, Serialize)]
pub struct Status {
    pub revision: u64,
    pub limits: super::Limits,
    pub sessions: Vec<SessionStatus>,
}
impl Connection {
    pub(super) fn activity(&self, bytes: usize, upload: bool, datagram: bool) -> Result<()> {
        let mut usage = self.usage.lock().map_err(|_| Error::Internal)?;
        if bytes > 0 || datagram {
            usage.last_activity = Instant::now();
        }
        let count = if upload {
            &mut usage.uploaded
        } else {
            &mut usage.downloaded
        };
        // 只用于统计，饱和而不回绕；TCP 写失败不会把不确定的部分发送计入成功。
        *count = count.saturating_add(bytes as u64);
        Ok(())
    }
    fn expired(&self) -> bool {
        *self.cancel.borrow()
            || self
                .usage
                .lock()
                .map(|usage| usage.last_activity.elapsed() >= self.idle_timeout)
                .unwrap_or(true)
    }
}
pub(super) fn reap(state: &Arc<Mutex<State>>) -> Result<()> {
    let removed = {
        let mut state = state.lock().map_err(|_| Error::Internal)?;
        let ids: Vec<_> = state
            .registry
            .iter()
            .filter_map(|(id, connection)| connection.expired().then_some(id))
            .collect();
        ids.into_iter()
            .filter_map(|id| state.registry.take(id))
            .collect::<Vec<_>>()
    };
    for connection in removed {
        connection.close();
    }
    Ok(())
}
async fn reaper(state: Weak<Mutex<State>>) {
    loop {
        sleep(Duration::from_millis(50)).await;
        let Some(state) = state.upgrade() else {
            return;
        };
        if state.lock().map(|state| state.stopped).unwrap_or(true) {
            return;
        }
        if reap(&state).is_err() {
            return;
        }
        // 强引用在本轮结束释放，不跨下一次等待持有引擎。
    }
}
impl Manager {
    pub(super) fn start_reaper(&self) -> Result<()> {
        let mut task = self.reaper.lock().map_err(|_| Error::Internal)?;
        if task.as_ref().is_none_or(JoinHandle::is_finished) {
            *task = Some(tokio::spawn(reaper(Arc::downgrade(&self.state))));
        }
        Ok(())
    }
    pub fn status(&self) -> Result<Status> {
        reap(&self.state)?;
        let state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.stopped {
            return Err(Error::InvalidState);
        }
        let mut sessions = Vec::with_capacity(state.registry.len());
        for (id, connection) in state.registry.iter() {
            let usage = connection.usage.lock().map_err(|_| Error::Internal)?;
            sessions.push(SessionStatus {
                id,
                address: connection.address.to_string(),
                transport: connection.transport,
                revision: connection.revision,
                uploaded: usage.uploaded,
                downloaded: usage.downloaded,
                idle_ms: usage
                    .last_activity
                    .elapsed()
                    .as_millis()
                    .min(u64::MAX as u128) as u64,
                idle_timeout_ms: connection.idle_timeout.as_millis() as u64,
            });
        }
        Ok(Status {
            revision: state.network.revision,
            limits: state.network.limits,
            sessions,
        })
    }
}
