//! 测试文件：tests/test_protocol_fingerprint.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/protocol/fingerprint.rs
//! 测试内容：
//! - fixed_templates_have_distinct_wire_shapes：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - chrome_shuffle_grease_ech_and_padding_are_bounded：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    fn extensions(bytes: &[u8]) -> Vec<(u16, &[u8])> {
        let mut result = Vec::new();
        let mut p = 0;
        while p < bytes.len() {
            let tag = u16::from_be_bytes([bytes[p], bytes[p + 1]]);
            let n = usize::from(u16::from_be_bytes([bytes[p + 2], bytes[p + 3]]));
            result.push((tag, &bytes[p + 4..p + 4 + n]));
            p += 4 + n;
        }
        result
    }
    fn grease(tag: u16) -> bool {
        tag & 0x0f0f == 0x0a0a && tag >> 8 == tag & 255
    }
    #[test]
    fn fixed_templates_have_distinct_wire_shapes() {
        for (kind, expected) in [
            (
                Fingerprint::Firefox,
                vec![
                    0, 23, 65281, 10, 11, 35, 16, 5, 34, 51, 43, 13, 45, 28, 65037,
                ],
            ),
            (
                Fingerprint::Safari,
                vec![0, 23, 65281, 10, 11, 16, 5, 13, 18, 51, 45, 43, 27],
            ),
            (
                Fingerprint::Ios,
                vec![0, 23, 65281, 10, 11, 16, 5, 13, 18, 51, 45, 43],
            ),
            (
                Fingerprint::Edge,
                vec![
                    0, 23, 65281, 10, 11, 35, 16, 5, 13, 18, 51, 45, 43, 27, 17513,
                ],
            ),
        ] {
            let m = build(kind, "example.com", &[7; 32]).unwrap();
            let e = extensions(&m.extensions);
            let tags: Vec<_> = e
                .iter()
                .map(|v| v.0)
                .filter(|v| !grease(*v) && *v != 21)
                .collect();
            assert_eq!(tags, expected, "{}", kind.version());
            assert_eq!(m.p256.is_some(), kind == Fingerprint::Firefox);
            assert!(m.ciphers.as_chunks::<2>().0.contains(&[0x13, 1]));
            if kind == Fingerprint::Firefox {
                assert_eq!(&m.ciphers[..6], &[0x13, 1, 0x13, 3, 0x13, 2]);
                assert_eq!(e.iter().find(|v| v.0 == 65037).unwrap().1.len(), 281);
            }
        }
    }
    #[test]
    fn chrome_shuffle_grease_ech_and_padding_are_bounded() {
        let mut orders = std::collections::HashSet::new();
        for kind in [
            Fingerprint::Chrome,
            Fingerprint::Android,
            Fingerprint::Chrome131,
            Fingerprint::Chrome133,
            Fingerprint::Android133,
        ] {
            for _ in 0..32 {
                let m = build(kind, "example.com", &[9; 32]).unwrap();
                let e = extensions(&m.extensions);
                assert!(grease(e[0].0));
                let tags: Vec<_> = e
                    .iter()
                    .map(|v| v.0)
                    .filter(|v| !grease(*v) && *v != 21)
                    .collect();
                let mut sorted = tags.clone();
                sorted.sort_unstable();
                let alps = if matches!(kind, Fingerprint::Chrome133 | Fingerprint::Android133) {
                    17613
                } else {
                    17513
                };
                assert_eq!(
                    sorted,
                    vec![
                        0, 5, 10, 11, 13, 16, 18, 23, 27, 35, 43, 45, 51, alps, 65037, 65281
                    ]
                );
                orders.insert(tags);
                let ech = e.iter().find(|v| v.0 == 65037).unwrap().1;
                assert_eq!(&ech[..5], &[0, 0, 1, 0, 1]);
                assert_eq!(&ech[6..8], &[0, 32]);
                let n = usize::from(u16::from_be_bytes([ech[40], ech[41]]));
                assert!([144, 176, 208, 240].contains(&n));
                assert_eq!(ech.len(), 42 + n);
                assert!(m.extensions.len() < 4096);
            }
        }
        assert!(orders.len() > 1);
        assert!(build(Fingerprint::Chrome, &"a".repeat(254), &[0; 32]).is_err());
        assert_eq!(
            Fingerprint::parse("chrome120").unwrap(),
            Fingerprint::Chrome
        );
        assert!(Fingerprint::parse("randomized").is_err());
    }
}
