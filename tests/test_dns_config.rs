//! 测试文件：tests/test_dns_config.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/dns/config.rs
//! 测试内容：
//! - upstream_count_must_stay_within_one_to_eight：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - timeout_must_stay_within_100_to_30000_ms：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - encrypted_and_plaintext_upstreams_cannot_mix：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - bootstrap_must_be_numeric_and_match_url_port：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - doh_url_rejects_credentials_fragment_and_non_https：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - endpoint_rejects_illegal_addresses：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - dot_server_name_must_be_a_valid_host：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    // # DNS 上游配置校验测试
    //
    // ## 测试类型
    // 集成测试；安全测试（上游与 bootstrap 的输入校验边界）；回归测试（既有校验规则的既定行为）。
    //
    // ## 测试对象
    // 生产代码路径：`src/dns/config.rs`（`Config::validate` 及其调用的 `Config::build`、
    // 私有函数 `endpoint`、`super::normalize` 对 `dot` 服务名的校验）。
    //
    // 公开接口：`nodus_core::dns::{Config, Server}`（由 `src/dns/mod.rs:14`
    // 的 `pub use config::{Config, Server};` 导出）、`nodus_core::error::Error`。
    // `src/dns/config.rs` 本身是私有模块（`src/dns/mod.rs:3` 的 `mod config;`），
    // 因此只经上述公开导出覆盖，未为测试扩大其可见性。
    //
    // ## 测试内容
    // 1. `upstream_count_must_stay_within_one_to_eight`：上游数量 1–8 合法，0 个与 9 个必须拒绝。
    // 2. `timeout_must_stay_within_100_to_30000_ms`：总期限 100–30000 毫秒合法，越界必须拒绝。
    // 3. `encrypted_and_plaintext_upstreams_cannot_mix`：纯明文或纯加密合法，两者混用必须拒绝。
    // 4. `bootstrap_must_be_numeric_and_match_url_port`：bootstrap 必须是数字 IP:端口，且端口与 URL 一致。
    // 5. `doh_url_rejects_credentials_fragment_and_non_https`：DoH 的 URL 禁止非 https、凭据与 fragment。
    // 6. `endpoint_rejects_illegal_addresses`：端口 0、未指定地址、组播、广播与不可解析地址必须拒绝。
    // 7. `dot_server_name_must_be_a_valid_host`：`dot` 的 server-name 必须是可规范化的主机名。
    //
    // ## 断言依据
    // `Documentation/DNS.md` 第 5 行「加密与明文上游不能混合……bootstrap 必须使用数字 IP 和端口」、
    // 第 7 行「查询总期限 100–30000 毫秒，上游 1–8 个」、第 25 行「URL 禁止凭据和 fragment，
    // bootstrap 端口必须与 URL 一致」。预期取自该规范，不取自当前实现的输出。
    //
    // ## 隔离边界
    // 只使用公开 API 与字面量配置，不发起任何网络查询，不修改业务代码。
    // 本文件不申请全局准入池配额，用例之间无共享可变状态，默认并行执行成立。
    //
    // ## 执行状态
    // 7 项已实现、含有效断言且实际执行，结果见同名报告 `tests/test_dns_config.md`。

    use nodus_core::dns::{Config, Server};
    use nodus_core::error::Error;

    fn plain(address: &str) -> Server {
        Server::Udp {
            address: address.into(),
        }
    }

    fn tcp(address: &str) -> Server {
        Server::Tcp {
            address: address.into(),
        }
    }

    fn dot(address: &str, server_name: &str) -> Server {
        Server::Dot {
            address: address.into(),
            server_name: server_name.into(),
        }
    }

    fn doh(url: &str, bootstrap: &str) -> Server {
        Server::Doh {
            url: url.into(),
            bootstrap: bootstrap.into(),
        }
    }

    fn config(servers: Vec<Server>, timeout_ms: u64) -> Config {
        Config {
            servers,
            timeout_ms,
            dnssec: false,
            fake_ip: None,
            service: None,
        }
    }

    fn accepted(servers: Vec<Server>, timeout_ms: u64) {
        let result = config(servers, timeout_ms).validate();
        assert!(result.is_ok(), "应当被接受，实际：{result:?}");
    }

    fn rejected(servers: Vec<Server>, timeout_ms: u64) {
        let result = config(servers, timeout_ms).validate();
        assert!(
            matches!(result, Err(Error::InvalidInput)),
            "必须返回 InvalidInput，实际：{result:?}"
        );
    }

    #[test]
    fn upstream_count_must_stay_within_one_to_eight() {
        rejected(vec![], 5_000);
        accepted(vec![plain("1.1.1.1:53")], 5_000);
        accepted(
            (1..=8)
                .map(|index| plain(&format!("10.0.0.{index}:53")))
                .collect(),
            5_000,
        );
        rejected(
            (1..=9)
                .map(|index| plain(&format!("10.0.0.{index}:53")))
                .collect(),
            5_000,
        );
    }

    #[test]
    fn timeout_must_stay_within_100_to_30000_ms() {
        accepted(vec![plain("1.1.1.1:53")], 100);
        accepted(vec![plain("1.1.1.1:53")], 30_000);
        rejected(vec![plain("1.1.1.1:53")], 99);
        rejected(vec![plain("1.1.1.1:53")], 30_001);
    }

    #[test]
    fn encrypted_and_plaintext_upstreams_cannot_mix() {
        accepted(vec![plain("1.1.1.1:53"), tcp("1.0.0.1:53")], 5_000);
        accepted(
            vec![
                dot("1.1.1.1:853", "dns.example"),
                doh("https://dns.example/dns-query", "1.0.0.1:443"),
            ],
            5_000,
        );
        rejected(
            vec![plain("1.1.1.1:53"), dot("1.1.1.1:853", "dns.example")],
            5_000,
        );
        rejected(
            vec![
                tcp("1.1.1.1:53"),
                doh("https://dns.example/dns-query", "1.1.1.1:443"),
            ],
            5_000,
        );
    }

    #[test]
    fn bootstrap_must_be_numeric_and_match_url_port() {
        accepted(
            vec![doh("https://dns.example/dns-query", "1.1.1.1:443")],
            5_000,
        );
        // bootstrap 不是数字 IP。
        rejected(
            vec![doh(
                "https://dns.example/dns-query",
                "bootstrap.example:443",
            )],
            5_000,
        );
        // bootstrap 端口与 URL 默认端口不一致。
        rejected(
            vec![doh("https://dns.example/dns-query", "1.1.1.1:8443")],
            5_000,
        );
        // URL 显式端口时必须与 bootstrap 一致。
        accepted(
            vec![doh("https://dns.example:8443/dns-query", "1.1.1.1:8443")],
            5_000,
        );
        rejected(
            vec![doh("https://dns.example:8443/dns-query", "1.1.1.1:443")],
            5_000,
        );
    }

    #[test]
    fn doh_url_rejects_credentials_fragment_and_non_https() {
        rejected(
            vec![doh("http://dns.example/dns-query", "1.1.1.1:80")],
            5_000,
        );
        rejected(
            vec![doh("https://user@dns.example/dns-query", "1.1.1.1:443")],
            5_000,
        );
        rejected(
            vec![doh(
                "https://user:secret@dns.example/dns-query",
                "1.1.1.1:443",
            )],
            5_000,
        );
        rejected(
            vec![doh("https://dns.example/dns-query#frag", "1.1.1.1:443")],
            5_000,
        );
    }

    #[test]
    fn endpoint_rejects_illegal_addresses() {
        for address in [
            "1.1.1.1:0",
            "0.0.0.0:53",
            "[::]:53",
            "224.0.0.1:53",
            "255.255.255.255:53",
            "not-an-address",
            "1.1.1.1",
        ] {
            let result = config(vec![plain(address)], 5_000).validate();
            assert!(
                matches!(result, Err(Error::InvalidInput)),
                "{address} 必须被拒绝，实际：{result:?}"
            );
        }
        accepted(vec![plain("[2606:4700:4700::1111]:53")], 5_000);
    }

    #[test]
    fn dot_server_name_must_be_a_valid_host() {
        accepted(vec![dot("1.1.1.1:853", "dns.example")], 5_000);
        let too_long = "a".repeat(254);
        for name in ["", "bad..example", too_long.as_str()] {
            let result = config(vec![dot("1.1.1.1:853", name)], 5_000).validate();
            assert!(
                matches!(result, Err(Error::InvalidInput)),
                "server-name {name:?} 必须被拒绝，实际：{result:?}"
            );
        }
    }
}
