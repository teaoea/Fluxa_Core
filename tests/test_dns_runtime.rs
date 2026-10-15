//! 测试文件：tests/test_dns_runtime.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/dns/runtime.rs
//! 测试内容：
//! - background_budget_failure_is_not_dns_timeout：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - historical_admission_failure_does_not_poison_new_query：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - same_poll_failure_cannot_be_returned_as_success：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - concurrent_queries_observe_event_without_clearing_each_other：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - task_budget_exhaustion_is_retryable_after_lease_release：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - revoked_runtime_cannot_recover_or_spawn：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - all_transports_recover_after_worker_limit：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    #[tokio::test]
    async fn background_budget_failure_is_not_dns_timeout() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        let expected = Error::ResourceBudgetExceeded {
            actual: TASK_BYTES as u64,
            limit: 16 * 1024 * 1024,
        };
        assert_eq!(
            runtime
                .guard(async {
                    runtime.tasks.failed(expected);
                    std::future::pending::<Result<()>>().await
                })
                .await,
            Err(expected)
        );
        runtime.stop().await.unwrap();
    }
    #[tokio::test]
    async fn historical_admission_failure_does_not_poison_new_query() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        runtime.tasks.failed(Error::ResourceLimit);
        assert_eq!(runtime.guard(async { Ok(()) }).await, Ok(()));
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn same_poll_failure_cannot_be_returned_as_success() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        assert_eq!(
            runtime
                .guard(async {
                    runtime.tasks.failed(Error::ResourceLimit);
                    Ok(())
                })
                .await,
            Err(Error::ResourceLimit)
        );
        assert_eq!(runtime.guard(async { Ok(()) }).await, Ok(()));
    }

    #[tokio::test]
    async fn concurrent_queries_observe_event_without_clearing_each_other() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        let first = runtime.guard(std::future::pending::<Result<()>>());
        let second = runtime.guard(async {
            runtime.tasks.failed(Error::ResourceLimit);
            // 新查询开始后不能清除仍在执行的旧查询所需的故障事件。
            assert_eq!(runtime.guard(async { Ok(()) }).await, Ok(()));
            Ok(())
        });
        let results = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(first, second)
        })
        .await
        .unwrap();
        assert_eq!(
            results,
            (Err(Error::ResourceLimit), Err(Error::ResourceLimit))
        );
    }

    #[tokio::test]
    async fn task_budget_exhaustion_is_retryable_after_lease_release() {
        // 隔离预算但走生产准入流程，不耗尽其他并发测试的全局预算。
        static BUDGET: ByteBudget = ByteBudget::new(TASK_BYTES);
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        let lease = BUDGET.reserve(TASK_BYTES).unwrap();
        let mut handle = runtime.create_handle();
        let error = Error::ResourceBudgetExceeded {
            actual: TASK_BYTES as u64,
            limit: TASK_BYTES as u64,
        };
        assert_eq!(
            runtime
                .guard(async {
                    handle.spawn_with_budget(std::future::pending(), &BUDGET);
                    Ok(())
                })
                .await,
            Err(error)
        );
        assert_eq!(BUDGET.usage().0, TASK_BYTES);
        drop(lease);
        let (finished, received) = tokio::sync::oneshot::channel();
        runtime
            .guard(async {
                handle.spawn_with_budget(
                    async move {
                        finished.send(()).unwrap();
                    },
                    &BUDGET,
                );
                received.await.map_err(|_| Error::Internal)
            })
            .await
            .unwrap();
        runtime.stop().await.unwrap();
        assert_eq!(BUDGET.usage().0, 0);
    }

    #[tokio::test]
    async fn revoked_runtime_cannot_recover_or_spawn() {
        let authority = Authority::new().unwrap();
        let runtime = ManagedRuntime::new(authority.clone());
        authority.revoke().unwrap();
        assert_eq!(
            runtime.guard(async { Ok(()) }).await,
            Err(Error::InvalidState)
        );
        runtime.create_handle().spawn_bg(async {
            panic!("关闭后不得执行驱动");
        });
        assert!(runtime.tasks.workers.lock().unwrap().is_empty());
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn all_transports_recover_after_worker_limit() {
        use super::super::super::{Config, Query, Resolver, Server};
        static BUDGET: ByteBudget = ByteBudget::new(MAX_BACKGROUND_TASKS * TASK_BYTES);
        let service = super::super::test_upstream::fixture().await;
        let mut roots = rustls::RootCertStore::empty();
        roots.add(service.root.clone()).unwrap();
        let trust = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
        for server in [
            Server::Udp {
                address: format!("127.0.0.1:{}", service.port),
            },
            Server::Tcp {
                address: format!("127.0.0.1:{}", service.port),
            },
            Server::Dot {
                address: format!("127.0.0.1:{}", service.dot_port),
                server_name: "localhost".into(),
            },
            Server::Doh {
                url: format!("https://localhost:{}/dns-query", service.doh_port),
                bootstrap: format!("127.0.0.1:{}", service.doh_port),
            },
        ] {
            let resolver = Arc::new(
                Resolver::with_tls_config(
                    Config {
                        servers: vec![server],
                        timeout_ms: 1000,
                        dnssec: false,
                        fake_ip: None,
                        service: None,
                    },
                    trust.clone(),
                )
                .unwrap(),
            );
            let runtime = resolver
                .resources
                .read()
                .await
                .as_ref()
                .unwrap()
                .runtime
                .clone();
            let mut handle = runtime.create_handle();
            for _ in 0..MAX_BACKGROUND_TASKS {
                handle.spawn_with_budget(std::future::pending(), &BUDGET);
            }
            assert_eq!(
                runtime.tasks.workers.lock().unwrap().len(),
                MAX_BACKGROUND_TASKS
            );
            let query = || Query {
                name: "example.test".into(),
                query_type: "A".into(),
            };
            // 真正的 Hickory 驱动准入失败，必须返回 8001，不能误报为 DNS 超时。
            assert_eq!(
                resolver.resolve(query()).await.unwrap_err(),
                Error::ResourceLimit
            );
            runtime.stop().await.unwrap();
            assert_eq!(BUDGET.usage().0, 0);
            // 同一个解析器、上游和会话；不清缓存、不重建 Resolver，也不要求前端介入。
            let answer = resolver.resolve(query()).await.unwrap();
            assert_eq!(answer.addresses, ["192.0.2.44"]);
            assert!(!answer.from_cache);
            assert!(resolver.resolve(query()).await.unwrap().from_cache);
            resolver.close().await.unwrap();
            assert_eq!(
                resolver.resolve(query()).await.unwrap_err(),
                Error::InvalidState
            );
        }
    }
}
