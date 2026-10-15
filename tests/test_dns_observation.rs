//! 测试文件：tests/test_dns_observation.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/dns/observation.rs
//! 测试内容：
//! - shares_only_live_matching_configuration：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    fn config(port: u16) -> super::super::super::Config {
        super::super::super::Config {
            servers: vec![super::super::super::Server::Udp {
                address: format!("127.0.0.1:{port}"),
            }],
            timeout_ms: 1000,
            dnssec: false,
            fake_ip: None,
            service: None,
        }
    }
    #[test]
    fn shares_only_live_matching_configuration() {
        let a = shared(&config(18001)).unwrap();
        let b = shared(&config(18001)).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        *a.lock().unwrap() = Some(Observation {
            duration_ms: 12,
            from_cache: false,
            successful: true,
        });
        assert_eq!(b.lock().unwrap().as_ref().unwrap().duration_ms, 12);
        let other = shared(&config(18002)).unwrap();
        assert!(other.lock().unwrap().is_none());
        drop(a);
        drop(b);
        assert!(shared(&config(18001)).unwrap().lock().unwrap().is_none());
    }
}
