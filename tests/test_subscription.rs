//! 测试文件：tests/test_subscription.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/subscription.rs
//! 测试内容：
//! - utc_boundary_manual_and_daily_retry：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - resume_catches_latest_slot_once_even_before_todays_slot：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - manual_before_slot_does_not_suppress_later_slot：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - failed_or_interrupted_attempt_retries_latest_slot_with_backoff：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - strict_input_and_midnight：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fetches_https_and_rejects_invalid_content：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - body_limits_precede_allocation_for_all_framings：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    fn check(time: Option<&str>, now: u64) -> Check {
        Check {
            schedule: Schedule {
                update_time: time.map(str::to_owned),
            },
            now,
            last_success: None,
            last_attempt: None,
            enabled: true,
            manual: false,
        }
    }
    #[test]
    fn utc_boundary_manual_and_daily_retry() {
        let mut c = check(Some("12:00"), DAY + 43199);
        c.last_success = Some(43200);
        assert!(!c.evaluate().unwrap().due);
        assert_eq!(c.evaluate().unwrap().next_update, Some(DAY + 43200));
        c.now += 1;
        assert!(c.evaluate().unwrap().due);
        c.last_attempt = Some(c.now);
        assert!(!c.evaluate().unwrap().due);
        c.now += DAY;
        assert!(c.evaluate().unwrap().due);
        c.last_success = Some(c.now);
        assert!(!c.evaluate().unwrap().due);
        c.manual = true;
        c.enabled = false;
        assert!(c.evaluate().unwrap().due);
        let mut absent = check(None, c.now);
        assert_eq!(
            absent.evaluate().unwrap(),
            Decision {
                due: false,
                next_update: None
            }
        );
        absent.manual = true;
        assert!(absent.evaluate().unwrap().due);
    }
    #[test]
    fn resume_catches_latest_slot_once_even_before_todays_slot() {
        let mut c = check(Some("12:00"), 10 * DAY + 3600);
        c.last_success = Some(DAY);
        assert!(c.evaluate().unwrap().due);
        assert_eq!(c.evaluate().unwrap().next_update, Some(10 * DAY + 43200));
        c.last_attempt = Some(c.now);
        assert!(!c.evaluate().unwrap().due);
        c.now = 10 * DAY + 43200;
        assert!(c.evaluate().unwrap().due);
    }
    #[test]
    fn manual_before_slot_does_not_suppress_later_slot() {
        let mut c = check(Some("12:00"), DAY + 43200);
        c.last_success = Some(DAY + 43199);
        assert!(c.evaluate().unwrap().due);
        c.last_success = Some(DAY + 43201);
        assert!(!c.evaluate().unwrap().due);
        c.enabled = false;
        c.now += DAY;
        assert_eq!(c.evaluate().unwrap().next_update, None);
    }
    #[test]
    fn failed_or_interrupted_attempt_retries_latest_slot_with_backoff() {
        let mut c = check(Some("12:00"), 10 * DAY + 50000);
        c.last_attempt = Some(c.now);
        assert!(!c.evaluate().unwrap().due);
        assert_eq!(c.evaluate().unwrap().next_update, Some(c.now + RETRY));
        c.now += RETRY;
        assert!(c.evaluate().unwrap().due);
        c.last_success = Some(c.now);
        c.now += RETRY;
        assert!(!c.evaluate().unwrap().due);
    }
    #[test]
    fn strict_input_and_midnight() {
        for value in [
            "", "1:00", "12:0", "24:00", "12:60", "12：00", " 12:00", "12:00Z", "-1:00", "aa:bb",
        ] {
            assert!(check(Some(value), DAY).evaluate().is_err());
        }
        for value in ["00:00", "23:59", "12:00"] {
            assert!(check(Some(value), DAY).evaluate().is_ok());
        }
        assert!(check(Some("00:00"), DAY).evaluate().unwrap().due);
        let mut c = check(Some("12:00"), MAX_TIME + 1);
        assert!(c.evaluate().is_err());
        c.now = 0;
        c.last_attempt = Some(MAX_TIME + 1);
        assert!(c.evaluate().is_err());
        assert!(serde_json::from_str::<Schedule>(r#"{"update-interval-hours":24}"#).is_err());
        assert!(
            serde_json::from_str::<Schedule>(r#"{"update-time":"12:00","update-time":"13:00"}"#)
                .is_err()
        );
    }
}

use crate::test_fixture_https as support;
// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    // 验证订阅实际 HTTPS 下载、重定向、安全边界和共用有界分帧。
    use super::support;
    use nodus_core::{error::Error, https::HttpsClient, subscription};
    #[tokio::test]
    async fn fetches_https_and_rejects_invalid_content() {
        let fixture = support::fixture().await;
        let mut roots = rustls::RootCertStore::empty();
        roots.add(fixture.root.clone()).unwrap();
        let client = HttpsClient::with_roots(roots)
            .unwrap()
            .with_network(fixture.network.clone());
        let url = format!("https://localhost:{}", fixture.port);
        for path in ["/subscription", "/subscription-redirect"] {
            let response = subscription::fetch(&client, &format!("{url}{path}"))
                .await
                .unwrap();
            assert_eq!(
                response.body,
                b"schema-version: 1\nkind: rules\nname: Fixture\nrules: []\n"
            );
        }
        for path in [
            "/subscription-downgrade",
            "/subscription-loop",
            "/subscription-limit",
            "/subscription-invalid",
            "/subscription-error",
        ] {
            assert!(
                subscription::fetch(&client, &format!("{url}{path}"))
                    .await
                    .is_err()
            );
        }
        for address in [
            "http://localhost/",
            "https://user:secret@localhost/",
            "https://localhost/#fragment",
        ] {
            assert!(subscription::fetch(&client, address).await.is_err());
        }
        assert_eq!(
            client
                .execute_limited(&format!("{url}/chunked"), 5)
                .await
                .err(),
            Some(Error::ResourceLimit)
        );
    }
    #[tokio::test]
    async fn body_limits_precede_allocation_for_all_framings() {
        use nodus_core::http::{Head, Header, read_body_limited};
        use tokio::io::BufReader;
        let head = |name: &str, value: &str| Head {
            first: "HTTP/1.1 200 OK".into(),
            headers: vec![Header {
                name: name.into(),
                value: value.into(),
            }],
        };
        let fixed = head("Content-Length", "2097153");
        let mut reader = BufReader::new(&b""[..]);
        assert_eq!(
            read_body_limited(&mut reader, &fixed, true, 2 * 1024 * 1024)
                .await
                .err(),
            Some(Error::ResourceLimit)
        );
        let chunked = head("Transfer-Encoding", "chunked");
        let mut reader = BufReader::new(&b"200001\r\n"[..]);
        assert_eq!(
            read_body_limited(&mut reader, &chunked, true, 2 * 1024 * 1024)
                .await
                .err(),
            Some(Error::ResourceLimit)
        );
        let eof = Head {
            first: fixed.first,
            headers: vec![],
        };
        let mut reader = BufReader::new(&b"abcdef"[..]);
        assert_eq!(
            read_body_limited(&mut reader, &eof, true, 5).await.err(),
            Some(Error::ResourceLimit)
        );
    }
}
