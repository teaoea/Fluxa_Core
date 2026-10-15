//! 测试文件：tests/test_routing_background.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/background.rs
//! 测试内容：
//! - queued_input_is_also_charged_to_shared_rule_pool：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - async_generation_keeps_connections_old_until_commit_and_bounds_workers：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::outbound::{Family, RoutedTarget};
    use crate::routing::{Match, Policy, Rule, Target, Transport};
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    fn sources(policy: Policy) -> Vec<Source> {
        vec![Source {
            id: "local".into(),
            layer: super::super::super::runtime::Layer::Local,
            enabled: true,
            rules: vec![Rule {
                protocol: None,
                r#type: Match::IpCidr,
                value: "127.0.0.0/8".into(),
                policy,
                outbound: None,
            }],
        }]
    }
    fn path() -> PathBuf {
        // 生命周期编号只保证本进程唯一；测试目录增加进程与时间，不能读入旧测试产物。
        std::env::temp_dir().join(format!(
            "fluxa-background-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            crate::lifecycle::identifier().unwrap()
        ))
    }
    async fn finished(service: &Service, id: u64) -> Job {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let job = service.job(id).unwrap();
            if job.phase != Phase::Compiling {
                return job;
            }
            assert!(Instant::now() < deadline, "后台任务必须在测试期限内完成");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
    #[test]
    fn queued_input_is_also_charged_to_shared_rule_pool() {
        let before = super::super::super::runtime::memory_usage().reserved_bytes;
        let mut parsed =
            GenerationInput::from_yaml("format_version: 1\nsettings: {}\nsources: []\n").unwrap();
        let prepared = PreparedInput {
            settings: Config::default(),
            sources: Vec::new(),
            _memory: parsed.take_memory(),
        };
        drop(parsed);
        assert!(
            super::super::super::runtime::memory_usage().reserved_bytes > before,
            "解析对象离开后，来源消费凭证仍持有配额"
        );
        drop(prepared);
        assert_eq!(
            super::super::super::runtime::memory_usage().reserved_bytes,
            before
        );
        let input = INPUTS.reserve(4096).unwrap();
        assert_eq!(
            super::super::super::runtime::memory_usage().reserved_bytes,
            before + 4096
        );
        drop(input);
        assert_eq!(
            super::super::super::runtime::memory_usage().reserved_bytes,
            before
        );
        let usage = super::super::super::runtime::memory_usage();
        let occupied = super::super::super::runtime::MEMORY
            .reserve(usage.limit_bytes - usage.reserved_bytes)
            .unwrap();
        assert!(matches!(
            INPUTS.reserve(1),
            Err(Error::ResourceBudgetExceeded { .. })
        ));
        drop(occupied);
        assert!(INPUTS.reserve(4096).is_ok(), "总池失败也归还输入子配额");
    }
    #[tokio::test]
    async fn async_generation_keeps_connections_old_until_commit_and_bounds_workers() {
        let directory = path();
        let manager = Arc::new(Manager::default());
        let store = Store::open(&directory).unwrap();
        store
            .update_network(Config::default(), sources(Policy::Direct), &manager)
            .unwrap();
        let service = Service::new(store, Arc::clone(&manager));
        let old = service.store.snapshot().unwrap();
        let (send, receive) = mpsc::channel();
        let job = service
            .spawn(
                move || {
                    receive.recv().unwrap();
                    Ok((Config::default(), sources(Policy::Reject)))
                },
                INPUTS.reserve(1).unwrap(),
            )
            .unwrap();
        assert_eq!(service.job(job.id).unwrap().phase, Phase::Compiling);
        assert!(matches!(
            service.start(Config::default(), sources(Policy::Reject)),
            Err(Error::InvalidState)
        ));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target = Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port: listener.local_addr().unwrap().port(),
            transport: Transport::Tcp,
        };
        let request = RoutedTarget {
            target: target.clone(),
            timeout_ms: 1000,
            family: Family::Ipv4,
            mapping_generation: None,
        };
        let opened = manager.connect_configured(request.clone()).await.unwrap();
        let (mut peer, _) = listener.accept().await.unwrap();
        assert_eq!(opened.decision.rule_version, Some(1));
        assert_eq!(service.store.snapshot().unwrap().document().version, 1);
        for _ in 0..100 {
            assert_eq!(
                manager
                    .network_snapshot()
                    .unwrap()
                    .router
                    .as_ref()
                    .unwrap()
                    .evaluate(target.clone())
                    .unwrap()
                    .policy,
                Policy::Direct
            );
        }
        send.send(()).unwrap();
        assert_eq!(finished(&service, job.id).await.version, Some(2));
        assert!(matches!(
            manager.connect_configured(request).await,
            Err(Error::Rejected)
        ));
        assert_eq!(old.evaluate(&target).unwrap().policy, Policy::Direct);
        // 独立验证新约定：连接只保留既定决策与活动资源；旧规则快照在连接仍工作时即可回收。
        let old_weak = Arc::downgrade(&old);
        drop(old);
        assert!(
            old_weak.upgrade().is_none(),
            "活动连接不得间接持有整份旧规则快照"
        );
        // 切换不能关闭已开始的连接；新请求已在上面断言使用版本 2 的拒绝规则。
        manager
            .write(opened.connection.id, b"x", 1000)
            .await
            .unwrap();
        let mut byte = [0];
        peer.read_exact(&mut byte).await.unwrap();
        peer.write_all(&byte).await.unwrap();
        assert_eq!(
            &*manager
                .read(opened.connection.id, 1, 1000)
                .await
                .unwrap()
                .bytes,
            b"x"
        );
        manager.close(opened.connection.id).await.unwrap();
        let before = std::fs::read(directory.join("runtime-rules.bin")).unwrap();
        let mut invalid = sources(Policy::Direct);
        invalid[0].rules[0].value = "invalid".into();
        let failed = service.start(Config::default(), invalid).unwrap();
        assert_eq!(finished(&service, failed.id).await.phase, Phase::Failed);
        assert_eq!(service.store.snapshot().unwrap().document().version, 2);
        let (send, receive) = mpsc::channel();
        let cancelled = service
            .spawn(
                move || {
                    receive.recv().unwrap();
                    Ok((Config::default(), sources(Policy::Direct)))
                },
                INPUTS.reserve(1).unwrap(),
            )
            .unwrap();
        service.cancel(cancelled.id).unwrap();
        assert!(matches!(
            service.start(Config::default(), sources(Policy::Direct)),
            Err(Error::InvalidState)
        ));
        send.send(()).unwrap();
        assert_eq!(
            finished(&service, cancelled.id).await.phase,
            Phase::Cancelled
        );
        assert_eq!(
            std::fs::read(directory.join("runtime-rules.bin")).unwrap(),
            before
        );
        let (send, receive) = mpsc::channel();
        let closing = service
            .spawn(
                move || {
                    receive.recv().unwrap();
                    Ok((Config::default(), sources(Policy::Direct)))
                },
                INPUTS.reserve(1).unwrap(),
            )
            .unwrap();
        service.close().unwrap();
        let reopened = Store::open(&directory).unwrap();
        send.send(()).unwrap();
        assert_eq!(finished(&service, closing.id).await.phase, Phase::Cancelled);
        assert_eq!(reopened.snapshot().unwrap().document().version, 2);
        reopened.close().unwrap();
        drop(reopened);
        drop(service);
        std::fs::remove_dir_all(directory).unwrap();
        // 阻塞四个加载器确定性耗尽全局槽位；第五个请求不能创建额外线程。
        let mut running = Vec::new();
        for _ in 0..4 {
            let directory = path();
            let service = Service::new(
                Store::open(&directory).unwrap(),
                Arc::new(Manager::default()),
            );
            let (send, receive) = mpsc::channel();
            let job = service
                .spawn(
                    move || {
                        receive.recv().unwrap();
                        Ok((Config::default(), sources(Policy::Direct)))
                    },
                    INPUTS.reserve(1).unwrap(),
                )
                .unwrap();
            running.push((directory, service, send, job.id));
        }
        let directory = path();
        let extra = Service::new(
            Store::open(&directory).unwrap(),
            Arc::new(Manager::default()),
        );
        assert!(matches!(
            extra.start(Config::default(), sources(Policy::Direct)),
            Err(Error::ResourceBudgetExceeded {
                actual: 1,
                limit: 4
            })
        ));
        for (directory, service, send, id) in running {
            send.send(()).unwrap();
            assert_eq!(finished(&service, id).await.phase, Phase::Succeeded);
            service.close().unwrap();
            drop(service);
            std::fs::remove_dir_all(directory).unwrap();
        }
        let restored = extra
            .start(Config::default(), sources(Policy::Direct))
            .unwrap();
        assert_eq!(finished(&extra, restored.id).await.phase, Phase::Succeeded);
        extra.close().unwrap();
        drop(extra);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
