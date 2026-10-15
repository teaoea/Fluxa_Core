//! 测试文件：tests/test_dns_mod.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/dns/mod.rs
//! 测试内容：
//! - observation_lock_failure_is_internal_not_dns_timeout：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
