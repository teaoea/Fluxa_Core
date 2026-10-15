//! 测试文件：tests/test_routing_debug.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/debug.rs
//! 测试内容：
//! - individual_deadlines_expire_even_in_retained_views：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - audit_debug_not_configurable_or_persistent_and_has_quota：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - audit_debug_ip_waits_for_dns_before_formal_reject：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - audit_scoped_debug_protocol_is_not_global_and_precedes_generic_domain：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - audit_debug_api_overrides_reject_and_clear_restores_formal_rules：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - debug_closed_and_yaml_syntax_return_real_failures：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - failed_debug_replacement_keeps_the_authorized_active_rules：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - policies_errors_and_debug_lifecycle_have_independent_expected_semantics：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - debug_direct_overrides_formal_udp_in_actual_outbound_and_revocation_restores_it：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - expiry_cancels_pending_io_without_frontend_cleanup：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - manager_uses_one_valid_debug_owner_across_control_endpoints：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - debug_exception_covers_tcp_and_quic_but_clear_restores_actual_data_guard：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - debug_overlay_still_overrides_formal_domain_transport_reject：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    #[test]
    fn individual_deadlines_expire_even_in_retained_views() {
        let now = Instant::now();
        let session = Session::new().unwrap();
        let rule = |lifetime, policy| DebugRule {
            rule: Rule {
                protocol: None,
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
mod semantic_tests {
    // Debug 专用入口内部语义回归，预期来自用户授权与撤销契约。
    use crate::{
        error::{Error, ErrorCode},
        routing::{Config, Match, Mode, Policy, Router, Rule, Target, Transport},
    };
    fn rule(kind: Match, value: &str, policy: Policy) -> Rule {
        Rule {
            protocol: None,
            r#type: kind,
            value: value.into(),
            policy,
            outbound: if policy == Policy::Proxy {
                Some("test".into())
            } else {
                None
            },
        }
    }
    fn target(name: &str, ip: &str) -> Target {
        Target {
            domain: Some(name.into()),
            address: Some(ip.into()),
            port: 443,
            transport: Transport::Tcp,
        }
    }

    #[test]
    fn audit_debug_not_configurable_or_persistent_and_has_quota() {
        use crate::routing::{
            debug::{DebugRule, MAX_DEBUG_RULES, Session},
            runtime::GenerationInput,
        };
        // 根字段及来源字段都不存在调试开关，严格解析明确拒绝。
        for input in [
            r#"{"format_version":1,"settings":{},"sources":[],"debug_rules":[]}"#,
            r#"{"format_version":1,"settings":{"debug_rules":[]},"sources":[]}"#,
            r#"{"format_version":1,"settings":{},"sources":[{"id":"debug","layer":"debug","rules":[]}]}"#,
        ] {
            assert!(GenerationInput::from_yaml(input).is_err());
        }
        let session = Session::new().unwrap();
        let over = (0..=MAX_DEBUG_RULES)
            .map(|_| DebugRule {
                rule: rule(Match::Domain, "example.com", Policy::Direct),
                protocol: None,
                lifetime_seconds: Some(60),
            })
            .collect();
        assert!(matches!(
            session.replace(over, None),
            Err(crate::error::Error::ResourceLimit)
        ));
        assert!(!session.status().unwrap().active);
        for _ in 0..128 {
            session
                .replace(
                    vec![DebugRule {
                        rule: rule(Match::Domain, "example.com", Policy::Direct),
                        protocol: None,
                        lifetime_seconds: Some(60),
                    }],
                    None,
                )
                .unwrap();
            session.clear().unwrap();
        }
    }

    #[test]
    fn audit_debug_ip_waits_for_dns_before_formal_reject() {
        use crate::routing::{
            Router,
            debug::{DebugRule, Session},
        };
        let session = Session::new().unwrap();
        session
            .replace(
                vec![DebugRule {
                    rule: rule(Match::IpCidr, "192.0.2.0/24", Policy::Direct),
                    protocol: None,
                    lifetime_seconds: Some(60),
                }],
                None,
            )
            .unwrap();
        let router = Router::compile(Config {
            rules: vec![rule(Match::Domain, "blocked.test", Policy::Reject)],
            ..Config::default()
        })
        .unwrap()
        .with_debug(&session)
        .unwrap();
        let mut request = target("blocked.test", "192.0.2.1");
        request.address = None;
        assert!(router.evaluate_before_dns(request).unwrap().is_none());
        assert_eq!(
            router
                .evaluate(target("blocked.test", "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Direct
        );
        assert_eq!(
            router
                .evaluate(target("blocked.test", "198.51.100.1"))
                .unwrap()
                .policy,
            Policy::Reject
        );
    }

    #[test]
    fn audit_scoped_debug_protocol_is_not_global_and_precedes_generic_domain() {
        use crate::routing::{
            Router,
            debug::{DebugRule, Session},
        };
        let session = Session::new().unwrap();
        session
            .replace(
                vec![
                    DebugRule {
                        rule: rule(Match::DomainSuffix, "example.com", Policy::Direct),
                        protocol: Some(Transport::Udp),
                        lifetime_seconds: Some(60),
                    },
                    DebugRule {
                        rule: rule(Match::Domain, "a.example.com", Policy::Proxy),
                        protocol: None,
                        lifetime_seconds: Some(60),
                    },
                ],
                None,
            )
            .unwrap();
        let router = Router::compile(Config::default())
            .unwrap()
            .with_debug(&session)
            .unwrap();
        let mut request = target("a.example.com", "192.0.2.1");
        request.transport = Transport::Udp;
        assert_eq!(
            router.evaluate(request.clone()).unwrap().policy,
            Policy::Direct
        );
        request.transport = Transport::Tcp;
        assert_eq!(router.evaluate(request).unwrap().policy, Policy::Proxy);
        let mut other = target("other.test", "192.0.2.1");
        other.transport = Transport::Udp;
        assert_eq!(router.evaluate(other).unwrap().policy, Policy::Reject);
        let before = session.status().unwrap();
        assert!(
            session
                .replace(
                    vec![DebugRule {
                        rule: rule(Match::Transport, "udp", Policy::Direct),
                        protocol: None,
                        lifetime_seconds: Some(60),
                    }],
                    None
                )
                .is_err()
        );
        assert_eq!(session.status().unwrap(), before);
    }

    #[test]
    fn audit_debug_api_overrides_reject_and_clear_restores_formal_rules() {
        use crate::routing::{
            Reason, Router,
            debug::{DebugRule, Session},
        };
        let session = Session::new().unwrap();
        let formal = || {
            Router::compile(Config {
                rules: vec![rule(Match::Domain, "example.com", Policy::Reject)],
                ..Config::default()
            })
            .unwrap()
        };
        session
            .replace(
                vec![DebugRule {
                    rule: rule(Match::Domain, "example.com", Policy::Direct),
                    protocol: None,
                    lifetime_seconds: Some(60),
                }],
                None,
            )
            .unwrap();
        let old = formal().with_debug(&session).unwrap();
        let decision = old.evaluate(target("example.com", "192.0.2.1")).unwrap();
        assert_eq!(
            (decision.policy, decision.reason),
            (Policy::Direct, Reason::DebugRule)
        );
        session.clear().unwrap();
        assert!(!session.status().unwrap().active);
        assert_eq!(
            formal()
                .with_debug(&session)
                .unwrap()
                .evaluate(target("example.com", "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Reject
        );
        // 安全契约：清空立即撤销授权；外部持有旧快照不能延长放行权。
        assert_eq!(
            old.evaluate(target("example.com", "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Reject
        );
        session.close().unwrap();
        assert!(session.replace(vec![], None).is_err());
    }

    #[test]
    fn debug_closed_and_yaml_syntax_return_real_failures() {
        use crate::routing::{debug::Session, runtime::GenerationInput};
        let session = Session::new().unwrap();
        session.close().unwrap();
        assert_eq!(session.clear(), Err(Error::DebugSessionClosed));
        assert_eq!(session.status(), Err(Error::DebugSessionClosed));
        assert_eq!(
            session.replace(vec![], None),
            Err(Error::DebugSessionClosed)
        );
        let result = GenerationInput::from_yaml("format_version: \"unterminated");
        assert_eq!(result.err().unwrap(), Error::ConfigYamlSyntax);
    }

    #[test]
    fn failed_debug_replacement_keeps_the_authorized_active_rules() {
        use crate::routing::{
            Match, Rule,
            debug::{DebugRule, Session},
        };
        let session = Session::new().unwrap();
        let row = |lifetime| DebugRule {
            rule: Rule {
                protocol: None,
                r#type: Match::Domain,
                value: "example.test".into(),
                policy: Policy::Direct,
                outbound: None,
            },
            protocol: None,
            lifetime_seconds: lifetime,
        };
        let before = session.replace(vec![row(Some(60))], None).unwrap();
        for lifetime in [None, Some(0), Some(3601)] {
            let error = session.replace(vec![row(lifetime)], None).unwrap_err();
            assert_eq!(error.code(), ErrorCode::DebugInvalidLifetime);
            assert_eq!(error.public_info().params.actual, lifetime);
            assert_eq!(session.status().unwrap().version, before.version);
        }
        let error = session.replace(vec![row(Some(60)); 257], None).unwrap_err();
        assert_eq!(error.code(), ErrorCode::ResourceLimit);
        assert_eq!(session.status().unwrap().version, before.version);
        let router = Router::compile(Config {
            fallback: Policy::Reject,
            ..Default::default()
        })
        .unwrap()
        .with_debug(&session)
        .unwrap();
        assert_eq!(
            router
                .evaluate(Target {
                    domain: Some("example.test".into()),
                    address: None,
                    port: 443,
                    transport: Transport::Tcp
                })
                .unwrap()
                .policy,
            Policy::Direct
        );
    }

    #[test]
    fn policies_errors_and_debug_lifecycle_have_independent_expected_semantics() {
        // 业务契约：纯匹配返回 Reject 是成功的策略决策，执行拒绝与远端 I/O 故障不同。
        let router = Router::compile(Config {
            mode: Mode::Rules,
            fallback: Policy::Reject,
            ..Default::default()
        })
        .unwrap();
        let decision = router
            .evaluate(Target {
                domain: Some("example.test".into()),
                address: None,
                port: 443,
                transport: Transport::Tcp,
            })
            .unwrap();
        assert_eq!(decision.policy, Policy::Reject);
        assert_eq!(Error::Rejected.code().value(), 1101);
        let refused: Error = std::io::Error::from(std::io::ErrorKind::ConnectionRefused).into();
        assert_eq!(refused.code().value(), 6003);
        assert_ne!(refused.code(), Error::Rejected.code());
        let session = crate::routing::debug::Session::new().unwrap();
        session.close().unwrap();
        assert_eq!(session.clear().unwrap_err().code().value(), 4002);
        assert_eq!(Error::DnsTimeout.code().value(), 5001);
        assert_eq!(Error::NetworkTimeout.code().value(), 6002);
        assert_eq!(Error::ResourceLimit.code().value(), 8001);
    }

    #[tokio::test]
    async fn debug_direct_overrides_formal_udp_in_actual_outbound_and_revocation_restores_it() {
        use crate::{
            outbound::Manager,
            routing::debug::{DebugRule, Session},
        };
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        let formal = Router::compile(Config {
            fallback: Policy::Direct,
            rules: vec![rule(Match::Transport, "udp", Policy::Reject)],
            ..Default::default()
        })
        .unwrap();
        let manager = Manager::default();
        manager
            .configure_routing(Some(std::sync::Arc::new(formal)))
            .unwrap();
        let session = std::sync::Arc::new(Session::new().unwrap());
        manager
            .replace_debug(
                session.clone(),
                vec![DebugRule {
                    rule: rule(Match::IpCidr, "127.0.0.1/32", Policy::Direct),
                    protocol: None,
                    lifetime_seconds: Some(60),
                }],
                None,
            )
            .unwrap();
        let target = Target {
            domain: None,
            address: Some(address.ip().to_string()),
            port: address.port(),
            transport: Transport::Udp,
        };
        let snapshot = manager.network_snapshot().unwrap();
        assert_eq!(
            snapshot
                .router
                .as_ref()
                .unwrap()
                .evaluate(target.clone())
                .unwrap()
                .policy,
            Policy::Direct
        );
        let connected = manager
            .connect_configured(crate::outbound::RoutedTarget {
                target: target.clone(),
                timeout_ms: 1000,
                family: crate::outbound::Family::Auto,
                mapping_generation: None,
            })
            .await
            .unwrap();
        session.clear().unwrap();
        assert_eq!(
            snapshot
                .router
                .as_ref()
                .unwrap()
                .evaluate(target.clone())
                .unwrap()
                .policy,
            Policy::Reject
        );
        assert_eq!(
            manager
                .connect_configured(crate::outbound::RoutedTarget {
                    target,
                    timeout_ms: 1000,
                    family: crate::outbound::Family::Auto,
                    mapping_generation: None
                })
                .await
                .err(),
            Some(Error::Rejected)
        );
        manager.close(connected.connection.id).await.unwrap();
        manager.stop().await.unwrap();
    }

    #[tokio::test]
    async fn expiry_cancels_pending_io_without_frontend_cleanup() {
        use crate::{
            outbound::{Family, Manager, RoutedTarget},
            routing::debug::{DebugRule, Session},
        };
        let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = peer.local_addr().unwrap();
        let manager = std::sync::Arc::new(Manager::default());
        manager
            .configure_routing(Some(std::sync::Arc::new(
                Router::compile(Config::default()).unwrap(),
            )))
            .unwrap();
        let debug = std::sync::Arc::new(Session::new().unwrap());
        manager
            .replace_debug(
                debug.clone(),
                vec![DebugRule {
                    rule: rule(Match::IpCidr, "127.0.0.1/32", Policy::Direct),
                    protocol: None,
                    lifetime_seconds: Some(1),
                }],
                None,
            )
            .unwrap();
        let opened = manager
            .connect_configured(RoutedTarget {
                target: Target {
                    domain: None,
                    address: Some("127.0.0.1".into()),
                    port: address.port(),
                    transport: Transport::Udp,
                },
                timeout_ms: 1000,
                family: Family::Auto,
                mapping_generation: None,
            })
            .await
            .unwrap();
        let reader = manager.clone();
        let reading =
            tokio::spawn(async move { reader.read(opened.connection.id, 32, 10000).await });
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), reading)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            result,
            Err(Error::Cancelled) | Err(Error::Rejected)
        ));
        assert!(!debug.status().unwrap().active);
        manager.stop().await.unwrap();
    }

    #[test]
    fn manager_uses_one_valid_debug_owner_across_control_endpoints() {
        use crate::{
            outbound::Manager,
            routing::debug::{DebugRule, Session},
        };
        let manager = Manager::default();
        manager
            .configure_routing(Some(std::sync::Arc::new(
                Router::compile(Config::default()).unwrap(),
            )))
            .unwrap();
        let first = std::sync::Arc::new(Session::new().unwrap());
        let second = std::sync::Arc::new(Session::new().unwrap());
        let row = || DebugRule {
            rule: rule(Match::Domain, "example.test", Policy::Direct),
            protocol: None,
            lifetime_seconds: Some(60),
        };
        manager
            .replace_debug(first.clone(), vec![row()], None)
            .unwrap();
        assert_eq!(
            manager.replace_debug(second.clone(), vec![row()], None),
            Err(Error::DebugUnauthorized)
        );
        assert!(!second.status().unwrap().active);
        first.clear().unwrap();
        manager.unbind_debug(first.identity()).unwrap();
        manager
            .replace_debug(second.clone(), vec![row()], None)
            .unwrap();
        manager.unbind_debug(first.identity()).unwrap();
        assert_eq!(
            manager.replace_debug(first, vec![row()], None),
            Err(Error::DebugUnauthorized)
        );
    }

    #[tokio::test]
    async fn debug_exception_covers_tcp_and_quic_but_clear_restores_actual_data_guard() {
        use crate::{
            outbound::{Family, Manager, RoutedTarget},
            routing::debug::{DebugRule, Session},
        };
        use tokio::io::AsyncReadExt;
        let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = tcp.local_addr().unwrap();
        let manager = Manager::default();
        manager
            .configure_routing(Some(std::sync::Arc::new(
                Router::compile(Config {
                    fallback: Policy::Direct,
                    rules: vec![rule(Match::Transport, "tcp", Policy::Reject)],
                    ..Default::default()
                })
                .unwrap(),
            )))
            .unwrap();
        let session = std::sync::Arc::new(Session::new().unwrap());
        let exception = || DebugRule {
            rule: rule(Match::IpCidr, "127.0.0.1/32", Policy::Direct),
            protocol: None,
            lifetime_seconds: Some(30),
        };
        manager
            .replace_debug(session.clone(), vec![exception()], None)
            .unwrap();
        let request = RoutedTarget {
            target: Target {
                domain: None,
                address: Some("127.0.0.1".into()),
                port: address.port(),
                transport: Transport::Tcp,
            },
            timeout_ms: 1000,
            family: Family::Auto,
            mapping_generation: None,
        };
        let opened = manager.connect_configured(request.clone()).await.unwrap();
        let (mut accepted, _) = tcp.accept().await.unwrap();
        manager
            .write(opened.connection.id, b"debug TCP", 1000)
            .await
            .unwrap();
        let mut bytes = [0; 9];
        accepted.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"debug TCP");
        session.clear().unwrap();
        assert_eq!(
            manager.connect_configured(request).await.err(),
            Some(Error::Rejected)
        );
        manager.stop().await.unwrap();

        let udp = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let manager = Manager::default();
        manager
            .configure_routing(Some(std::sync::Arc::new(
                Router::compile(Config {
                    fallback: Policy::Direct,
                    rules: vec![rule(Match::Transport, "quic", Policy::Reject)],
                    ..Default::default()
                })
                .unwrap(),
            )))
            .unwrap();
        let session = std::sync::Arc::new(Session::new().unwrap());
        manager
            .replace_debug(session.clone(), vec![exception()], None)
            .unwrap();
        let request = RoutedTarget {
            target: Target {
                domain: None,
                address: Some("127.0.0.1".into()),
                port: udp.local_addr().unwrap().port(),
                transport: Transport::Udp,
            },
            timeout_ms: 1000,
            family: Family::Auto,
            mapping_generation: None,
        };
        let opened = manager.connect_configured(request.clone()).await.unwrap();
        let mut packet = vec![0xc0, 0, 0, 0, 1, 1, 1, 1, 2, 0, 17];
        packet.extend([0; 17]);
        assert_eq!(
            manager
                .write(opened.connection.id, &packet, 1000)
                .await
                .unwrap(),
            packet.len()
        );
        let mut received = [0; 64];
        assert_eq!(udp.recv(&mut received).await.unwrap(), packet.len());
        session.clear().unwrap();
        let ordinary = manager.connect_configured(request).await.unwrap();
        assert_eq!(
            manager
                .write(ordinary.connection.id, &packet, 1000)
                .await
                .err(),
            Some(Error::Rejected)
        );
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(30),
                udp.recv(&mut received)
            )
            .await
            .is_err()
        );
        manager.stop().await.unwrap();
    }

    #[test]
    fn debug_overlay_still_overrides_formal_domain_transport_reject() {
        use crate::routing::debug::{DebugRule, Session};
        let mut formal = rule(Match::DomainSuffix, "example.com", Policy::Reject);
        formal.protocol = Some(Transport::Tcp);
        let session = Session::new().unwrap();
        session
            .replace(
                vec![DebugRule {
                    rule: rule(Match::Domain, "a.example.com", Policy::Direct),
                    protocol: None,
                    lifetime_seconds: Some(60),
                }],
                None,
            )
            .unwrap();
        let router = Router::compile(Config {
            rules: vec![formal],
            ..Config::default()
        })
        .unwrap()
        .with_debug(&session)
        .unwrap();
        assert_eq!(
            router
                .evaluate(target("a.example.com", "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Direct
        );
        session.clear().unwrap();
        assert_eq!(
            router
                .evaluate(target("a.example.com", "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Reject
        );
    }
}
