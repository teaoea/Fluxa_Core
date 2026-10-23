//! 测试文件：tests/test_vpn_transaction.rs
//! 测试类型：单元测试、启动事务故障与取消回归
//! 测试对象：src/vpn/transaction.rs
//! 测试内容：
//! - four_stages_verify_before_running_and_stop_once：四阶段顺序、验证前不运行、重复停止。
//! - each_stage_failure_cleans_and_preserves_cause：每阶段故障拒绝后续操作并清理。
//! - cancellation_during_commit_waits_for_completion_then_restores：提交中途取消等待确定结果。
//! - rollback_failure_retains_slot_and_explicit_retry：清理失败保留未知系统状态与恢复入口。
//! - deadline_signals_cancellation_without_detaching_work：超时不丢弃在途步骤。
//! - old_transaction_cannot_stop_new_start：旧事务标识和重复启动隔离。
//! - controller_drop_cancels_running_work：宿主释放控制入口后内核继续清理。
//! - panicking_step_still_rolls_back：步骤与清理panic不遗失恢复入口。
//! - limits_reject_before_execution：期限与全局事务槽上限。
//! - terminal_cleanup_releases_steps_and_failure_retains_recovery：完成清理释放步骤资源，失败保留恢复所需对象。
//!
//! 测试依据：VPN启动技术规范、会话生命周期与资源所有权隔离原则。
//!
//! 前置条件：仅可信测试步骤替身；不接管系统、不改路由/DNS、不连接外部网络。
use super::TEST_GATE as GATE;
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
struct Fixture {
    calls: Mutex<Vec<Phase>>,
    fail: Option<Phase>,
    hold: Option<Phase>,
    release: Notify,
    acknowledged: AtomicBool,
    bad_cleanup: AtomicBool,
    gate_verify: AtomicBool,
    panic_phase: Mutex<Option<Phase>>,
}
impl trusted::Sealed for Fixture {}
impl Fixture {
    fn new(fail: Option<Phase>, hold: Option<Phase>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail,
            hold,
            release: Notify::new(),
            acknowledged: AtomicBool::new(false),
            bad_cleanup: AtomicBool::new(false),
            gate_verify: AtomicBool::new(false),
            panic_phase: Mutex::new(None),
        })
    }
    fn step(&self, phase: Phase, mut cancel: Cancellation) -> Step<'_> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(phase);
            let panic = *self.panic_phase.lock().unwrap() == Some(phase);
            assert!(!panic, "故障注入：阶段panic");
            // 单独暂停验证，以观察验证完成前的真实状态，而不是只比对调用列表。
            if phase == Phase::Verify && self.gate_verify.load(Ordering::SeqCst) {
                self.release.notified().await;
            }
            if self.hold == Some(phase) {
                cancel.notified().await;
                self.acknowledged.store(true, Ordering::SeqCst);
                self.release.notified().await;
            }
            if self.fail == Some(phase) {
                Err(Error::Network)
            } else {
                Ok(())
            }
        })
    }
}
impl Steps for Fixture {
    fn preflight(&self, _: u64, cancel: Cancellation) -> Step<'_> {
        self.step(Phase::Preflight, cancel)
    }
    fn prepare(&self, _: u64, cancel: Cancellation) -> Step<'_> {
        self.step(Phase::Prepare, cancel)
    }
    fn commit(&self, _: u64, cancel: Cancellation) -> Step<'_> {
        self.step(Phase::Commit, cancel)
    }
    fn verify(&self, _: u64, cancel: Cancellation) -> Step<'_> {
        self.step(Phase::Verify, cancel)
    }
    fn rollback(&self, _: u64) -> Step<'_> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(Phase::RollingBack);
            let panic = *self.panic_phase.lock().unwrap() == Some(Phase::RollingBack);
            assert!(!panic, "故障注入：回滚panic");
            if self.bad_cleanup.load(Ordering::SeqCst) {
                Err(Error::Internal)
            } else {
                Ok(())
            }
        })
    }
}
async fn wait_for(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}
fn phases() -> Vec<Phase> {
    vec![
        Phase::Preflight,
        Phase::Prepare,
        Phase::Commit,
        Phase::Verify,
    ]
}
#[tokio::test]
async fn four_stages_verify_before_running_and_stop_once() {
    let _gate = GATE.lock().await;
    let owner = Controller::default();
    let driver = Fixture::new(None, None);
    driver.gate_verify.store(true, Ordering::SeqCst);
    let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
    wait_for(|| driver.calls.lock().unwrap().contains(&Phase::Verify)).await;
    assert_eq!(owner.status(id).unwrap().phase, Phase::Verify);
    assert_eq!(owner.status(id).unwrap().system, SystemState::MayBeChanged);
    driver.release.notify_one();
    wait_for(|| owner.status(id).unwrap().phase == Phase::Running).await;
    assert_eq!(*driver.calls.lock().unwrap(), phases());
    assert_eq!(owner.status(id).unwrap().system, SystemState::MayBeChanged);
    let (first, second) = tokio::join!(owner.stop(id), owner.stop(id));
    first.unwrap();
    second.unwrap();
    owner.stop(id).await.unwrap();
    assert_eq!(
        driver
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|p| **p == Phase::RollingBack)
            .count(),
        1
    );
    assert_eq!(owner.status(id).unwrap().phase, Phase::Stopped);
    assert_eq!(owner.status(id).unwrap().system, SystemState::Restored);
}
#[tokio::test]
async fn each_stage_failure_cleans_and_preserves_cause() {
    let _gate = GATE.lock().await;
    for (index, phase) in phases().into_iter().enumerate() {
        let owner = Controller::default();
        let driver = Fixture::new(Some(phase), None);
        let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
        wait_for(|| owner.status(id).unwrap().phase == Phase::Failed).await;
        let status = owner.status(id).unwrap();
        assert_eq!(status.failure, Some(Error::Network));
        assert_eq!(status.failure_phase, Some(phase));
        assert_eq!(status.cleanup_failure, None);
        let mut expected = phases()[..=index].to_vec();
        expected.push(Phase::RollingBack);
        assert_eq!(*driver.calls.lock().unwrap(), expected);
        assert_eq!(
            status.system,
            if index < 2 {
                SystemState::Unchanged
            } else {
                SystemState::Restored
            }
        );
    }
}
#[tokio::test]
async fn cancellation_during_commit_waits_for_completion_then_restores() {
    let _gate = GATE.lock().await;
    let owner = Controller::default();
    let driver = Fixture::new(None, Some(Phase::Commit));
    let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
    wait_for(|| driver.calls.lock().unwrap().contains(&Phase::Commit)).await;
    let stopping = owner.stop(id);
    tokio::pin!(stopping);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut stopping)
            .await
            .is_err()
    );
    assert!(driver.acknowledged.load(Ordering::SeqCst));
    assert!(!driver.calls.lock().unwrap().contains(&Phase::RollingBack));
    assert_eq!(owner.status(id).unwrap().system, SystemState::MayBeChanged);
    driver.release.notify_one();
    stopping.await.unwrap();
    assert_eq!(owner.status(id).unwrap().failure, Some(Error::Cancelled));
    assert!(!driver.calls.lock().unwrap().contains(&Phase::Verify));
    assert_eq!(owner.status(id).unwrap().system, SystemState::Restored);
}
#[tokio::test]
async fn rollback_failure_retains_slot_and_explicit_retry() {
    let _gate = GATE.lock().await;
    let owner = Controller::default();
    let driver = Fixture::new(Some(Phase::Verify), None);
    driver.bad_cleanup.store(true, Ordering::SeqCst);
    let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
    wait_for(|| owner.status(id).unwrap().phase == Phase::RollbackFailed).await;
    let status = owner.status(id).unwrap();
    assert_eq!(status.failure, Some(Error::Network));
    assert_eq!(status.cleanup_failure, Some(Error::Internal));
    assert_eq!(status.system, SystemState::Unknown);
    assert_eq!(owner.stop(id).await, Err(Error::Internal));
    assert_eq!(
        owner.begin(Fixture::new(None, None), Duration::from_secs(1)),
        Err(Error::InvalidState)
    );
    drop(owner);
    let owner = Controller::default();
    assert_eq!(owner.current_status().unwrap().unwrap().transaction, id);
    assert_eq!(owner.status(id).unwrap().phase, Phase::RollbackFailed);
    driver.bad_cleanup.store(false, Ordering::SeqCst);
    owner.retry_cleanup(id).await.unwrap();
    let status = owner.status(id).unwrap();
    assert_eq!(status.phase, Phase::Failed);
    assert_eq!(status.failure, Some(Error::Network));
    assert_eq!(status.cleanup_failure, None);
    assert_eq!(status.system, SystemState::Restored);
}
#[tokio::test]
async fn deadline_signals_cancellation_without_detaching_work() {
    let _gate = GATE.lock().await;
    let owner = Controller::default();
    let driver = Fixture::new(None, Some(Phase::Prepare));
    let id = owner
        .begin(driver.clone(), Duration::from_millis(10))
        .unwrap();
    wait_for(|| driver.acknowledged.load(Ordering::SeqCst)).await;
    assert_eq!(owner.status(id).unwrap().phase, Phase::Prepare);
    assert!(!driver.calls.lock().unwrap().contains(&Phase::RollingBack));
    driver.release.notify_one();
    wait_for(|| owner.status(id).unwrap().phase == Phase::Failed).await;
    assert_eq!(
        owner.status(id).unwrap().failure,
        Some(Error::NetworkTimeout)
    );
    assert!(!driver.calls.lock().unwrap().contains(&Phase::Commit));
}
#[tokio::test]
async fn old_transaction_cannot_stop_new_start() {
    let _gate = GATE.lock().await;
    let owner = Controller::default();
    let first = owner
        .begin(Fixture::new(None, None), Duration::from_secs(1))
        .unwrap();
    wait_for(|| owner.status(first).unwrap().phase == Phase::Running).await;
    assert_eq!(
        owner.begin(Fixture::new(None, None), Duration::from_secs(1)),
        Err(Error::InvalidState)
    );
    owner.stop(first).await.unwrap();
    let second = owner
        .begin(Fixture::new(None, None), Duration::from_secs(1))
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(owner.stop(first).await, Err(Error::InvalidState));
    wait_for(|| owner.status(second).unwrap().phase == Phase::Running).await;
    owner.stop(second).await.unwrap();
}
#[tokio::test]
async fn controller_drop_cancels_running_work() {
    let _gate = GATE.lock().await;
    let owner = Controller::default();
    let driver = Fixture::new(None, None);
    let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
    wait_for(|| owner.status(id).unwrap().phase == Phase::Running).await;
    drop(owner);
    let observer = Controller::default();
    wait_for(|| observer.status(id).unwrap().phase == Phase::Stopped).await;
}
#[tokio::test]
async fn limits_reject_before_execution() {
    let _gate = GATE.lock().await;
    let first = Controller::default();
    let second = Controller::default();
    let third = Controller::default();
    assert_eq!(
        first.begin(Fixture::new(None, None), Duration::ZERO),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        first.begin(Fixture::new(None, None), Duration::from_secs(121)),
        Err(Error::InvalidInput)
    );
    let a = first
        .begin(Fixture::new(None, None), Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        second.begin(Fixture::new(None, None), Duration::from_secs(1)),
        Err(Error::InvalidState)
    );
    assert_eq!(
        third.begin(Fixture::new(None, None), Duration::from_secs(1)),
        Err(Error::InvalidState)
    );
    first.stop(a).await.unwrap();
}

#[tokio::test]
async fn panicking_step_still_rolls_back() {
    let _gate = GATE.lock().await;
    let owner = Controller::default();
    let driver = Fixture::new(None, None);
    *driver.panic_phase.lock().unwrap() = Some(Phase::Commit);
    let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
    wait_for(|| owner.status(id).unwrap().phase == Phase::Failed).await;
    assert_eq!(owner.status(id).unwrap().failure, Some(Error::Internal));
    assert_eq!(owner.status(id).unwrap().system, SystemState::Restored);
    let driver = Fixture::new(None, None);
    *driver.panic_phase.lock().unwrap() = Some(Phase::RollingBack);
    let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
    wait_for(|| owner.status(id).unwrap().phase == Phase::Running).await;
    assert_eq!(owner.stop(id).await, Err(Error::Internal));
    assert_eq!(owner.status(id).unwrap().phase, Phase::RollbackFailed);
    *driver.panic_phase.lock().unwrap() = None;
    owner.retry_cleanup(id).await.unwrap();
}

#[tokio::test]
async fn terminal_cleanup_releases_steps_and_failure_retains_recovery() {
    let _gate = GATE.lock().await;
    // 外部不保留步骤对象；清理后的只读状态不应延长运行资源的生命。
    for failure in [None, Some(Phase::Commit)] {
        let owner = Controller::default();
        let driver = Fixture::new(failure, None);
        let weak = Arc::downgrade(&driver);
        let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
        drop(driver);
        if failure.is_none() {
            wait_for(|| owner.status(id).unwrap().phase == Phase::Running).await;
        } else {
            wait_for(|| owner.status(id).unwrap().phase == Phase::Failed).await;
        }
        owner.stop(id).await.unwrap();
        assert!(weak.upgrade().is_none(), "清理完成后仍持有步骤资源");
        assert_eq!(owner.status(id).unwrap().system, SystemState::Restored);
    }
    // 回滚失败是相反边界：不能丢掉唯一的恢复对象，显式恢复成功再释放。
    let owner = Controller::default();
    let driver = Fixture::new(Some(Phase::Verify), None);
    driver.bad_cleanup.store(true, Ordering::SeqCst);
    let weak = Arc::downgrade(&driver);
    let id = owner.begin(driver.clone(), Duration::from_secs(1)).unwrap();
    drop(driver);
    wait_for(|| owner.status(id).unwrap().phase == Phase::RollbackFailed).await;
    assert_eq!(owner.status(id).unwrap().system, SystemState::Unknown);
    let recovery = weak.upgrade().expect("失败清理必须保留恢复步骤");
    recovery.bad_cleanup.store(false, Ordering::SeqCst);
    drop(recovery);
    owner.retry_cleanup(id).await.unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(owner.status(id).unwrap().phase, Phase::Failed);
    assert_eq!(owner.status(id).unwrap().failure, Some(Error::Network));
}
