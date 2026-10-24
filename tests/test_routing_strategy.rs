//! 测试文件：tests/test_routing_strategy.rs
//! 测试类型：单元测试、策略输入安全回归
//! 测试对象：src/routing/strategy.rs
//! 测试内容：ordered_unicode_and_explicit_terminals、invalid_or_unbounded_lists_fail、single_proxy_name_is_preserved。
//! 测试依据：用户有序候选语义；上限8项、每节点128字节；禁止空项、重复、嵌套及reject后不可达候选。
//! 前置条件：无网络；不证明节点互通。
use super::*;
#[test]
fn ordered_unicode_and_explicit_terminals() {
    let p = parse("节点1,节点2,reject").unwrap();
    assert_eq!(p.items(), &["节点1", "节点2", "reject"]);
    assert_eq!(p.nodes().collect::<Vec<_>>(), vec!["节点1", "节点2"]);
    assert_eq!(p.canonical(), "(节点1,节点2,reject)");
    assert_eq!(
        parse("(direct,节点2)").unwrap().items(),
        &["direct", "节点2"]
    );
    assert!(parse(&["a", "b", "c", "d", "e", "f", "g", "h"].join(",")).is_ok());
}
#[test]
fn invalid_or_unbounded_lists_fail() {
    for value in [
        "",
        "a,",
        ",a",
        "a,,b",
        "a,a",
        "a, b",
        "(a)",
        "(a,b",
        "a,b)",
        "((a,b),c)",
        "(a,b)x",
        "reject,a",
    ] {
        assert!(parse(value).is_err(), "{value}");
    }
    assert!(matches!(
        parse("a,b,c,d,e,f,g,h,i"),
        Err(Error::ResourceLimit)
    ));
    assert!(parse(&"x".repeat(129)).is_err());
    assert!(parse(&"x".repeat(128)).is_ok());
}
#[test]
fn single_proxy_name_is_preserved() {
    assert_eq!(parse("节点").unwrap().canonical(), "节点");
    assert_eq!(members("direct").unwrap(), vec!["direct"]);
}
