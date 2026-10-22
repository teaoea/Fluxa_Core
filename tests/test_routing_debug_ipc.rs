//! 测试文件：tests/test_routing_debug_ipc.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/debug/ipc.rs
//! 测试内容：
//! - handshake_preserves_real_failure_and_never_turns_error_into_success：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - signature_is_bound_to_session_sequence_payload_and_deadline：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - control_fields_are_closed_and_permissions_are_separate：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - oversized_frame_rejected_before_reading_body：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - failed_binding_does_not_activate_debug_and_audit_is_bounded：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - signed_os_peer_and_operation_permissions_use_real_unix_socket：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn handshake_preserves_real_failure_and_never_turns_error_into_success() {
        for error in [
            Error::ResourceLimit,
            Error::DebugIdentityRejected,
            Error::DebugAuthenticationFailed,
            Error::Internal,
            Error::InvalidInput,
        ] {
            let reply = Reply::from_result(Err(error));
            validate_reply(&reply).unwrap();
            assert_eq!(handshake_failure(reply.error_info), error);
        }
        let mut forged = Reply::from_result(Err(Error::DebugUnauthorized));
        forged.success = true;
        assert_eq!(
            validate_reply(&forged),
            Err(Error::DebugAuthenticationFailed)
        );
        assert_eq!(handshake_failure(None), Error::DebugAuthenticationFailed);
    }
    #[test]
    fn signature_is_bound_to_session_sequence_payload_and_deadline() {
        let key = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let public_key = key.public_key().as_ref().try_into().unwrap();
        let hello = Hello {
            version: VERSION,
            session: 17,
            challenge: [19; 32],
        };
        let payload = b"{\"operation\":\"status\"}";
        let signature = key.sign(&transcript(&hello, 1, payload));
        assert!(verify_signature(&public_key, &hello, 1, payload, signature.as_ref()).is_ok());
        for (sequence, body) in [
            (2, payload.as_slice()),
            (1, b"{\"operation\":\"clear\"}".as_slice()),
        ] {
            assert_eq!(
                verify_signature(&public_key, &hello, sequence, body, signature.as_ref()),
                Err(Error::DebugAuthenticationFailed)
            );
        }
        let other = Hello {
            session: 18,
            ..hello
        };
        assert_eq!(
            verify_signature(&public_key, &other, 1, payload, signature.as_ref()),
            Err(Error::DebugAuthenticationFailed)
        );
        assert_eq!(
            verify_signature(&[0; 32], &hello, 1, payload, signature.as_ref()),
            Err(Error::DebugAuthenticationFailed)
        );
    }
    #[test]
    fn control_fields_are_closed_and_permissions_are_separate() {
        for input in [
            r#"{"operation":"status","authorized":true}"#,
            r#"{"operation":"clear","uid":0}"#,
            r#"{"operation":"status","operation":"clear"}"#,
            r#"{"operation":"shell","command":"whoami"}"#,
        ] {
            assert!(serde_json::from_str::<Request>(input).is_err());
        }
        assert_eq!(Request::Status {}.permission(), Permission::Status);
        assert_ne!(Request::Clear {}.permission(), Permission::Status);
        let mut policy = Policy {
            version: VERSION,
            principals: vec![Principal {
                uid: 0,
                requirement: "identifier test".into(),
                public_key: [7; 32],
                permissions: vec![Permission::Status],
            }],
        };
        assert!(validate_policy(&policy).is_ok());
        policy.principals[0].permissions.push(Permission::Status);
        assert_eq!(validate_policy(&policy), Err(Error::InvalidInput));
    }
    #[tokio::test]
    async fn oversized_frame_rejected_before_reading_body() {
        let (mut sender, mut receiver) = UnixStream::pair().unwrap();
        sender.write_u32((MAX_FRAME + 1) as u32).await.unwrap();
        assert_eq!(
            read_frame(&mut receiver).await.unwrap_err(),
            Error::ResourceLimit
        );
    }
    #[test]
    fn failed_binding_does_not_activate_debug_and_audit_is_bounded() {
        let manager = Arc::new(Manager::default());
        let control = Arc::new(Control {
            manager,
            owner: Mutex::new(None),
            audit: Mutex::new(VecDeque::new()),
        });
        let session = Arc::new(Session::new().unwrap());
        let error = control
            .dispatch(
                9,
                &session,
                Request::Replace {
                    rules: vec![],
                    outbound: None,
                },
            )
            .err();
        assert_eq!(error, Some(Error::InvalidState));
        assert!(!session.status().unwrap().active);
        assert_eq!(*control.owner.lock().unwrap(), None);
        for _ in 0..MAX_AUDIT + 3 {
            control
                .record(None, Some(Permission::Status), Some(4004))
                .unwrap();
        }
        assert_eq!(control.audit.lock().unwrap().len(), MAX_AUDIT);
    }
}
#[cfg(all(test, target_os = "macos"))]
mod signed_tests {
    use super::super::*;
    async fn own_peer() -> (UnixStream, UnixStream) {
        // socketpair 没有 accept 身份上下文，使用真实监听/接受产生 OS 对端凭据。
        let path = std::env::temp_dir().join(format!(
            "nodus-peer-{}",
            crate::lifecycle::identifier().unwrap()
        ));
        let listener = UnixListener::bind(&path).unwrap();
        let client = UnixStream::connect(&path).await.unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        drop(listener);
        std::fs::remove_file(path).unwrap();
        (stream, client)
    }
    #[tokio::test]
    async fn signed_os_peer_and_operation_permissions_use_real_unix_socket() {
        // 测试启动器给本测试二进制加 Hardened Runtime 签名；生产不存在 env 绕过入口。
        let requirement = match std::env::var("NODUS_DEBUG_TEST_REQUIREMENT") {
            Ok(value) => value,
            Err(_) => {
                let (stream, _client) = own_peer().await;
                let peer = debug_identity::inspect(&stream).unwrap();
                assert_eq!(
                    debug_identity::verify(&peer, "identifier nodus-debug-test"),
                    Err(Error::DebugIdentityRejected)
                );
                return;
            }
        };
        let (stream, _client) = own_peer().await;
        let peer = debug_identity::inspect(&stream).unwrap();
        debug_identity::verify(&peer, &requirement).unwrap();
        let key = Ed25519KeyPair::from_seed_unchecked(&[11; 32]).unwrap();
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "nodus-debug-auth-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(
            &directory,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let credential = directory.join("test-only-seed");
        std::fs::write(&credential, [11; 32]).unwrap();
        std::fs::set_permissions(
            &credential,
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )
        .unwrap();
        let manager = Arc::new(Manager::default());
        manager
            .configure_routing(Some(Arc::new(
                crate::routing::Router::compile(crate::routing::Config::default()).unwrap(),
            )))
            .unwrap();
        let second_key = Ed25519KeyPair::from_seed_unchecked(&[17; 32]).unwrap();
        let mut policy = Policy {
            version: VERSION,
            principals: vec![
                Principal {
                    uid: peer.uid,
                    requirement: requirement.clone(),
                    public_key: key.public_key().as_ref().try_into().unwrap(),
                    permissions: vec![Permission::Status],
                },
                Principal {
                    uid: peer.uid,
                    requirement: requirement.clone(),
                    public_key: second_key.public_key().as_ref().try_into().unwrap(),
                    permissions: vec![
                        Permission::Replace,
                        Permission::Clear,
                        Permission::Status,
                        Permission::Close,
                        Permission::Audit,
                    ],
                },
            ],
        };
        let swift = std::env::var("NODUS_DEBUG_TEST_SWIFT").ok();
        if swift.is_some() {
            let swift_requirement = std::env::var("NODUS_DEBUG_TEST_SWIFT_REQUIREMENT").unwrap();
            policy.principals.push(Principal {
                uid: peer.uid,
                requirement: swift_requirement,
                public_key: Ed25519KeyPair::from_seed_unchecked(&[23; 32])
                    .unwrap()
                    .public_key()
                    .as_ref()
                    .try_into()
                    .unwrap(),
                permissions: vec![
                    Permission::Replace,
                    Permission::Clear,
                    Permission::Status,
                    Permission::Close,
                    Permission::Audit,
                ],
            });
        }
        let mut service = Service::start_policy(policy, &directory, manager.clone())
            .await
            .unwrap();
        let mut client = Client::connect(
            &directory.join("debug.sock"),
            &credential,
            peer.uid,
            &requirement,
            60,
        )
        .await
        .unwrap();
        assert!(client.command(&Request::Status {}).await.unwrap().success);
        let reply = client
            .command(&Request::Replace {
                rules: vec![],
                outbound: None,
            })
            .await
            .unwrap();
        assert!(!reply.success);
        assert_eq!(
            reply.error_info.unwrap().code,
            crate::error::ErrorCode::DebugUnauthorized
        );
        assert_eq!(
            manager
                .network_snapshot()
                .unwrap()
                .router
                .as_ref()
                .unwrap()
                .evaluate(crate::routing::Target {
                    domain: Some("example.test".into()),
                    address: None,
                    port: 443,
                    transport: Transport::Tcp
                })
                .unwrap()
                .policy,
            crate::routing::Policy::Reject
        );
        drop(client);
        std::fs::write(&credential, [17; 32]).unwrap();
        let mut authorized = Client::connect(
            &directory.join("debug.sock"),
            &credential,
            peer.uid,
            &requirement,
            60,
        )
        .await
        .unwrap();
        let row = WireRule {
            rule: crate::routing::Rule {
                protocol: None,
                r#type: crate::routing::Match::IpCidr,
                value: "127.0.0.1/32".into(),
                policy: crate::routing::Policy::Direct,
                outbound: None,
            },
            protocol: None,
            lifetime_seconds: Some(30),
        };
        assert!(
            authorized
                .command(&Request::Replace {
                    rules: vec![row.clone()],
                    outbound: None
                })
                .await
                .unwrap()
                .success
        );
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let target = crate::routing::Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port: socket.local_addr().unwrap().port(),
            transport: Transport::Udp,
        };
        let snapshot = manager.network_snapshot().unwrap();
        let request = crate::outbound::RoutedTarget {
            target: target.clone(),
            timeout_ms: 1000,
            family: crate::outbound::Family::Auto,
            mapping_generation: None,
        };
        let opened = manager.connect_configured(request.clone()).await.unwrap();
        assert_eq!(
            manager
                .write(opened.connection.id, b"authorized", 1000)
                .await
                .unwrap(),
            10
        );
        let mut received = [0; 16];
        assert_eq!(socket.recv(&mut received).await.unwrap(), 10);
        assert!(
            authorized
                .command(&Request::Clear {})
                .await
                .unwrap()
                .success
        );
        assert_eq!(
            snapshot
                .router
                .as_ref()
                .unwrap()
                .evaluate(target.clone())
                .unwrap()
                .policy,
            crate::routing::Policy::Reject
        );
        assert_eq!(
            manager
                .write(opened.connection.id, b"revoked", 1000)
                .await
                .err(),
            Some(Error::InvalidState)
        );
        // clear 成功意味着旧连接已经关闭并释放句柄名额。
        assert!(manager.status().unwrap().sessions.is_empty());
        assert!(
            authorized
                .command(&Request::Replace {
                    rules: vec![row],
                    outbound: None
                })
                .await
                .unwrap()
                .success
        );
        assert!(
            authorized
                .command(&Request::Clear {})
                .await
                .unwrap()
                .success
        );
        if let Some(swift) = swift {
            std::fs::write(&credential, [23; 32]).unwrap();
            let output = std::process::Command::new(swift)
                .arg(directory.join("debug.sock"))
                .arg(&credential)
                .arg(peer.uid.to_string())
                .arg(&requirement)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "Swift 签名 IPC 夹具失败：{}；{}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            );
            eprintln!("{}", String::from_utf8_lossy(&output.stdout));
            std::fs::write(&credential, [17; 32]).unwrap();
        }
        assert!(
            authorized
                .command(&Request::Replace {
                    rules: vec![WireRule {
                        rule: crate::routing::Rule {
                            protocol: None,
                            r#type: crate::routing::Match::IpCidr,
                            value: "127.0.0.1/32".into(),
                            policy: crate::routing::Policy::Direct,
                            outbound: None
                        },
                        protocol: None,
                        lifetime_seconds: Some(30)
                    }],
                    outbound: None
                })
                .await
                .unwrap()
                .success
        );
        // 正确签名但重复序号仍必须拒绝并撤销当前连接授权。
        let payload = serde_json::to_vec(&Request::Status {}).unwrap();
        let sequence = authorized.sequence - 1;
        let signature = authorized
            .key
            .sign(&transcript(&authorized.hello, sequence, &payload))
            .as_ref()
            .to_vec();
        write_frame(
            &mut authorized.stream,
            &Envelope {
                sequence,
                payload,
                signature,
            },
        )
        .await
        .unwrap();
        let reply: Reply =
            serde_json::from_slice(&read_frame(&mut authorized.stream).await.unwrap()).unwrap();
        assert_eq!(
            reply.error_info.unwrap().code,
            crate::error::ErrorCode::DebugAuthenticationFailed
        );
        drop(authorized);
        service.close().await.unwrap();
        service.close().await.unwrap();
        assert!(!directory.join("debug.sock").exists());
        manager.stop().await.unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }
}
