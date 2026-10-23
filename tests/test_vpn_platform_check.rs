//! 测试文件：tests/test_vpn_platform_check.rs
//! 测试类型：单元测试、平台证据契约与拒绝边界回归
//! 测试对象：src/vpn/platform_check.rs
//! 测试内容：
//! - absent_adapter_never_passes：无真实生产者时三项不可用。
//! - matching_observations_only_validate_platform_subset：有效替身仅验证平台子集。
//! - foreign_request_platform_and_time_are_rejected：事务、挑战、平台与观测时间不符拒绝。
//! - consumed_cancelled_expired_requests_fail_closed：单次、取消和期限边界。
//! - revoked_or_wrong_source_identity_cannot_authorize：撤销与代次不符不能通过。
//! - permission_and_required_capabilities_are_distinct：权限拒绝、未决与必要能力缺失分开。
//! - egress_coverage_requires_exact_protected_routes：缺少、重复、额外和错误范围的路由拒绝。
//! - unresolved_and_empty_egress_are_not_equivalent：未知集合不能当空集合通过。
//! - inputs_are_bounded_before_copy_and_challenges_differ：数量、地址、ID与独立请求。
//! - serialized_assessment_cannot_leak_or_restore_proof：摘要无挑战、地址或可恢复凭证。
//!
//! 测试依据：VPN启动技术规范、Core只读和生命周期信任边界；预期独立于实现输出。
//! 前置条件：仅模块内替身，固定单调时间；不修改权限、系统路由、DNS或VPN，不读用户资产。
use super::*;
fn targets() -> [IpAddr; 2] {
    [
        "192.0.2.10".parse().unwrap(),
        "2001:db8::10".parse().unwrap(),
    ]
}
fn request(now: Instant) -> Request {
    Request::new(7, IpStrategy::PreferIpv4, true, Some(&targets()), now).unwrap()
}
fn proof(request: &Request, now: Instant) -> VerifiedEvidence {
    let owner = Authority::new().unwrap();
    let identity = owner.identity();
    let mut routes = [None; MAX_EGRESS];
    for (index, destination) in targets().into_iter().enumerate() {
        routes[index] = Some(RouteWitness {
            destination,
            interface: 4,
            protected_scope: 99,
        });
    }
    VerifiedEvidence {
        transaction: request.transaction,
        challenge: request.challenge,
        platform: request.platform,
        observed: now,
        owner,
        identity,
        permission: Permission::Granted,
        capabilities: Capabilities {
            ip_packets: true,
            ipv4: true,
            ipv6: true,
            system_dns: true,
        },
        egress_scope: 99,
        routes,
    }
}
#[test]
fn absent_adapter_never_passes() {
    let now = Instant::now();
    let mut request = request(now);
    let result = request.unavailable(now, false);
    assert_eq!(result.transaction, 7);
    assert_eq!(result.reason, Some(Reason::MissingAdapter));
    assert_eq!(result.permission, CheckState::Unavailable);
    assert_eq!(result.system_capability, CheckState::Unavailable);
    assert_eq!(result.loop_protection, CheckState::Unavailable);
}
#[test]
fn matching_observations_only_validate_platform_subset() {
    let now = Instant::now();
    let mut request = request(now);
    let evidence = proof(&request, now);
    let result = request.assess(Some(evidence), now, false);
    assert_eq!(result.permission, CheckState::Passed);
    assert_eq!(result.system_capability, CheckState::Passed);
    assert_eq!(result.loop_protection, CheckState::Passed);
    assert_eq!(result.reason, None);
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("activation_permitted")
    );
}
#[test]
fn foreign_request_platform_and_time_are_rejected() {
    let now = Instant::now();
    for (case, expected) in [
        (0, Reason::WrongRequest),
        (1, Reason::WrongRequest),
        (2, Reason::WrongPlatform),
        (3, Reason::InvalidObservation),
        (4, Reason::InvalidObservation),
    ] {
        let mut request = request(now);
        let mut evidence = proof(&request, now);
        match case {
            0 => evidence.transaction += 1,
            1 => evidence.challenge[0] ^= 1,
            2 => {
                evidence.platform = if request.platform == Platform::Windows {
                    Platform::Macos
                } else {
                    Platform::Windows
                }
            }
            3 => evidence.observed = now - Duration::from_secs(1),
            _ => evidence.observed = now + Duration::from_secs(1),
        }
        let result = request.assess(Some(evidence), now, false);
        assert_eq!(result.reason, Some(expected));
        assert_ne!(result.permission, CheckState::Passed);
    }
}
#[test]
fn consumed_cancelled_expired_requests_fail_closed() {
    let now = Instant::now();
    let mut one = request(now);
    let evidence = proof(&one, now);
    assert_eq!(
        one.assess(Some(evidence), now, true).reason,
        Some(Reason::Cancelled)
    );
    let evidence = proof(&one, now);
    assert_eq!(
        one.assess(Some(evidence), now, false).reason,
        Some(Reason::Consumed)
    );
    for (age, expected) in [
        (Duration::from_secs(29), None),
        (Duration::from_secs(30), Some(Reason::Expired)),
        (Duration::from_secs(31), Some(Reason::Expired)),
    ] {
        let mut one = request(now);
        let evidence = proof(&one, now);
        assert_eq!(
            one.assess(Some(evidence), now + age, false).reason,
            expected
        );
    }
    let mut one = request(now);
    let evidence = proof(&one, now);
    assert_eq!(
        one.assess(Some(evidence), now - Duration::from_secs(1), false)
            .reason,
        Some(Reason::Expired)
    );
}
#[test]
fn revoked_or_wrong_source_identity_cannot_authorize() {
    let now = Instant::now();
    for revoked in [true, false] {
        let mut one = request(now);
        let mut evidence = proof(&one, now);
        if revoked {
            evidence.owner.revoke().unwrap();
        } else {
            evidence.identity.generation += 1;
        }
        assert_eq!(
            one.assess(Some(evidence), now, false).reason,
            Some(Reason::SourceRevoked)
        );
    }
    let mut one = request(now);
    let evidence = proof(&one, now);
    let owner = evidence.owner.clone();
    let identity = evidence.identity;
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner.with_active(identity, || -> Result<()> {
            panic!("来源故障注入");
        });
    }));
    assert!(panicked.is_err());
    assert_eq!(
        one.assess(Some(evidence), now, false).reason,
        Some(Reason::SourceFailure)
    );
}
#[test]
fn permission_and_required_capabilities_are_distinct() {
    let now = Instant::now();
    for permission in [Permission::Denied, Permission::Undetermined] {
        let mut one = request(now);
        let mut evidence = proof(&one, now);
        evidence.permission = permission;
        let result = one.assess(Some(evidence), now, false);
        assert_eq!(
            result.reason,
            Some(if permission == Permission::Denied {
                Reason::PermissionDenied
            } else {
                Reason::PermissionUndetermined
            })
        );
        assert_eq!(result.system_capability, CheckState::NotRun);
    }
    for missing in 0..4 {
        let mut one = request(now);
        let mut evidence = proof(&one, now);
        match missing {
            0 => evidence.capabilities.ip_packets = false,
            1 => evidence.capabilities.ipv4 = false,
            2 => evidence.capabilities.ipv6 = false,
            _ => evidence.capabilities.system_dns = false,
        }
        let result = one.assess(Some(evidence), now, false);
        assert_eq!(result.permission, CheckState::Passed);
        assert_eq!(result.reason, Some(Reason::MissingCapability));
        assert_eq!(result.loop_protection, CheckState::NotRun);
    }
    for strategy in [IpStrategy::OnlyIpv4, IpStrategy::OnlyIpv6] {
        let mut one = Request::new(7, strategy, false, Some(&targets()), now).unwrap();
        let mut evidence = proof(&one, now);
        evidence.capabilities.system_dns = false;
        if matches!(strategy, IpStrategy::OnlyIpv4) {
            evidence.capabilities.ipv6 = false;
        } else {
            evidence.capabilities.ipv4 = false;
        }
        assert_eq!(one.assess(Some(evidence), now, false).reason, None);
    }
}
#[test]
fn egress_coverage_requires_exact_protected_routes() {
    let now = Instant::now();
    for case in 0..6 {
        let mut one = request(now);
        let mut evidence = proof(&one, now);
        match case {
            0 => evidence.routes[0] = None,
            1 => evidence.routes[1] = evidence.routes[0],
            2 => {
                evidence.routes[2] = Some(RouteWitness {
                    destination: "198.51.100.1".parse().unwrap(),
                    interface: 4,
                    protected_scope: 99,
                })
            }
            3 => evidence.routes[0].as_mut().unwrap().interface = 0,
            4 => evidence.routes[0].as_mut().unwrap().protected_scope = 100,
            _ => evidence.egress_scope = 0,
        }
        let result = one.assess(Some(evidence), now, false);
        assert_eq!(result.loop_protection, CheckState::Rejected);
        assert_eq!(
            result.reason,
            Some(if case == 5 {
                Reason::MissingLoopGuard
            } else {
                Reason::InvalidRouteCoverage
            })
        );
    }
}
#[test]
fn unresolved_and_empty_egress_are_not_equivalent() {
    let now = Instant::now();
    for known in [true, false] {
        let mut one = Request::new(
            7,
            IpStrategy::PreferIpv4,
            true,
            if known { Some(&[]) } else { None },
            now,
        )
        .unwrap();
        let mut evidence = proof(&one, now);
        evidence.routes = [None; MAX_EGRESS];
        assert_eq!(
            one.assess(Some(evidence), now, false).reason,
            if known {
                None
            } else {
                Some(Reason::EgressUnresolved)
            }
        );
    }
    let mut one = Request::new(7, IpStrategy::PreferIpv4, true, Some(&[]), now).unwrap();
    let mut evidence = proof(&one, now);
    evidence.routes = [None; MAX_EGRESS];
    evidence.egress_scope = 0;
    assert_eq!(
        one.assess(Some(evidence), now, false).reason,
        Some(Reason::MissingLoopGuard)
    );
}
#[test]
fn inputs_are_bounded_before_copy_and_challenges_differ() {
    let now = Instant::now();
    assert!(matches!(
        Request::new(0, IpStrategy::PreferIpv4, true, None, now),
        Err(Error::InvalidInput)
    ));
    assert!(matches!(
        Request::new(
            7,
            IpStrategy::PreferIpv4,
            true,
            Some(&[targets()[0]; 33]),
            now
        ),
        Err(Error::ResourceLimit)
    ));
    for targets in [
        vec![targets()[0]; 2],
        vec!["0.0.0.0".parse().unwrap()],
        vec!["ff02::1".parse().unwrap()],
    ] {
        assert!(matches!(
            Request::new(7, IpStrategy::PreferIpv4, true, Some(&targets), now),
            Err(Error::InvalidInput)
        ));
    }
    let bounded: Vec<IpAddr> = (1..=32).map(|v| IpAddr::from([192, 0, 2, v])).collect();
    assert!(Request::new(7, IpStrategy::PreferIpv4, true, Some(&bounded), now).is_ok());
    let one = request(now);
    let two = request(now);
    assert_ne!(one.challenge, two.challenge);
    let mut one = request(now);
    let evidence = proof(&two, now);
    assert_eq!(
        one.assess(Some(evidence), now, false).reason,
        Some(Reason::WrongRequest)
    );
}
#[test]
fn serialized_assessment_cannot_leak_or_restore_proof() {
    let now = Instant::now();
    let mut one = request(now);
    let evidence = proof(&one, now);
    let result = one.assess(Some(evidence), now, false);
    let value = serde_json::to_value(result).unwrap();
    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 6);
    for field in [
        "challenge",
        "routes",
        "identity",
        "owner",
        "activation_permitted",
    ] {
        assert!(!object.contains_key(field));
    }
    assert!(!value.to_string().contains("192.0.2.10"));
    let evidence = proof(&one, now);
    assert_eq!(
        one.assess(Some(evidence), now, false).reason,
        Some(Reason::Consumed)
    );
}
