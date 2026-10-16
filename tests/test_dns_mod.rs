//! 测试文件：tests/test_dns_mod.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/dns/mod.rs
//! 测试内容：
//! - observation_lock_failure_is_internal_not_dns_timeout：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! - retirement_drains_admitted_query_without_cancelling_it：验证退休准入、确定性清理及配置原子性，不以引用计数判断活动资源。
//! - replacement_reclaims_services_and_quota_with_old_snapshots_alive：验证退休准入、确定性清理及配置原子性，不以引用计数判断活动资源。
//! - failed_or_same_resolver_update_keeps_admission_active：验证退休准入、确定性清理及配置原子性，不以引用计数判断活动资源。
//! - explicit_close_cancels_query_during_retirement：验证退休准入、确定性清理及配置原子性，不以引用计数判断活动资源。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod error_semantics_tests {
    use super::super::*;
    #[tokio::test]
    async fn observation_lock_failure_is_internal_not_dns_timeout() {
        let upstream = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let resolver = Resolver::new(Config {
            servers: vec![Server::Udp {
                address: upstream.local_addr().unwrap().to_string(),
            }],
            timeout_ms: 200,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
        .unwrap();
        let observation = std::sync::Arc::clone(&resolver.observation);
        // 仅测试自己的统计锁；业务锁中毒必须受控失败，不能恢复成解析成功或超时。
        assert!(
            std::thread::spawn(move || {
                let _guard = observation.lock().unwrap();
                panic!("注入统计锁中毒");
            })
            .join()
            .is_err()
        );
        let error = resolver
            .resolve(Query {
                name: "example.test".into(),
                query_type: "A".into(),
            })
            .await
            .unwrap_err();
        assert_eq!(error, Error::Internal);
        assert_eq!(error.code().value(), 9901);
    }
}

// 独立业务预期：配置替换不能取消已准入查询，关闭会话则必须取消；两者都拒绝新查询。
mod retirement_contract {
    use super::super::*;
    use crate::outbound::{Limits, Manager};
    use hickory_resolver::proto::{
        op::{Message, MessageType},
        rr::{RData, Record, rdata::A},
    };
    use tokio::{
        net::{TcpListener, UdpSocket},
        sync::oneshot,
    };

    fn config(address: std::net::SocketAddr) -> Config {
        Config {
            servers: vec![Server::Udp {
                address: address.to_string(),
            }],
            timeout_ms: 1000,
            dnssec: false,
            fake_ip: None,
            service: None,
        }
    }
    fn query() -> Query {
        Query {
            name: "retire.example.test".into(),
            query_type: "A".into(),
        }
    }
    // 用真实回环报文和显式屏障确定查询已到达上游，不依赖 sleep 猜测调度顺序。
    async fn pending_query() -> (
        Arc<Resolver>,
        tokio::task::JoinHandle<Result<Answer>>,
        oneshot::Sender<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let resolver = Arc::new(Resolver::new(config(socket.local_addr().unwrap())).unwrap());
        let (arrived_tx, arrived) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut bytes = [0; 4096];
            let (len, peer) = socket.recv_from(&mut bytes).await.unwrap();
            arrived_tx.send(()).unwrap();
            if released.await.is_ok() {
                let mut response = Message::from_vec(&bytes[..len]).unwrap();
                response.metadata.message_type = MessageType::Response;
                response.metadata.recursion_available = true;
                response.add_answer(Record::from_rdata(
                    response.queries[0].name.clone(),
                    30,
                    RData::A(A(std::net::Ipv4Addr::LOCALHOST)),
                ));
                socket
                    .send_to(&response.to_vec().unwrap(), peer)
                    .await
                    .unwrap();
            }
        });
        let own = resolver.clone();
        let request = tokio::spawn(async move { own.resolve(query()).await });
        tokio::time::timeout(Duration::from_secs(2), arrived)
            .await
            .unwrap()
            .unwrap();
        (resolver, request, release, server)
    }
    async fn closed(resolver: &Resolver) {
        tokio::time::timeout(Duration::from_secs(2), resolver.authority.wait_closed())
            .await
            .unwrap()
            .unwrap();
        assert!(resolver.resources.read().await.is_none());
        assert_eq!(resolver.admission.phase().unwrap(), Phase::Closed);
    }
    #[tokio::test]
    async fn retirement_drains_admitted_query_without_cancelling_it() {
        let (old, request, release, server) = pending_query().await;
        let manager = Manager::default();
        manager.configure_dns(Some(old.clone())).unwrap();
        let snapshot = manager.network_snapshot().unwrap();
        let transaction = old.operation().unwrap();
        manager.configure_dns(None).unwrap();
        assert!(matches!(
            old.resolve(query()).await,
            Err(Error::InvalidState)
        ));
        assert_eq!(old.authority.phase().unwrap(), Phase::Active);
        release.send(()).unwrap();
        let answer = request.await.unwrap().unwrap();
        assert_eq!(answer.addresses, vec!["127.0.0.1"]);
        // 已准入事务可在退休后继续其后续查询；缓存不改变准入判据。
        assert!(
            old.resolve_operation(query(), true, &transaction)
                .await
                .unwrap()
                .from_cache
        );
        let unrelated = Arc::new(Resolver::new(config("127.0.0.1:9".parse().unwrap())).unwrap());
        assert!(matches!(
            unrelated
                .resolve_operation(query(), true, &transaction)
                .await,
            Err(Error::InvalidState)
        ));
        unrelated.close().await.unwrap();
        drop(transaction);
        closed(&old).await;
        assert!(snapshot.resolver.is_some());
        assert!(matches!(
            old.respond(&[], true).await,
            Err(Error::InvalidState)
        ));
        server.await.unwrap();
    }
    #[tokio::test]
    async fn replacement_reclaims_services_and_quota_with_old_snapshots_alive() {
        let upstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let manager = Manager::default();
        let first = Arc::new(Resolver::new(config(upstream.local_addr().unwrap())).unwrap());
        manager.configure_dns(Some(first.clone())).unwrap();
        let mut service = service::Service::start(
            service::Config {
                listen: "127.0.0.1:0".into(),
                allow_lan: false,
                allowed_clients: vec![],
            },
            first.clone(),
        )
        .await
        .unwrap();
        let address = service.status().address;
        let mut retained = Vec::new();
        // 超过全局 16 个活动解析器上限；所有旧快照仍存活时也必须释放实际实例配额。
        for _ in 0..24 {
            let snapshot = manager.network_snapshot().unwrap();
            let old = snapshot.resolver.as_ref().unwrap().clone();
            let new = Arc::new(Resolver::new(config(upstream.local_addr().unwrap())).unwrap());
            manager.configure_dns(Some(new)).unwrap();
            closed(&old).await;
            retained.push(snapshot);
        }
        assert!(!service.status().running);
        let tcp = TcpListener::bind(&address).await.unwrap();
        let udp = UdpSocket::bind(&address).await.unwrap();
        assert_eq!(retained.len(), 24);
        manager.configure_dns(None).unwrap();
        service.stop().await.unwrap();
        drop((tcp, udp));
    }
    #[tokio::test]
    async fn failed_or_same_resolver_update_keeps_admission_active() {
        let upstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let manager = Manager::default();
        let resolver = Arc::new(Resolver::new(config(upstream.local_addr().unwrap())).unwrap());
        manager.configure_dns(Some(resolver.clone())).unwrap();
        manager.configure_dns(Some(resolver.clone())).unwrap();
        manager.configure_limits(Limits::default()).unwrap();
        assert!(
            manager
                .configure_network(
                    None,
                    None,
                    Limits {
                        maximum_connections: 0,
                        ..Limits::default()
                    }
                )
                .is_err()
        );
        resolver.ensure_active().unwrap();
        let retired = Arc::new(Resolver::new(config(upstream.local_addr().unwrap())).unwrap());
        retired.close().await.unwrap();
        assert!(manager.configure_dns(Some(retired)).is_err());
        assert!(Arc::ptr_eq(
            manager
                .network_snapshot()
                .unwrap()
                .resolver
                .as_ref()
                .unwrap(),
            &resolver
        ));
        resolver.ensure_active().unwrap();
        resolver.close().await.unwrap();
    }
    #[tokio::test]
    async fn explicit_close_cancels_query_during_retirement() {
        let (old, request, release, server) = pending_query().await;
        old.retire().unwrap();
        old.close().await.unwrap();
        assert!(matches!(request.await.unwrap(), Err(Error::InvalidState)));
        closed(&old).await;
        drop(release);
        server.await.unwrap();
    }
}
