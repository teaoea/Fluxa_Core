//! configuration_check_returns_editor_location_without_applying：检查错误代码和 UTF-16 定位，运行版本不变。
//! script_cache_clear_is_owned_and_schema_is_strict：验证缓存清理命令不接受路径或宿主权限注入。
//!
//! 测试文件：tests/test_ffi.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/ffi.rs
//! 测试内容：
//! - error_query_contains_serializer_panic_and_preserves_state：编码 panic 被捕获，查询容量不足不写入也不清除原错误。
//! - destroy_continues_after_failed_close_with_retained_engine：毒化准备槽后仍停止出站并释放真实代理监听，旧 Arc 不保持活动资源。
//! - storage_commands_reject_unentitled_hosts_and_role_injection：真实小栈 ABI 拒绝无共享组宿主、越界文件名与自报写权限。
//! - candidate_commands_are_safe_on_bounded_host_stack：256 KiB 实际 ABI 命令分发、候选查询与关闭。
//! - http_request_works_on_bounded_host_stack：256 KiB 宿主栈可构造请求并获得真实校验错误，不溢出或触网。
//! - main_dns_and_geoip_status_use_same_configuration：真实 ABI hosts、Disabled 状态和拒绝部分关闭保持同一快照。
//! - provider_commands_use_real_tls_and_keep_process_rules_ignored：真实 ABI 刷新由内核下载，远程进程条目不注入。
//! - main_configuration_apply_is_atomic_and_status_is_read_only：公开命令原子提交、错误不改版本且查询不启动 VPN。
//! - main_configuration_validation_is_read_only_and_structured：主配置 C ABI 校验不改网络版本，失败返回整数错误。
//! - invalid_internal_error_encoding_does_not_become_no_error：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - error_protocol_is_thread_local_and_success_clears_stale_error：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - error_query_remains_available_when_response_registry_is_full：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - legacy_validation_order_preserves_handle_before_json：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - failed_test_assertion_returns_owned_response_capacity：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - abi_rejects_invalid_and_destroyed_handles：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - concurrent_owners_release_their_own_handles：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - output_buffers_obey_budget_and_handle_type：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - admission_failure_keeps_tcp_bytes_and_returns_reservations：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - admission_failure_keeps_tun_return_packet：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - nested_runtime_does_not_detach_failed_http_calls：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn error_query_contains_serializer_panic_and_preserves_state() {
        struct Panicking;
        impl Serialize for Panicking {
            fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                panic!("仅注入错误编码器 unwind");
            }
        }
        let mut buffer = [0u8; crate::error::MAX_ERROR_BYTES];
        assert_eq!(encode_abi_error(&Panicking, &mut buffer), None);
        assert_eq!(
            ErrorInfo::from_json(ABI_ENCODING_FAILURE).unwrap().code,
            ErrorCode::SerializationFailed
        );
        set_last_error(ErrorInfo::new(ErrorCode::InvalidHandle));
        // SAFETY: 空目标只查询长度，不能被解引用。
        let needed = unsafe { nodus_core_last_error_copy(std::ptr::null_mut(), 0) };
        assert!(needed > 1);
        let mut short = [0xa5u8; 1];
        // SAFETY: 独占一字节目标在同步调用期间有效；容量不足必须完全不写入。
        let short_length = unsafe { nodus_core_last_error_copy(short.as_mut_ptr(), short.len()) };
        assert_eq!(short_length, needed);
        assert_eq!(short, [0xa5]);
        assert_eq!(
            nodus_core_last_error_code(),
            ErrorCode::InvalidHandle.value()
        );
        clear_last_error();
    }
    #[test]
    fn destroy_continues_after_failed_close_with_retained_engine() {
        let _gate = TEST_GATE.blocking_lock();
        let handle = nodus_core_create();
        let held = engine(handle).unwrap();
        held.outbound
            .configure_network(
                None,
                Some(Arc::new(
                    crate::routing::Router::compile(crate::routing::Config {
                        mode: crate::routing::Mode::Direct,
                        ..Default::default()
                    })
                    .unwrap(),
                )),
                crate::outbound::Limits::default(),
            )
            .unwrap();
        let proxy = runtime()
            .unwrap()
            .block_on(MitmProxy::start(
                0,
                &[],
                Arc::new(Authority::generate().unwrap()),
                HttpsClient::with_roots(rustls::RootCertStore::empty())
                    .unwrap()
                    .with_network(held.outbound.clone()),
                held.captures.clone(),
                false,
            ))
            .unwrap();
        let port = proxy.port();
        *held.proxy.lock().unwrap() = Some(proxy);
        let poisoned = catch_unwind(AssertUnwindSafe(|| {
            let _slot = held.vpn_preparation.lock().unwrap();
            panic!("仅毒化当前测试引擎的准备槽");
        }));
        assert!(poisoned.is_err());
        assert_eq!(nodus_core_destroy(handle), INTERNAL_ERROR);
        assert_eq!(nodus_core_last_error_code(), ErrorCode::Internal.value());
        assert!(held.vpn_closed.load(Ordering::Acquire));
        // 旧 Arc 仍然存活，不能依赖 Engine/Manager 的 Drop 才撤销权限。
        assert!(matches!(
            held.outbound
                .configure_network(None, None, crate::outbound::Limits::default()),
            Err(Error::InvalidState)
        ));
        let rebound = std::net::TcpListener::bind(("127.0.0.1", port));
        assert!(rebound.is_ok(), "失败关闭后仍须等待后续代理监听真正释放");
        assert!(held.proxy.lock().unwrap().is_none());
        assert!(engine(handle).is_err());
        assert_eq!(nodus_core_destroy(handle), INVALID_HANDLE);
    }
    #[test]
    fn script_cache_clear_is_owned_and_schema_is_strict() {
        let _gate = TEST_GATE.blocking_lock();
        let owner = TestHandle(nodus_core_create());
        let held = engine(owner.0).unwrap();
        held.busy.store(true, Ordering::Release);
        for (request, success) in [
            (br#"{"operation":"script_cache_clear"}"#.as_slice(), true),
            (
                br#"{"operation":"script_cache_clear","path":"/tmp/forbidden"}"#.as_slice(),
                false,
            ),
        ] {
            // SAFETY: 字节常量同步有效，回复句柄只由本测试的 TestHandle 释放。
            let response = TestHandle(unsafe {
                nodus_core_command_v2(owner.0, request.as_ptr(), request.len())
            });
            assert_ne!(response.0, 0);
            let value: serde_json::Value =
                serde_json::from_slice(&buffer(response.0).unwrap().bytes).unwrap();
            assert_eq!(value["success"], success);
            if !success {
                assert_eq!(value["error_info"]["code"], 1001);
            }
        }
        held.busy.store(false, Ordering::Release);
    }
    #[test]
    fn configuration_check_returns_editor_location_without_applying() {
        let _gate = TEST_GATE.blocking_lock();
        let owner = TestHandle(nodus_core_create());
        let revision = engine(owner.0)
            .unwrap()
            .outbound
            .network_snapshot()
            .unwrap()
            .revision;
        let yaml = "# 中文 😀\n\nsystem-proxy: {port: wrong}\nrules: ['final,direct']";
        let bytes =
            serde_json::to_vec(&serde_json::json!({"operation":"configuration_check","yaml":yaml}))
                .unwrap();
        // SAFETY: 请求字节在同步调用期间保持可读，响应由 TestHandle 唯一释放。
        let response =
            TestHandle(unsafe { nodus_core_command_v2(owner.0, bytes.as_ptr(), bytes.len()) });
        let value: serde_json::Value =
            serde_json::from_slice(&buffer(response.0).unwrap().bytes).unwrap();
        assert_eq!(value["success"], false);
        assert_eq!(value["error_info"]["code"], 1001);
        assert_eq!(value["configuration_issue"]["field"], "system-proxy.port");
        assert_eq!(value["configuration_issue"]["position"]["line"], 3);
        let offset = yaml[..yaml.find("wrong").unwrap()].encode_utf16().count();
        assert_eq!(
            value["configuration_issue"]["position"]["utf16_offset"],
            offset
        );
        assert_eq!(
            engine(owner.0)
                .unwrap()
                .outbound
                .network_snapshot()
                .unwrap()
                .revision,
            revision
        );
        assert_eq!(nodus_core_last_error_code(), 1001);
    }
    #[test]
    fn storage_commands_reject_unentitled_hosts_and_role_injection() {
        let _gate = TEST_GATE.blocking_lock();
        // 公开主配置允许 4 MiB 正文，不能把加载回复误限为普通小命令的 256 KiB。
        assert!(
            output_limit(br#"{"operation":"configuration_load","filename":"config.yaml"}"#)
                >= 2 * crate::configuration::MAX_CONFIG_BYTES + 4096
        );
        std::thread::Builder::new().stack_size(256 * 1024).spawn(|| {
            let owner = TestHandle(nodus_core_create());
            for (request, code) in [
                (serde_json::json!({"operation":"storage_initialize"}), 1001),
                (serde_json::json!({"operation":"storage_initialize","read_only":false}), 1001),
                (serde_json::json!({"operation":"runtime_rules_open","directory":"/tmp/forbidden-nodus-path"}), 1001),
                (serde_json::json!({"operation":"configuration_load","filename":"../escape.yaml"}), 1001),
                (serde_json::json!({"operation":"configuration_save","filename":"../escape.yaml","yaml":"rules: ['final,reject']"}), 1001),
            ] {
                let bytes = serde_json::to_vec(&request).unwrap();
                // SAFETY: 输入字节同步借用，响应句柄只由 TestHandle 释放一次。
                let response = TestHandle(unsafe { nodus_core_command_v2(owner.0, bytes.as_ptr(), bytes.len()) });
                assert_ne!(response.0, 0);
                let json: serde_json::Value = serde_json::from_slice(&buffer(response.0).unwrap().bytes).unwrap();
                assert_eq!(json["success"], false, "{json}");
                assert_eq!(json["error_info"]["code"], code, "{json}");
                assert!(json.get("default_configuration_created").is_none());
                assert!(!json.to_string().contains("Group Containers"));
            }
        }).unwrap().join().unwrap();
    }
    #[test]
    fn candidate_commands_are_safe_on_bounded_host_stack() {
        let _gate = TEST_GATE.blocking_lock();
        std::thread::Builder::new().stack_size(256 * 1024).spawn(|| {
            let owner = TestHandle(nodus_core_create());
            let call = |value: serde_json::Value| {
                let bytes = serde_json::to_vec(&value).unwrap();
                // SAFETY: 字节仅同步借用，返回句柄由测试守卫唯一释放。
                let result = TestHandle(unsafe { nodus_core_command_v2(owner.0, bytes.as_ptr(), bytes.len()) });
                assert_ne!(result.0, 0);
                serde_json::from_slice::<serde_json::Value>(&buffer(result.0).unwrap().bytes).unwrap()
            };
            assert_eq!(call(serde_json::json!({"operation":"vpn_preparation_begin","yaml":"rules: ['final,direct']"}))["success"],true);
            let status=call(serde_json::json!({"operation":"vpn_preparation_status"}));
            assert_eq!(status["vpn_preparation"]["activation_permitted"],false);
            assert_eq!(call(serde_json::json!({"operation":"vpn_preparation_close"}))["success"],true);
        }).unwrap().join().unwrap();
    }
    #[test]
    fn http_request_works_on_bounded_host_stack() {
        let _gate = TEST_GATE.blocking_lock();
        // 小于常见 GCD 工作线程栈；无效方法在真实 HTTPS 请求路径中校验，不需要网络成功。
        let task = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let input = RequestInput {
                    method: "\r".into(),
                    url: "https://127.0.0.1/".into(),
                    headers: Vec::new(),
                    body: String::new(),
                    proxy_port: None,
                };
                assert!(matches!(
                    execute_request(&Engine::default(), input, true),
                    Err(Error::InvalidInput)
                ));
            })
            .unwrap();
        task.join().unwrap();
    }
    #[test]
    fn main_dns_and_geoip_status_use_same_configuration() {
        let _gate = TEST_GATE.blocking_lock();
        let owner = TestHandle(nodus_core_create());
        let handle = owner.0;
        let home = std::env::temp_dir().join(format!(
            "nodus-ffi-storage-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&home).unwrap();
        let manager = engine(handle).unwrap().outbound.clone();

        run_network(async move {
            manager
                .apply_provider_document(
                    Arc::new(crate::configuration::Document::from_yaml(
                        "dns: {enable: true,server: ['127.0.0.1:9']}\nrules: ['final,reject']",
                    )?),
                    HttpsClient::system()?,
                    crate::storage::Resources::default(),
                )
                .await
        })
        .unwrap();
        let call = |value: serde_json::Value| {
            let bytes = serde_json::to_vec(&value).unwrap();
            // SAFETY: 请求缓冲区在同步 ABI 调用期间有效；返回句柄由 TestHandle 唯一释放。
            let output =
                TestHandle(unsafe { nodus_core_command_v2(handle, bytes.as_ptr(), bytes.len()) });
            serde_json::from_slice::<serde_json::Value>(&buffer(output.0).unwrap().bytes).unwrap()
        };
        let result = call(
            serde_json::json!({"operation":"configuration_apply","yaml":"dns: {enable: true, server: ['127.0.0.1:9'],hosts: {pinned.test: 192.0.2.77}}\nrules: ['final,reject']"}),
        );
        assert_eq!(result["success"], true, "{result}");
        assert_eq!(result["configuration_status"]["geoip_status"]["code"], 2410);
        let query =
            serde_json::json!({"operation":"dns_query","name":"pinned.test","query_type":"A"});
        assert_eq!(
            call(query.clone())["dns_answer"]["addresses"][0],
            "192.0.2.77"
        );
        let close = call(serde_json::json!({"operation":"dns_close"}));
        assert_eq!(close["success"], false);
        assert_eq!(close["error_info"]["code"], 9001);
        assert_eq!(call(query)["dns_answer"]["addresses"][0], "192.0.2.77");
        let disabled = call(
            serde_json::json!({"operation":"geoip_lookup","address":"8.8.8.8","language":"en"}),
        );
        assert_eq!(disabled["success"], true);
        assert_eq!(disabled["geoip_status"]["code"], 2410);
        drop(owner);
        std::fs::remove_dir_all(home).unwrap();
    }
    #[tokio::test]
    async fn main_configuration_validation_is_read_only_and_structured() {
        let _gate = TEST_GATE.lock().await;
        let owner = TestHandle(nodus_core_create());
        let initial = engine(owner.0)
            .unwrap()
            .outbound
            .network_snapshot()
            .unwrap()
            .revision;
        for (yaml, expected) in [
            ("rules: ['final,direct']", None),
            ("rules: ['final,missing']", Some(1001u64)),
            // 普通配置校验命令不能授予全量解密权限，失败也不能改变当前网络 revision。
            (
                "rules: ['final,direct']\nmitm: {enabled: false, decrypt-all: true}",
                Some(2002u64),
            ),
            (
                "rules: ['final,direct']\nunknown: secret-value",
                Some(2002u64),
            ),
        ] {
            let request = serde_json::to_vec(
                &serde_json::json!({"operation":"configuration_validate", "yaml":yaml}),
            )
            .unwrap();
            // SAFETY: 不可变请求字节在同步调用期间保持有效，不向内核转移所有权。
            let output = TestHandle(unsafe {
                nodus_core_command_v2(owner.0, request.as_ptr(), request.len())
            });
            assert_ne!(output.0, 0);
            let reply: serde_json::Value =
                serde_json::from_slice(&buffer(output.0).unwrap().bytes).unwrap();
            assert_eq!(reply["success"], expected.is_none());
            if let Some(code) = expected {
                assert_eq!(reply["error_info"]["code"], code);
                assert!(reply.get("configuration").is_none());
            } else {
                assert_eq!(reply["configuration"]["mode"], "rule");
                assert_eq!(reply["configuration"]["rule_count"], 1);
            }
            assert!(!reply.to_string().contains("secret-value"));
            assert_eq!(
                engine(owner.0)
                    .unwrap()
                    .outbound
                    .network_snapshot()
                    .unwrap()
                    .revision,
                initial
            );
        }
    }
    #[tokio::test]
    async fn main_configuration_apply_is_atomic_and_status_is_read_only() {
        let _gate = TEST_GATE.lock().await;
        let owner = TestHandle(nodus_core_create());
        let handle = owner.0;
        // 同步 ABI 从不带 Tokio 上下文的独立系统线程调用，异步宿主必须采用此模式，不能嵌套 block_on。
        let call = move |value: serde_json::Value| async move {
            std::thread::spawn(move || {
                let bytes = serde_json::to_vec(&value).unwrap();
                // SAFETY: 请求字节在同步调用期间有效，返回缓冲区由 TestHandle 确定性回收。
                let output = TestHandle(unsafe {
                    nodus_core_command_v2(handle, bytes.as_ptr(), bytes.len())
                });
                assert_ne!(output.0, 0);
                serde_json::from_slice::<serde_json::Value>(&buffer(output.0).unwrap().bytes)
                    .unwrap()
            })
            .join()
            .unwrap()
        };
        let applied = call(
            serde_json::json!({"operation":"configuration_apply", "yaml":"rules: ['final,reject']"}),
        ).await;
        assert_eq!(applied["success"], true, "{applied}");
        assert_eq!(applied["provider_status"]["active"], false);
        let status = call(serde_json::json!({"operation":"provider_status"})).await;
        assert_eq!(status["success"], true);
        assert_eq!(
            status["provider_status"]["providers"],
            serde_json::json!([])
        );
        let missing =
            call(serde_json::json!({"operation":"provider_refresh", "name":"missing"})).await;
        assert_eq!(missing["success"], false);
        assert_eq!(missing["error_info"]["code"], 9001);
        let revision = applied["network_revision"].as_u64().unwrap();
        let nested = serde_json::to_vec(&serde_json::json!({"operation":"configuration_apply", "yaml":"rules: ['final,direct']"})).unwrap();
        // SAFETY: 请求只读且在整个同步调用存活；此处故意验证异步线程上的准入拒绝。
        let denied =
            TestHandle(unsafe { nodus_core_command_v2(handle, nested.as_ptr(), nested.len()) });
        let denied: serde_json::Value =
            serde_json::from_slice(&buffer(denied.0).unwrap().bytes).unwrap();
        assert_eq!(denied["success"], false);
        assert_eq!(denied["error_info"]["code"], 9001);
        assert_eq!(
            engine(handle)
                .unwrap()
                .outbound
                .network_snapshot()
                .unwrap()
                .revision,
            revision
        );
        for (yaml, code) in [
            ("rules: ['final,unknown']", 1001),
            ("rules: ['final,direct']\nsystem-proxy: {port: 1054}", 1021),
        ] {
            let failed =
                call(serde_json::json!({"operation":"configuration_apply", "yaml":yaml})).await;
            assert_eq!(failed["success"], false);
            assert_eq!(failed["error_info"]["code"], code);
            let status = call(serde_json::json!({"operation":"configuration_status"})).await;
            assert_eq!(status["configuration_status"]["revision"], revision);
            assert_eq!(
                status["configuration_status"]["configuration"]["rule_count"],
                1
            );
            assert!(status.to_string().find("running").is_none());
        }
        let valid = call(
            serde_json::json!({"operation":"configuration_validate", "yaml":"rules: ['final,direct']"}),
        ).await;
        assert_eq!(valid["success"], true);
        assert_eq!(
            engine(owner.0)
                .unwrap()
                .outbound
                .network_snapshot()
                .unwrap()
                .revision,
            revision
        );
    }
    #[tokio::test]
    async fn provider_commands_use_real_tls_and_keep_process_rules_ignored() {
        let _gate = TEST_GATE.lock().await;
        let fixture = crate::test_fixture_https::fixture().await;
        let home = std::env::temp_dir().join(format!(
            "nodus-ffi-provider-{}",
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&home).unwrap();
        let owner = TestHandle(nodus_core_create());
        let manager = engine(owner.0).unwrap().outbound.clone();
        let mut roots = rustls::RootCertStore::empty();
        roots.add(fixture.root.clone()).unwrap();
        let client = crate::https::HttpsClient::with_roots(roots).unwrap();
        let resource = home.join("one.yaml");
        std::fs::write(
            &resource,
            "rules: ['domain,one.example', 'process-name,WeChat']",
        )
        .unwrap();
        let yaml = format!(
            "rule-providers: [{{name: one, url: 'https://localhost:{}/provider-one', policy: direct, enable: true}}]\nrules: ['domain,localhost,direct','final,reject']",
            fixture.port
        );
        manager
            .apply_provider_document(
                Arc::new(crate::configuration::Document::from_yaml(&yaml).unwrap()),
                client,
                crate::storage::Resources {
                    rules: vec![crate::storage::NamedFile {
                        name: "one".into(),
                        path: resource.clone(),
                    }],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let handle = owner.0;
        // Tokio 的 blocking 线程仍带运行时上下文；再进入独立系统线程调用同步 ABI。
        let call = move |value: serde_json::Value| async move {
            tokio::task::spawn_blocking(move || {
                std::thread::spawn(move || {
                    let request = serde_json::to_vec(&value).unwrap();
                    // SAFETY: 不变请求字节真实有效；响应句柄按所有权释放。
                    let output = TestHandle(unsafe {
                        nodus_core_command_v2(handle, request.as_ptr(), request.len())
                    });
                    assert_ne!(output.0, 0);
                    serde_json::from_slice::<serde_json::Value>(&buffer(output.0).unwrap().bytes)
                        .unwrap()
                })
                .join()
                .unwrap()
            })
            .await
            .unwrap()
        };
        let status = call(serde_json::json!({"operation":"provider_status"})).await;
        assert_eq!(status["provider_status"]["providers"][0]["ignored"], 1);
        let revision = manager.network_snapshot().unwrap().revision;
        let updated = call(serde_json::json!({"operation":"provider_refresh","name":"one"})).await;
        assert_eq!(updated["success"], false);
        assert_eq!(updated["error_info"]["code"], 1021);
        assert_eq!(manager.network_snapshot().unwrap().revision, revision);
        assert_eq!(
            std::fs::read_to_string(&resource).unwrap(),
            "rules: ['domain,one.example', 'process-name,WeChat']"
        );
        assert_eq!(
            fixture
                .provider
                .requests
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        manager.stop().await.unwrap();
        std::fs::remove_dir_all(home).unwrap();
    }
    fn last_info() -> Option<ErrorInfo> {
        let mut bytes = [0; crate::error::MAX_ERROR_BYTES];
        // SAFETY: 固定数组在本同步调用中可写，无并发访问。
        let count = unsafe { nodus_core_last_error_copy(bytes.as_mut_ptr(), bytes.len()) };
        (count != 0).then(|| ErrorInfo::from_json(&bytes[..count]).unwrap())
    }
    #[test]
    fn invalid_internal_error_encoding_does_not_become_no_error() {
        set_last_error(ErrorInfo::new(ErrorCode::ResourceBudgetExceeded));
        let mut bytes = [0; crate::error::MAX_ERROR_BYTES];
        // SAFETY: 独占固定数组在整个同步复制期间可写。
        let count = unsafe { nodus_core_last_error_copy(bytes.as_mut_ptr(), bytes.len()) };
        let info = ErrorInfo::from_json(&bytes[..count]).unwrap();
        assert_eq!(info.code, ErrorCode::SerializationFailed);
        clear_last_error();
    }
    #[tokio::test]
    async fn error_protocol_is_thread_local_and_success_clears_stale_error() {
        let _gate = TEST_GATE.lock().await;
        let handle = nodus_core_create();
        assert_ne!(handle, 0);
        // SAFETY: 空指针必须在解引用前拒绝，无实际输入借用。
        // SAFETY: 空输入在解引用前拒绝。
        let rejected = unsafe { nodus_core_command(handle, std::ptr::null(), 1) };
        assert_eq!(rejected, 0);
        assert_eq!(last_info().unwrap().code, ErrorCode::InvalidInput);
        let thread = std::thread::spawn(|| {
            assert!(last_info().is_none());
            assert_eq!(nodus_core_start(0), INVALID_HANDLE);
            assert_eq!(last_info().unwrap().code, ErrorCode::InvalidHandle);
            assert_eq!(nodus_core_last_error_code(), 1201);
        });
        thread.join().unwrap();
        assert_eq!(last_info().unwrap().code, ErrorCode::InvalidInput);
        let bytes = br#"{"operation":"tun_start"}"#;
        // SAFETY: 常量字节在整个同步调用期间可读且不可变。
        let output = unsafe { nodus_core_command_v2(handle, bytes.as_ptr(), bytes.len()) };
        assert_ne!(output, 0);
        assert_eq!(last_info().unwrap().code, ErrorCode::Unsupported);
        let reply: serde_json::Value =
            serde_json::from_slice(&buffer(output).unwrap().bytes).unwrap();
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error_info"]["code"], 1021);
        assert_eq!(nodus_core_destroy(output), OK);
        assert!(last_info().is_none());
        assert_eq!(nodus_core_destroy(handle), OK);
    }
    #[tokio::test]
    async fn error_query_remains_available_when_response_registry_is_full() {
        let _gate = TEST_GATE.lock().await;
        let owner = TestHandle(nodus_core_create());
        let handle = owner.0;
        assert_ne!(handle, 0);
        let filled: Vec<_> = (0..63)
            .map(|_| TestHandle(store(vec![1]).unwrap()))
            .collect();
        let bytes = br#"{"operation":"tun_start"}"#;
        // SAFETY: 常量字节在整个同步调用期间有效。
        // SAFETY: 常量输入在同步调用期间可读且不可变。
        let rejected = unsafe { nodus_core_command(handle, bytes.as_ptr(), bytes.len()) };
        assert_eq!(rejected, 0);
        assert_eq!(last_info().unwrap().code, ErrorCode::ResourceLimit);
        // 查询不创建句柄，也不因容量不足而写入或覆盖原错误。
        let mut untouched = [0xab; 1];
        // SAFETY: 长度为一的数组有效且独占可写；容量不足必须保持原字节。
        let required = unsafe { nodus_core_last_error_copy(untouched.as_mut_ptr(), 1) };
        assert!(required > 1);
        assert_eq!(untouched, [0xab]);
        assert_eq!(last_info().unwrap().code, ErrorCode::ResourceLimit);
        for output in filled {
            assert_eq!(output.release(), OK);
        }
        assert!(last_info().is_none());
        assert_eq!(owner.release(), OK);
    }
    /// 仅供测试：先登记所有权再断言，失败展开时也归还响应名额与预算。
    struct TestHandle(u64);
    impl TestHandle {
        fn release(mut self) -> i32 {
            let handle = std::mem::take(&mut self.0);
            nodus_core_destroy(handle)
        }
    }
    impl Drop for TestHandle {
        fn drop(&mut self) {
            if self.0 != 0 {
                let _ = nodus_core_destroy(self.0);
            }
        }
    }
    #[tokio::test]
    async fn legacy_validation_order_preserves_handle_before_json() {
        let _gate = TEST_GATE.lock().await;
        let owner = TestHandle(nodus_core_create());
        assert_ne!(owner.0, 0);
        // 预期来自旧接口的句柄先行契约；非法 JSON 不得绕过句柄验证。
        for (handle, input, expected) in [
            (0, b"not-json".as_slice(), ErrorCode::InvalidHandle),
            (
                0,
                br#"{"operation":"tun_start"}"#.as_slice(),
                ErrorCode::InvalidHandle,
            ),
            (owner.0, b"not-json".as_slice(), ErrorCode::InvalidInput),
            (
                owner.0,
                br#"{"operation":"tun_start"}"#.as_slice(),
                ErrorCode::Unsupported,
            ),
        ] {
            // SAFETY: 测试字节在整个同步调用期间有效且不可变。
            let output =
                TestHandle(unsafe { nodus_core_command(handle, input.as_ptr(), input.len()) });
            assert_ne!(output.0, 0);
            assert_eq!(nodus_core_last_error_code(), expected.value());
            let reply: serde_json::Value =
                serde_json::from_slice(&buffer(output.0).unwrap().bytes).unwrap();
            assert_eq!(reply["success"], false);
            assert_eq!(reply["error_info"]["code"], expected.value());
        }
    }
    #[test]
    fn failed_test_assertion_returns_owned_response_capacity() {
        let _gate = TEST_GATE.blocking_lock();
        // 模拟断言展开：测试专用守卫必须释放已登记的引擎和响应，防止连锁失败。
        let failed = std::panic::catch_unwind(|| {
            let owner = TestHandle(nodus_core_create());
            assert_ne!(owner.0, 0);
            let output = TestHandle(store(vec![1]).unwrap());
            assert_ne!(output.0, 0);
            panic!("模拟测试断言失败，验证守卫回收");
        });
        assert!(failed.is_err());
        // 释放后完整的 64 个响应名额都可重新申请，并逐一验证正常释放。
        let outputs: Vec<_> = (0..64)
            .map(|_| TestHandle(store(vec![1]).unwrap()))
            .collect();
        assert!(store(vec![1]).is_none());
        for output in outputs {
            assert_eq!(output.release(), OK);
        }
    }
    // 配额测试需要独占同一进程的 FFI 注册表；业务实现不增加全局执行锁。
    static TEST_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    #[test]
    fn abi_rejects_invalid_and_destroyed_handles() {
        let _gate = TEST_GATE.blocking_lock();
        let handle = nodus_core_create();
        assert_ne!(handle, 0);
        assert_eq!(nodus_core_start(handle), UNSUPPORTED);
        assert_eq!(nodus_core_destroy(handle), OK);
        assert_eq!(nodus_core_destroy(handle), INVALID_HANDLE);
        assert_eq!(nodus_core_start(handle), INVALID_HANDLE);
        // 保留既有能力，位 29 只表示候选资产准备；仍精确检查完整掩码。
        assert_eq!(
            nodus_core_capabilities(),
            if cfg!(target_os = "macos") {
                3221225471
            } else {
                3154116607
            }
        );
    }
    #[test]
    fn concurrent_owners_release_their_own_handles() {
        let _gate = TEST_GATE.blocking_lock();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| {
                    for _ in 0..100 {
                        let handle = nodus_core_create();
                        assert_ne!(handle, 0);
                        assert_eq!(nodus_core_destroy(handle), OK);
                        assert_eq!(nodus_core_start(handle), INVALID_HANDLE);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
    }
    #[test]
    fn output_buffers_obey_budget_and_handle_type() {
        let _gate = TEST_GATE.blocking_lock();
        let handle = store(vec![1; 32]).unwrap();
        assert_eq!(nodus_core_start(handle), INVALID_HANDLE);
        assert_eq!(nodus_core_buffer_length(handle), 32);
        assert!(store(vec![0; MAX_OUTPUT + 1]).is_none());
        assert_eq!(nodus_core_destroy(handle), OK);
        assert_eq!(nodus_core_buffer_length(handle), 0);
    }
    #[test]
    fn admission_failure_keeps_tcp_bytes_and_returns_reservations() {
        let _gate = TEST_GATE.blocking_lock();
        for exhaust_memory in [false, true] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let peer = std::thread::spawn(move || {
                use std::io::Write;
                let (mut socket, _) = listener.accept().unwrap();
                socket.write_all(b"FIRSTSECOND").unwrap();
            });
            let handle = nodus_core_create();
            let request = serde_json::to_vec(&serde_json::json!({
                "operation":"outbound_open", "address":address.to_string(),
                "transport":"tcp", "policy":"direct", "timeout_ms":1000
            }))
            .unwrap();
            let id = command(handle, &request).unwrap().outbound_id.unwrap();
            peer.join().unwrap();
            let mut buffers = Vec::new();
            let memory = if exhaust_memory {
                Some(OUTPUT.reserve(MAX_OUTPUT_MEMORY).unwrap())
            } else {
                for _ in 0..63 {
                    buffers.push(store(vec![1]).unwrap());
                }
                None
            };
            let request = serde_json::to_vec(&serde_json::json!({
                "operation":"outbound_read", "id":id, "maximum":11, "timeout_ms":1000
            }))
            .unwrap();
            // SAFETY: 同步调用期间输入存活且只读，FFI 不保存该指针。
            let refused = unsafe { nodus_core_command(handle, request.as_ptr(), request.len()) };
            assert_eq!(refused, 0);
            drop(memory);
            if let Some(buffer) = buffers.pop() {
                assert_eq!(nodus_core_destroy(buffer), OK);
            }
            // SAFETY: 输入指针契约同上，释放配额后重试不会改变输入所有权。
            let output = unsafe { nodus_core_command(handle, request.as_ptr(), request.len()) };
            let bytes = buffer(output).unwrap();
            let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                STANDARD.decode(reply["body"].as_str().unwrap()).unwrap(),
                b"FIRSTSECOND"
            );
            drop(bytes);
            assert_eq!(nodus_core_destroy(output), OK);
            for buffer in buffers {
                assert_eq!(nodus_core_destroy(buffer), OK);
            }
            assert_eq!(nodus_core_destroy(handle), OK);
        }
        // JSON 转义后的操作名称同样采用大响应上限，不能退回小预算后再执行。
        assert_eq!(output_limit(br#"{"operation":"requ\u0065st"}"#), MAX_OUTPUT);
        // 守卫未发布时归还名额和预算；完成后只持有实际容量。
        for _ in 0..128 {
            drop(OutputReservation::new(SMALL_OUTPUT).unwrap());
        }
        let output = store(vec![1; 32]).unwrap();
        let extra = OUTPUT.reserve(MAX_OUTPUT_MEMORY - 32).unwrap();
        drop(extra);
        assert_eq!(nodus_core_destroy(output), OK);
    }
    #[test]
    fn admission_failure_keeps_tun_return_packet() {
        let _gate = TEST_GATE.blocking_lock();
        for exhaust_memory in [false, true] {
            let peer = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            peer.set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let destination = peer.local_addr().unwrap();
            let peer = std::thread::spawn(move || {
                let mut bytes = [0; 32];
                let (length, source) = peer.recv_from(&mut bytes).unwrap();
                peer.send_to(&bytes[..length], source).unwrap();
            });
            let handle = nodus_core_create();
            let engine = engine(handle).unwrap();
            engine
                .outbound
                .configure_network(
                    None,
                    Some(Arc::new(
                        crate::routing::Router::compile(crate::routing::Config {
                            mode: crate::routing::Mode::Direct,
                            ..Default::default()
                        })
                        .unwrap(),
                    )),
                    crate::outbound::Limits::default(),
                )
                .unwrap();
            let session = Arc::new(
                crate::tun::udp::Session::new(
                    engine.outbound.clone(),
                    crate::tun::udp::Options::default(),
                )
                .unwrap(),
            );
            *engine.tun_udp.lock().unwrap() = Some(ManagedTun::Udp(session.clone()));
            let packet = crate::tun::udp::encode(
                "192.0.2.1:12345".parse().unwrap(),
                destination,
                b"retained",
                1500,
            )
            .unwrap();
            runtime()
                .unwrap()
                .block_on(session.submit(&packet.bytes, None))
                .unwrap();
            peer.join().unwrap();
            let mut buffers = Vec::new();
            let memory = if exhaust_memory {
                Some(OUTPUT.reserve(MAX_OUTPUT_MEMORY).unwrap())
            } else {
                for _ in 0..63 {
                    buffers.push(TestHandle(store(vec![1]).unwrap()));
                }
                None
            };
            let mut output = 123;
            // SAFETY: output 对齐、可写且调用期间独占，不跨语言保存地址。
            let status = unsafe { nodus_core_tun_receive(handle, &mut output) };
            assert_eq!(status, if exhaust_memory { 8002 } else { 8001 });
            assert_eq!(output, 0);
            drop(memory);
            if let Some(buffer) = buffers.pop() {
                assert_eq!(buffer.release(), OK);
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            loop {
                // SAFETY: 重试期间同样独占这个有效 u64，不与内核缓冲区重叠。
                let status = unsafe { nodus_core_tun_receive(handle, &mut output) };
                if status == 0 {
                    break;
                }
                assert_eq!(status, 11);
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let packet = buffer(output).unwrap();
            assert!(packet.ends_with(b"retained"));
            drop(packet);
            nodus_core_destroy(output);
            for buffer in buffers {
                assert_eq!(buffer.release(), OK);
            }
            nodus_core_destroy(handle);
        }
    }
    #[tokio::test]
    async fn nested_runtime_does_not_detach_failed_http_calls() {
        let _gate = TEST_GATE.lock().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let handle = nodus_core_create();
        engine(handle)
            .unwrap()
            .outbound
            .configure_network(
                None,
                Some(Arc::new(
                    crate::routing::Router::compile(crate::routing::Config {
                        mode: crate::routing::Mode::Direct,
                        ..Default::default()
                    })
                    .unwrap(),
                )),
                crate::outbound::Limits::default(),
            )
            .unwrap();
        let request=serde_json::to_vec(&serde_json::json!({"operation":"request","method":"GET","url":format!("http://{}/",listener.local_addr().unwrap())})).unwrap();
        // SAFETY: JSON 输入在整个同步调用期间存活、只读，不保留任何跨语言借用。
        let response =
            TestHandle(unsafe { nodus_core_command(handle, request.as_ptr(), request.len()) });
        // 受控拒绝应返回结构化状态错误；不再依赖嵌套 block_on panic 被 ABI 捕获为零句柄。
        let reply: serde_json::Value =
            serde_json::from_slice(&buffer(response.0).unwrap().bytes).unwrap();
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error_info"]["code"], 9001);
        assert_eq!(response.release(), OK);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        );
        assert_eq!(nodus_core_destroy(handle), 0);
    }
}
