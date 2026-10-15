//! 测试文件：tests/test_ffi.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/ffi.rs
//! 测试内容：
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
    fn last_info() -> Option<ErrorInfo> {
        let mut bytes = [0; crate::error::MAX_ERROR_BYTES];
        // SAFETY: 固定数组在本同步调用中可写，无并发访问。
        let count = unsafe { fluxa_engine_last_error_copy(bytes.as_mut_ptr(), bytes.len()) };
        (count != 0).then(|| ErrorInfo::from_json(&bytes[..count]).unwrap())
    }
    #[test]
    fn invalid_internal_error_encoding_does_not_become_no_error() {
        set_last_error(ErrorInfo::new(ErrorCode::ResourceBudgetExceeded));
        let mut bytes = [0; crate::error::MAX_ERROR_BYTES];
        // SAFETY: 独占固定数组在整个同步复制期间可写。
        let count = unsafe { fluxa_engine_last_error_copy(bytes.as_mut_ptr(), bytes.len()) };
        let info = ErrorInfo::from_json(&bytes[..count]).unwrap();
        assert_eq!(info.code, ErrorCode::SerializationFailed);
        clear_last_error();
    }
    #[tokio::test]
    async fn error_protocol_is_thread_local_and_success_clears_stale_error() {
        let _gate = TEST_GATE.lock().await;
        let handle = fluxa_engine_create();
        assert_ne!(handle, 0);
        // SAFETY: 空指针必须在解引用前拒绝，无实际输入借用。
        // SAFETY: 空输入在解引用前拒绝。
        let rejected = unsafe { fluxa_engine_command(handle, std::ptr::null(), 1) };
        assert_eq!(rejected, 0);
        assert_eq!(last_info().unwrap().code, ErrorCode::InvalidInput);
        let thread = std::thread::spawn(|| {
            assert!(last_info().is_none());
            assert_eq!(fluxa_engine_start(0), INVALID_HANDLE);
            assert_eq!(last_info().unwrap().code, ErrorCode::InvalidHandle);
            assert_eq!(fluxa_engine_last_error_code(), 1201);
        });
        thread.join().unwrap();
        assert_eq!(last_info().unwrap().code, ErrorCode::InvalidInput);
        let bytes = br#"{"operation":"tun_start"}"#;
        // SAFETY: 常量字节在整个同步调用期间可读且不可变。
        let output = unsafe { fluxa_engine_command_v2(handle, bytes.as_ptr(), bytes.len()) };
        assert_ne!(output, 0);
        assert_eq!(last_info().unwrap().code, ErrorCode::Unsupported);
        let reply: serde_json::Value =
            serde_json::from_slice(&buffer(output).unwrap().bytes).unwrap();
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error_info"]["code"], 1021);
        assert_eq!(fluxa_engine_destroy(output), OK);
        assert!(last_info().is_none());
        assert_eq!(fluxa_engine_destroy(handle), OK);
    }
    #[tokio::test]
    async fn error_query_remains_available_when_response_registry_is_full() {
        let _gate = TEST_GATE.lock().await;
        let owner = TestHandle(fluxa_engine_create());
        let handle = owner.0;
        assert_ne!(handle, 0);
        let filled: Vec<_> = (0..63)
            .map(|_| TestHandle(store(vec![1]).unwrap()))
            .collect();
        let bytes = br#"{"operation":"tun_start"}"#;
        // SAFETY: 常量字节在整个同步调用期间有效。
        // SAFETY: 常量输入在同步调用期间可读且不可变。
        let rejected = unsafe { fluxa_engine_command(handle, bytes.as_ptr(), bytes.len()) };
        assert_eq!(rejected, 0);
        assert_eq!(last_info().unwrap().code, ErrorCode::ResourceLimit);
        // 查询不创建句柄，也不因容量不足而写入或覆盖原错误。
        let mut untouched = [0xab; 1];
        // SAFETY: 长度为一的数组有效且独占可写；容量不足必须保持原字节。
        let required = unsafe { fluxa_engine_last_error_copy(untouched.as_mut_ptr(), 1) };
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
            fluxa_engine_destroy(handle)
        }
    }
    impl Drop for TestHandle {
        fn drop(&mut self) {
            if self.0 != 0 {
                let _ = fluxa_engine_destroy(self.0);
            }
        }
    }
    #[tokio::test]
    async fn legacy_validation_order_preserves_handle_before_json() {
        let _gate = TEST_GATE.lock().await;
        let owner = TestHandle(fluxa_engine_create());
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
                TestHandle(unsafe { fluxa_engine_command(handle, input.as_ptr(), input.len()) });
            assert_ne!(output.0, 0);
            assert_eq!(fluxa_engine_last_error_code(), expected.value());
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
            let owner = TestHandle(fluxa_engine_create());
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
        let handle = fluxa_engine_create();
        assert_ne!(handle, 0);
        assert_eq!(fluxa_engine_start(handle), UNSUPPORTED);
        assert_eq!(fluxa_engine_destroy(handle), OK);
        assert_eq!(fluxa_engine_destroy(handle), INVALID_HANDLE);
        assert_eq!(fluxa_engine_start(handle), INVALID_HANDLE);
        // 原位 0–25 保留；macOS 独立安全 IPC 明确新增位 26，仍精确检查掩码。
        assert_eq!(
            fluxa_engine_capabilities(),
            if cfg!(target_os = "macos") {
                268435455
            } else {
                201326591
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
                        let handle = fluxa_engine_create();
                        assert_ne!(handle, 0);
                        assert_eq!(fluxa_engine_destroy(handle), OK);
                        assert_eq!(fluxa_engine_start(handle), INVALID_HANDLE);
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
        assert_eq!(fluxa_engine_start(handle), INVALID_HANDLE);
        assert_eq!(fluxa_engine_buffer_length(handle), 32);
        assert!(store(vec![0; MAX_OUTPUT + 1]).is_none());
        assert_eq!(fluxa_engine_destroy(handle), OK);
        assert_eq!(fluxa_engine_buffer_length(handle), 0);
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
            let handle = fluxa_engine_create();
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
            let refused = unsafe { fluxa_engine_command(handle, request.as_ptr(), request.len()) };
            assert_eq!(refused, 0);
            drop(memory);
            if let Some(buffer) = buffers.pop() {
                assert_eq!(fluxa_engine_destroy(buffer), OK);
            }
            // SAFETY: 输入指针契约同上，释放配额后重试不会改变输入所有权。
            let output = unsafe { fluxa_engine_command(handle, request.as_ptr(), request.len()) };
            let bytes = buffer(output).unwrap();
            let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                STANDARD.decode(reply["body"].as_str().unwrap()).unwrap(),
                b"FIRSTSECOND"
            );
            drop(bytes);
            assert_eq!(fluxa_engine_destroy(output), OK);
            for buffer in buffers {
                assert_eq!(fluxa_engine_destroy(buffer), OK);
            }
            assert_eq!(fluxa_engine_destroy(handle), OK);
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
        assert_eq!(fluxa_engine_destroy(output), OK);
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
            let handle = fluxa_engine_create();
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
            *engine.tun_udp.lock().unwrap() = Some(session.clone());
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
            let status = unsafe { fluxa_engine_tun_receive(handle, &mut output) };
            assert_eq!(status, if exhaust_memory { 8002 } else { 8001 });
            assert_eq!(output, 0);
            drop(memory);
            if let Some(buffer) = buffers.pop() {
                assert_eq!(buffer.release(), OK);
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            loop {
                // SAFETY: 重试期间同样独占这个有效 u64，不与内核缓冲区重叠。
                let status = unsafe { fluxa_engine_tun_receive(handle, &mut output) };
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
            fluxa_engine_destroy(output);
            for buffer in buffers {
                assert_eq!(buffer.release(), OK);
            }
            fluxa_engine_destroy(handle);
        }
    }
    #[tokio::test]
    async fn nested_runtime_does_not_detach_failed_http_calls() {
        let _gate = TEST_GATE.lock().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let handle = fluxa_engine_create();
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
        let response = unsafe { fluxa_engine_command(handle, request.as_ptr(), request.len()) };
        assert_eq!(response, 0);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        );
        assert_eq!(fluxa_engine_destroy(handle), 0);
    }
}
