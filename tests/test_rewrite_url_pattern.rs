//! 测试文件：tests/test_rewrite_url_pattern.rs
//! 测试类型：单元测试、URL 语义与边界回归、独立算法差分。
//! 测试对象：src/rewrite/url_pattern.rs。
//! 测试内容：user_single_and_double_star_examples 验证用户示例；literal_paths_are_anchored_and_case_sensitive 验证完整字面路径；
//! double_star_supports_zero_segments_and_literal_suffix 验证中间/多处 **；empty_and_trailing_segments_remain_explicit 验证空段及尾斜杠；
//! query_is_separate_from_path_segments 验证查询约定；encoding_and_dot_segments_are_not_rewritten 验证不解码归一化；
//! invalid_patterns_and_targets_are_rejected 验证输入拒绝；length_and_deep_segment_bounds_are_enforced 验证资源上限和深路径；
//! greedy_matches_independent_dynamic_programming 验证枚举输入与独立 DP 结果一致。
//! 测试依据：用户 2026-10-20 第三项，* 单个非空段、** 零或多段、完整路径及大小写语义。
//! 前置条件：纯内存匹配；不访问网络、共享容器或 JS 引擎。
use super::*;
fn matcher(path: &str) -> UrlPattern {
    UrlPattern::compile(&format!("https://example.com{path}")).unwrap()
}
fn matches(pattern: &str, path: &str) -> bool {
    matcher(pattern)
        .matches(&format!("https://example.com{path}"))
        .unwrap()
}
#[test]
fn user_single_and_double_star_examples() {
    for (pattern, target, expected) in [
        ("/api/*", "/api/a", true),
        ("/api/*", "/api/aa/a", false),
        ("/api/**", "/api/a", true),
        ("/api/**", "/api/aa/a", true),
        ("/api/*", "/api", false),
        ("/api/*", "/api/", false),
        ("/api/**", "/api", true),
        ("/api/**", "/api/", true),
        ("/api/*", "/other/api/a", false),
        ("/api/**", "/apis/a", false),
    ] {
        assert_eq!(matches(pattern, target), expected, "{pattern} -> {target}");
    }
}
#[test]
fn literal_paths_are_anchored_and_case_sensitive() {
    for (target, expected) in [
        ("/api/a", true),
        ("/api/aa", false),
        ("/api/a/", false),
        ("/api/a/child", false),
        ("/API/a", false),
        ("/api/A", false),
    ] {
        assert_eq!(matches("/api/a", target), expected);
    }
    let pattern = matcher("/api/a");
    for target in [
        "http://example.com/api/a",
        "https://other.example.com/api/a",
        "https://EXAMPLE.com/api/a",
        "https://example.com:443/api/a",
    ] {
        assert!(!pattern.matches(target).unwrap());
    }
}
#[test]
fn double_star_supports_zero_segments_and_literal_suffix() {
    for (pattern, target, expected) in [
        ("/api/**/tail", "/api/tail", true),
        ("/api/**/tail", "/api/a/b/tail", true),
        ("/api/**/tail", "/api/a/tail/child", false),
        ("/api/**/tail", "/api/tails", false),
        ("/**/a/**/b", "/a/b", true),
        ("/**/a/**/b", "/x/a/y/a/z/b", true),
        ("/**/a/**/b", "/x/a/y/a/z/b/child", false),
        ("/**/**/b", "/b", true),
        ("/**/a/*/b", "/a/x/b", true),
        ("/**/a/*/b", "/a/b", false),
    ] {
        assert_eq!(matches(pattern, target), expected, "{pattern} -> {target}");
    }
}
#[test]
fn empty_and_trailing_segments_remain_explicit() {
    for (pattern, target, expected) in [
        ("/api/*", "/api//", false),
        ("/api/*", "/api/a/", false),
        ("/api/*/b", "/api//b", false),
        ("/api/**/b", "/api//b", true),
        ("/api//b", "/api/b", false),
        ("/api//b", "/api//b", true),
        ("/api/**/", "/api", false),
        ("/api/**/", "/api/", true),
        ("/**", "/", true),
        ("/*", "/", false),
        ("/", "/a", false),
    ] {
        assert_eq!(matches(pattern, target), expected, "{pattern} -> {target}");
    }
    assert!(
        UrlPattern::compile("https://example.com")
            .unwrap()
            .matches("https://example.com/")
            .unwrap()
    );
}
#[test]
fn query_is_separate_from_path_segments() {
    assert!(
        matcher("/api/*")
            .matches("https://example.com/api/a?next=/a/b")
            .unwrap()
    );
    let pattern = UrlPattern::compile("https://example.com/api/*?source=Nodus&x=1").unwrap();
    for (query, expected) in [
        ("?source=Nodus&x=1", true),
        ("?source=nodus&x=1", false),
        ("?x=1&source=Nodus", false),
        ("", false),
    ] {
        assert_eq!(
            pattern
                .matches(&format!("https://example.com/api/a{query}"))
                .unwrap(),
            expected
        );
    }
    assert!(
        UrlPattern::compile("https://example.com/api/*?")
            .unwrap()
            .matches("https://example.com/api/a?")
            .unwrap()
    );
    assert!(
        !UrlPattern::compile("https://example.com/api/*?")
            .unwrap()
            .matches("https://example.com/api/a")
            .unwrap()
    );
}
#[test]
fn encoding_and_dot_segments_are_not_rewritten() {
    assert!(matches("/api/*", "/api/a%2Fb"));
    assert!(!matches("/api/*", "/api/a/b"));
    assert!(matches("/api/%2F", "/api/%2F"));
    assert!(!matches("/api/%2F", "/api/%2f"));
    assert!(!matches("/api/a", "/api/%61"));
    assert!(matches("/a/../b", "/a/../b"));
    assert!(!matches("/a/../b", "/b"));
    assert!(matches("/路径/*", "/路径/正文"));
}
#[test]
fn invalid_patterns_and_targets_are_rejected() {
    for pattern in [
        "",
        "/api/*",
        "ftp://example.com/a",
        "https://u:p@example.com/a",
        "https://@example.com/a",
        "https://*.example.com/a",
        "https://example.com/a*b",
        "https://example.com/***",
        "https://example.com/a?x=*",
        "https://example.com/a#fragment",
        "https://example.com/a b",
        "https://example.com/a\\b",
        "https://example.com:bad/a",
    ] {
        assert!(UrlPattern::compile(pattern).is_err(), "accepted {pattern}");
    }
    for target in [
        "",
        "/api/a",
        "ftp://example.com/api/a",
        "https://u:p@example.com/api/a",
        "https://example.com/api/a#x",
        "https://example.com/api/a\n",
    ] {
        assert!(matcher("/api/*").matches(target).is_err());
    }
}
#[test]
fn length_and_deep_segment_bounds_are_enforced() {
    let prefix = "https://example.com/";
    let maximal = format!("{prefix}{}", "a".repeat(MAX_URL_BYTES - prefix.len()));
    let pattern = UrlPattern::compile(&maximal).unwrap();
    assert!(pattern.matches(&maximal).unwrap());
    assert!(matches!(
        UrlPattern::compile(&(maximal.clone() + "a")),
        Err(Error::ResourceLimit)
    ));
    assert!(matches!(
        pattern.matches(&(maximal + "a")),
        Err(Error::ResourceLimit)
    ));
    let path = format!("/{}z", "a/".repeat(1000));
    assert!(matches("/**/z", &path));
    assert!(!matches("/**/z", &(path + "/suffix")));
    let pattern = format!("/**/{}b", "a/".repeat(200));
    let target = format!("/{}c", "a/".repeat(300));
    assert!(!matches(&pattern, &target));
}
// 独立算法：二维可达性表枚举 ** 消费零段或多段，和生产的最近 ** 贪心重试不同。
fn reference(pattern: &str, target: &str) -> bool {
    let p: Vec<_> = pattern[1..].split('/').collect();
    let t: Vec<_> = target[1..].split('/').collect();
    let mut reachable = vec![vec![false; t.len() + 1]; p.len() + 1];
    reachable[0][0] = true;
    for i in 0..p.len() {
        for j in 0..=t.len() {
            if !reachable[i][j] {
                continue;
            }
            if p[i] == "**" {
                reachable[i + 1][j] = true;
                if j < t.len() {
                    reachable[i][j + 1] = true;
                }
            } else if j < t.len() && (p[i] == t[j] || (p[i] == "*" && !t[j].is_empty())) {
                reachable[i + 1][j + 1] = true;
            }
        }
    }
    reachable[p.len()][t.len()]
}
fn paths(alphabet: &[&str], depth: usize) -> Vec<String> {
    let mut output = vec!["/".to_owned()];
    let mut level = vec![Vec::<&str>::new()];
    for _ in 0..depth {
        let mut next = Vec::new();
        for prefix in &level {
            for segment in alphabet {
                let mut path = prefix.clone();
                path.push(segment);
                output.push(format!("/{}", path.join("/")));
                next.push(path);
            }
        }
        level = next;
    }
    output
}
#[test]
fn greedy_matches_independent_dynamic_programming() {
    let patterns = paths(&["a", "b", "*", "**", ""], 4);
    let targets = paths(&["a", "b", ""], 4);
    for pattern in patterns {
        let compiled = matcher(&pattern);
        for target in &targets {
            assert_eq!(
                compiled
                    .matches(&format!("https://example.com{target}"))
                    .unwrap(),
                reference(&pattern, target),
                "{pattern} -> {target}"
            );
        }
    }
}
