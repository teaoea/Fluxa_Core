//! 四档构建/更新/并发恢复验收：原始输入由调用方拥有，规则准入池不冒充进程 RSS 硬限制。
//! cargo run --release --example rules_acceptance -- 10000 mixed
use fluxa_core::{
    error::Error,
    routing::{
        Config, Match, Policy, Rule, Target, Transport,
        runtime::{self, Layer, Snapshot, Source},
        store::Store,
    },
};
use std::{
    sync::{Arc, Barrier},
    time::Instant,
};
const WORKERS: usize = 4;
fn sources(count: usize, shape: &str) -> Vec<Source> {
    let mut groups: Vec<_> = (0..8)
        .map(|i| Source {
            id: format!("source-{i}"),
            layer: if i < 2 { Layer::Local } else { Layer::Remote },
            enabled: true,
            rules: Vec::new(),
        })
        .collect();
    for i in 0..count {
        let (kind, value, protocol) = match shape {
            "suffix" => (
                Match::DomainSuffix,
                format!("{}example.net", format!("{i:x}.").repeat(35)),
                None,
            ),
            "keyword" => (
                Match::DomainKeyword,
                format!("k{i}-{}", "x".repeat(200)),
                None,
            ),
            "duplicate" => (Match::Domain, "duplicate.example.net".into(), None),
            "mixed" => match i % 8 {
                0 => (
                    Match::Domain,
                    format!("d{i}.example.net"),
                    Some(Transport::Tcp),
                ),
                1 => (
                    Match::DomainSuffix,
                    format!("s{i}.example.net"),
                    Some(Transport::Udp),
                ),
                2 => (
                    Match::DomainSuffix,
                    format!("a.b.c.d.e.f.s{i}.example.net"),
                    None,
                ),
                3 => (Match::DomainKeyword, format!("keyword-{i}-"), None),
                4 => (
                    Match::IpCidr,
                    format!("10.{}.{}.{}/32", (i >> 16) & 255, (i >> 8) & 255, i & 255),
                    None,
                ),
                5 => (
                    Match::IpCidr,
                    format!("2001:db8::{:x}:{:x}/128", i >> 16, i & 65535),
                    None,
                ),
                _ => (Match::Domain, format!("d{i}.example.net"), None),
            },
            _ => panic!("未知形态"),
        };
        groups[i % 8].rules.push(Rule {
            r#type: kind,
            value,
            protocol,
            policy: if i % 97 == 0 {
                Policy::Reject
            } else {
                Policy::Direct
            },
            outbound: None,
        });
    }
    groups
}
fn controlled(error: Error) -> bool {
    matches!(
        error,
        Error::ResourceLimit | Error::ResourceBudgetExceeded { .. }
    )
}
fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "10000".into())
        .parse()
        .unwrap();
    let shape = std::env::args().nth(2).unwrap_or_else(|| "mixed".into());
    assert!((1..=runtime::MAX_RULES).contains(&count));
    let path = std::env::temp_dir().join(format!("fluxa-rules-acceptance-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let store = Store::memory();
    let mut retained = Vec::new();
    let started = Instant::now();
    for round in 0..3 {
        let begin = Instant::now();
        match store.update(Config::default(), sources(count, &shape)) {
            Ok(commit) => {
                retained.push(store.snapshot().unwrap());
                println!(
                    "phase=update round={round} elapsed_us={} compile_us={} encode_us={} reserved_bytes={} peak_reserved_bytes={}",
                    begin.elapsed().as_micros(),
                    commit.timings.compile.as_micros(),
                    commit.timings.encode.as_micros(),
                    runtime::memory_usage().reserved_bytes,
                    runtime::memory_usage().peak_reserved_bytes
                );
            }
            Err(error) if controlled(error) => {
                println!(
                    "phase=update round={round} controlled_code={}",
                    error.code().value()
                );
                break;
            }
            Err(error) => panic!("非资源错误：{error:?}"),
        }
    }
    if let Ok(snapshot) = store.snapshot() {
        std::fs::write(
            path.join("runtime-rules.bin"),
            snapshot.to_binary().unwrap().as_ref(),
        )
        .unwrap();
    }
    store.close().unwrap();
    drop(store);
    if retained.is_empty() {
        assert_ne!(shape, "duplicate", "十万重复规则应合并而不是误报资源失败");
        assert_eq!(runtime::memory_usage().reserved_bytes, 0);
        println!(
            "phase=complete count={count} shape={shape} admission=rejected peak_reserved_bytes={}",
            runtime::memory_usage().peak_reserved_bytes
        );
        std::fs::remove_dir_all(path).unwrap();
        return;
    }
    // 已关闭 Store 的旧快照仍只读占用内存，其预算不能提前归还。
    let gate = Arc::new(Barrier::new(WORKERS));
    std::thread::scope(|scope| {
        for worker in 0..WORKERS {
            let gate = gate.clone();
            let path = &path;
            let shape = &shape;
            scope.spawn(move || {
                let directory = path.join(format!("worker-{worker}"));
                std::fs::create_dir(&directory).unwrap();
                std::fs::copy(path.join("runtime-rules.bin"), directory.join("runtime-rules.bin")).unwrap();
                gate.wait();
                let reopened = Store::open(directory.join("runtime-rules.bin"));
                let loaded = match &reopened {
                    Ok(store) => Some(store.snapshot().unwrap()),
                    Err(error) if controlled(*error) => None,
                    Err(error) => panic!("恢复出现非资源错误：{error:?}"),
                };
                let generated = Snapshot::generate(Config::default(), sources(count, shape), 9);
                if let Err(error) = &generated { assert!(controlled(*error), "{error:?}"); }
                gate.wait();
                println!("phase=concurrent worker={worker} loaded={} generated={} reserved_bytes={} peak_reserved_bytes={}", loaded.is_some(), generated.is_ok(), runtime::memory_usage().reserved_bytes, runtime::memory_usage().peak_reserved_bytes);
                if let Some(snapshot) = loaded {
                    let query = Target { domain: Some("missing.example.org".into()), address: Some("192.0.2.1".into()), port: 443, transport: Transport::Tcp };
                    assert_eq!(snapshot.evaluate(&query).unwrap().policy, Policy::Reject);
                }
                if let Ok(store) = reopened { store.close().unwrap(); }
            });
        }
    });
    drop(retained);
    assert_eq!(
        runtime::memory_usage().reserved_bytes,
        0,
        "全部实际对象释放后归还共享池"
    );
    // 压力失败不会污染预算或活动文件；释放旧引用后仍可恢复。
    let store = Store::open(path.join("runtime-rules.bin")).unwrap();
    store.close().unwrap();
    drop(store);
    let usage = runtime::memory_usage();
    assert_eq!(usage.reserved_bytes, 0);
    assert!(usage.peak_reserved_bytes <= usage.limit_bytes);
    println!(
        "phase=complete count={count} shape={shape} total_us={} peak_reserved_bytes={} limit_bytes={}",
        started.elapsed().as_micros(),
        usage.peak_reserved_bytes,
        usage.limit_bytes
    );
    std::fs::remove_dir_all(path).unwrap();
}
