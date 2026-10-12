//! 资源估算压力示例：唯一多标签后缀与长关键词；达到预算时受控拒绝。
use libfluxa_core::routing::{
    Config, Match, Policy, Rule,
    runtime::{Layer, Snapshot, Source},
};
const LABEL_COUNT: usize = 40;
const KEYWORD_PADDING: usize = 200;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let count: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(1000);
    let keyword = args.get(2).is_some_and(|v| v == "keyword");
    let rules = (0..count)
        .map(|i| Rule {
            r#type: if keyword {
                Match::DomainKeyword
            } else {
                Match::DomainSuffix
            },
            value: if keyword {
                format!("keyword-{i}-{}", "a".repeat(KEYWORD_PADDING))
            } else {
                format!("{}com", format!("{i:x}.").repeat(LABEL_COUNT))
            },
            policy: Policy::Direct,
            outbound: None,
        })
        .collect();
    let start = std::time::Instant::now();
    match Snapshot::generate(
        Config::default(),
        vec![Source {
            id: "stress".into(),
            layer: Layer::Local,
            enabled: true,
            rules,
        }],
        1,
    ) {
        Ok(snapshot) => println!(
            "count={count} keyword={keyword} effective={} compile_ms={}",
            snapshot.document().report.effective,
            start.elapsed().as_millis()
        ),
        Err(error) => println!(
            "count={count} keyword={keyword} controlled_error={error:?} elapsed_ms={}",
            start.elapsed().as_millis()
        ),
    }
}
