//! 测试文件：tests/test_mitm_capture.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/mitm/capture.rs
//! 测试内容：
//! - oldest_records_are_evicted_and_acknowledgement_is_exact：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn oldest_records_are_evicted_and_acknowledgement_is_exact() {
        let store = CaptureStore::default();
        for _ in 0..65 {
            store.push(Capture::new()).unwrap();
        }
        assert_eq!(store.dropped().unwrap(), 1);
        let first = store.peek().unwrap().unwrap();
        assert_eq!(first.id, 2);
        assert!(store.acknowledge(1).is_err());
        assert_eq!(store.peek().unwrap().unwrap().id, 2);
        store.acknowledge(2).unwrap();
        assert_eq!(store.peek().unwrap().unwrap().id, 3);
    }
}
