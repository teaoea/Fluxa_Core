//! 测试文件：tests/test_vpn_startup_budget.rs
//! 测试类型：单元测试、启动预算安全回归
//! 测试对象：src/vpn/startup_budget.rs
//! 测试内容：complete_plan_counts_coexisting_and_transient_resources、each_missing_component_is_unknown_not_zero、process_and_system_limits_are_independent、overflow_is_rejected_in_each_dimension、observations_are_transaction_bound_fresh_and_one_shot、cancelled_or_absent_observation_never_admits、summary_is_diagnostic_only。
//! 测试依据：VPN启动技术规范的峰值准入、旧快照真实内存和失败拒绝原则。
//! 前置条件：隔离数值与单调时钟；不读取系统余量，不接管VPN。
use super::*;
fn usage(memory_bytes: u64, file_descriptors: u64) -> Usage {
    Usage {
        memory_bytes,
        file_descriptors,
    }
}
fn plan() -> Plan {
    Plan {
        retained: Some(usage(100, 10)),
        candidate: Some(usage(200, 20)),
        buffers: Some(usage(30, 3)),
        temporary: Some(usage(40, 4)),
        safety_margin: usage(5, 1),
        process_limit: usage(375, 38),
    }
}
fn observation(now: Instant) -> VerifiedHeadroom {
    VerifiedHeadroom {
        transaction: 7,
        observed: now,
        available: usage(275, 28),
    }
}
fn assess(plan: Plan, observation: Option<VerifiedHeadroom>, now: Instant) -> Assessment {
    Request::new(7, plan, now)
        .unwrap()
        .assess(observation, now, false)
        .unwrap()
}
#[test]
fn complete_plan_counts_coexisting_and_transient_resources() {
    let now = Instant::now();
    let result = assess(plan(), Some(observation(now)), now);
    assert!(result.budget_satisfied);
    assert_eq!(result.peak, Some(usage(375, 38)));
    assert_eq!(result.additional, Some(usage(275, 28)));
    // 余量只比较新增部分，旧快照已经在系统现有占用中；进程峰值仍包含旧快照。
    let mut p = plan();
    p.retained = Some(usage(101, 10));
    assert_eq!(
        assess(p, Some(observation(now)), now).reason,
        Some(Reason::ProcessMemoryLimit)
    );
}
#[test]
fn each_missing_component_is_unknown_not_zero() {
    let now = Instant::now();
    for index in 0..4 {
        let mut p = plan();
        match index {
            0 => p.retained = None,
            1 => p.candidate = None,
            2 => p.buffers = None,
            _ => p.temporary = None,
        }
        let r = assess(p, Some(observation(now)), now);
        assert!(!r.budget_satisfied);
        assert!(r.peak.is_none());
        assert_eq!(r.reason, Some(Reason::IncompletePlan));
    }
    let mut p = plan();
    p.temporary = Some(usage(0, 0));
    assert!(assess(p, Some(observation(now)), now).budget_satisfied);
}
#[test]
fn process_and_system_limits_are_independent() {
    let now = Instant::now();
    let mut p = plan();
    p.process_limit.memory_bytes = 374;
    assert_eq!(
        assess(p, Some(observation(now)), now).reason,
        Some(Reason::ProcessMemoryLimit)
    );
    let mut p = plan();
    p.process_limit.file_descriptors = 37;
    assert_eq!(
        assess(p, Some(observation(now)), now).reason,
        Some(Reason::ProcessDescriptorLimit)
    );
    let mut o = observation(now);
    o.available.memory_bytes = 274;
    assert_eq!(
        assess(plan(), Some(o), now).reason,
        Some(Reason::SystemMemoryHeadroom)
    );
    let mut o = observation(now);
    o.available.file_descriptors = 27;
    assert_eq!(
        assess(plan(), Some(o), now).reason,
        Some(Reason::SystemDescriptorHeadroom)
    );
}
#[test]
fn overflow_is_rejected_in_each_dimension() {
    let now = Instant::now();
    for component in 0..5 {
        for value in [usage(u64::MAX, 0), usage(0, u64::MAX)] {
            let mut p = plan();
            match component {
                0 => p.retained = Some(value),
                1 => p.candidate = Some(value),
                2 => p.buffers = Some(value),
                3 => p.temporary = Some(value),
                _ => p.safety_margin = value,
            }
            let r = assess(p, Some(observation(now)), now);
            assert_eq!(r.reason, Some(Reason::ArithmeticOverflow));
            assert!(!r.budget_satisfied);
            assert!(r.peak.is_none());
        }
    }
}
#[test]
fn observations_are_transaction_bound_fresh_and_one_shot() {
    let now = Instant::now();
    let mut o = observation(now);
    o.transaction = 8;
    assert_eq!(
        assess(plan(), Some(o), now).reason,
        Some(Reason::InvalidObservation)
    );
    for observed in [now - Duration::from_nanos(1), now + Duration::from_nanos(1)] {
        let mut o = observation(now);
        o.observed = observed;
        assert_eq!(
            assess(plan(), Some(o), now).reason,
            Some(Reason::InvalidObservation)
        );
    }
    let mut request = Request::new(7, plan(), now).unwrap();
    assert_eq!(
        request
            .assess(Some(observation(now)), now + Duration::from_secs(5), false)
            .unwrap()
            .reason,
        Some(Reason::ExpiredObservation)
    );
    assert!(matches!(
        request.assess(Some(observation(now)), now, false),
        Err(Error::InvalidState)
    ));
    let mut request = Request::new(7, plan(), now).unwrap();
    assert!(
        request
            .assess(
                Some(observation(now)),
                now + Duration::from_millis(4999),
                false
            )
            .unwrap()
            .budget_satisfied
    );
    assert!(matches!(
        Request::new(0, plan(), now),
        Err(Error::InvalidInput)
    ));
}
#[test]
fn cancelled_or_absent_observation_never_admits() {
    let now = Instant::now();
    assert_eq!(
        assess(plan(), None, now).reason,
        Some(Reason::MissingSystemObservation)
    );
    let mut request = Request::new(7, plan(), now).unwrap();
    assert_eq!(
        request
            .assess(Some(observation(now)), now, true)
            .unwrap()
            .reason,
        Some(Reason::Cancelled)
    );
    let mut request = Request::new(7, plan(), now).unwrap();
    assert_eq!(
        request
            .assess(Some(observation(now)), now - Duration::from_nanos(1), false)
            .unwrap()
            .reason,
        Some(Reason::InvalidObservation)
    );
}
#[test]
fn summary_is_diagnostic_only() {
    let now = Instant::now();
    let r = assess(Plan::incomplete(), None, now);
    let json = serde_json::to_value(r).unwrap();
    assert_eq!(json["reason"], "incomplete-plan");
    assert_eq!(json["budget_satisfied"], false);
    assert_eq!(json["peak"], serde_json::Value::Null);
    assert!(json.get("available").is_none());
}
