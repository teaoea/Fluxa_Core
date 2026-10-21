//! 优化构建测量生成与索引查询；不使用少量重复规则冒充十万条有效规则。
use fluxa_core::routing::{
    Config, Match, Policy, Rule, Target, Transport,
    runtime::{Layer, Snapshot, Source},
};
use std::{hint::black_box, time::Instant};
fn rule(kind: Match, value: String, policy: Policy) -> Rule {
    Rule {
        protocol: None,
        r#type: kind,
        value,
        policy,
        outbound: if policy == Policy::Proxy {
            Some("test".into())
        } else {
            None
        },
    }
}
fn percentile(values: &mut [u128], percent: usize) -> u128 {
    values.sort_unstable();
    values[(values.len() - 1) * percent / 100]
}
fn rss() -> String {
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".into())
}
// 数十条本地规则与四份顺序固定的远程订阅，复用同一生成流程。
fn sources(rules: Vec<Rule>) -> Vec<Source> {
    let local_count = rules.len().min(40);
    let width = (rules.len() - local_count).div_ceil(4).max(1);
    let mut groups: Vec<_> = (0..5)
        .map(|index| Source {
            id: format!("source-{index}"),
            layer: if index == 0 {
                Layer::Local
            } else {
                Layer::Remote
            },
            enabled: true,
            rules: Vec::new(),
        })
        .collect();
    for (index, rule) in rules.into_iter().enumerate() {
        let group = if index < local_count {
            0
        } else {
            1 + (index - local_count) / width
        };
        groups[group].rules.push(rule);
    }
    groups
}
fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    if let Some(position) = arguments.iter().position(|arg| arg == "--load") {
        println!("baseline_rss_kib={}", rss());
        let started = Instant::now();
        let store = fluxa_core::routing::store::Store::open(&arguments[position + 1]).unwrap();
        let snapshot = store.snapshot().unwrap();
        println!(
            "cold_load_us={} cold_rss_kib={}",
            started.elapsed().as_micros(),
            rss()
        );
        measure(&snapshot, snapshot.document().rules.len());
        store.close().unwrap();
        return;
    }
    let count: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "100000".into())
        .parse()
        .expect("规则规模必须为数字");
    println!("baseline_rss_kib={}", rss());
    let mut rules = Vec::with_capacity(count);
    for i in 0..count {
        let (kind, value) = match i % 10 {
            0..=2 => (Match::Domain, format!("d{i}.example.net")),
            3..=4 => (Match::DomainSuffix, format!("s{i}.example.org")),
            5..=6 => (
                Match::IpCidr,
                format!(
                    "{}.{}.{}.{}/32",
                    10 + ((i >> 24) & 15),
                    (i >> 16) & 255,
                    (i >> 8) & 255,
                    i & 255
                ),
            ),
            7 => (
                Match::IpCidr,
                format!("2001:db8::{:x}:{:x}/128", i >> 16, i & 65535),
            ),
            8 => (Match::DomainKeyword, format!("keyword-{i}-")),
            _ => (Match::DestinationPort, (20000 + i / 10).to_string()),
        };
        rules.push(rule(
            kind,
            value,
            if i % 97 == 0 {
                Policy::Reject
            } else {
                Policy::Proxy
            },
        ));
    }
    println!("input_rss_kib={}", rss());
    let start = Instant::now();
    let snapshot = Snapshot::generate(
        Config {
            fallback: Policy::Direct,
            ..Config::default()
        },
        sources(rules),
        1,
    )
    .expect("规模超过内存或输入限制时必须明确失败");
    let compile = start.elapsed();
    println!(
        "rules_input={count} rules_effective={} compile_ms={} stable_rss_kib={}",
        snapshot.document().rules.len(),
        compile.as_millis(),
        rss()
    );
    // 重启后的查询单独采样，两种缓存使用相同规则和调用路径。
    let snapshot = if std::env::args().any(|arg| arg == "--reopen") {
        let path = std::env::temp_dir().join(format!("fluxa-cache-query-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let input = snapshot
            .document()
            .rules
            .iter()
            .map(|row| row.rule.clone())
            .collect();
        let store = fluxa_core::routing::store::Store::memory();
        store
            .update(snapshot.document().settings.clone(), sources(input))
            .unwrap();
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("runtime-rules.bin"),
            store.snapshot().unwrap().to_binary().unwrap().as_ref(),
        )
        .unwrap();
        store.close().unwrap();
        drop(store);
        drop(snapshot);
        let started = Instant::now();
        let store =
            fluxa_core::routing::store::Store::open(path.join("runtime-rules.bin")).unwrap();
        let snapshot = store.snapshot().unwrap();
        println!(
            "reopen_us={} reopened_rss_kib={}",
            started.elapsed().as_micros(),
            rss()
        );
        store.close().unwrap();
        std::fs::remove_dir_all(path).unwrap();
        snapshot
    } else {
        std::sync::Arc::new(snapshot)
    };
    if std::env::args().any(|arg| arg == "--persist") {
        let input: Vec<_> = snapshot
            .document()
            .rules
            .iter()
            .map(|row| row.rule.clone())
            .collect();
        drop(snapshot);
        let path = std::env::temp_dir().join(format!("fluxa-runtime-bench-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        let store = fluxa_core::routing::store::Store::memory();
        let start = Instant::now();
        if std::env::args().any(|arg| arg == "--background") {
            let service = fluxa_core::routing::background::Service::new(
                store,
                std::sync::Arc::new(fluxa_core::outbound::Manager::default()),
            );
            for rules in [input.clone(), input] {
                let submitted = Instant::now();
                let job = service
                    .start(
                        Config {
                            fallback: Policy::Direct,
                            ..Config::default()
                        },
                        sources(rules),
                    )
                    .expect("后台提交必须成功");
                println!("background_submit_us={}", submitted.elapsed().as_micros());
                loop {
                    let job = service.job(job.id).unwrap();
                    match job.phase {
                        fluxa_core::routing::background::Phase::Compiling => {
                            std::thread::sleep(std::time::Duration::from_millis(1))
                        }
                        fluxa_core::routing::background::Phase::Succeeded => break,
                        _ => panic!("后台生成失败：{:?}", job.error),
                    }
                }
            }
            std::fs::write(
                path.join("runtime-rules.bin"),
                service.export_binary().unwrap().as_ref(),
            )
            .unwrap();
            service.close().unwrap();
            // Service 关闭后仍可从磁盘验证完整生成产物。
        } else {
            let first = store
                .update(
                    Config {
                        fallback: Policy::Direct,
                        ..Config::default()
                    },
                    sources(input.clone()),
                )
                .expect("第一次缓存提交必须实际成功");
            print_update(1, &first);
            let second = store
                .update(
                    Config {
                        fallback: Policy::Direct,
                        ..Config::default()
                    },
                    sources(input),
                )
                .expect("新旧快照共存更新必须实际成功");
            print_update(2, &second);
            println!("active_snapshot_rss_kib={}", rss());
            std::fs::write(
                path.join("runtime-rules.bin"),
                store.snapshot().unwrap().to_binary().unwrap().as_ref(),
            )
            .unwrap();
            store.close().unwrap();
        }
        println!(
            "two_atomic_updates_ms={} closed_rss_kib={}",
            start.elapsed().as_millis(),
            rss()
        );
        // 同步模式在上面的分支内关闭，异步模式由 Service 关闭。
        let reopen_started = Instant::now();
        let reopened = fluxa_core::routing::store::Store::open(path.join("runtime-rules.bin"))
            .expect("磁盘缓存必须能够重新加载");
        println!(
            "reopen_us={} reopened_rss_kib={}",
            reopen_started.elapsed().as_micros(),
            rss()
        );
        let persisted = std::fs::read_dir(&path)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("runtime-rules.")
            })
            .unwrap();
        println!("cache_bytes={}", persisted.metadata().unwrap().len());
        assert_eq!(reopened.snapshot().unwrap().document().rules.len(), count);
        reopened.close().unwrap();
        if std::env::args().any(|arg| arg == "--keep-cache") {
            println!("cache_directory={}", path.display());
        } else {
            std::fs::remove_dir_all(path).unwrap();
        }
        return;
    }
    measure(&snapshot, count);
}

fn measure(snapshot: &Snapshot, count: usize) {
    for (case, name) in [
        ("first", "d0.example.net".to_owned()),
        ("last", format!("d{}.example.net", count - 8)),
        ("miss", "not-present.example.net".to_owned()),
    ] {
        let target = Target {
            domain: Some(name),
            address: Some("192.0.2.1".into()),
            port: 443,
            transport: Transport::Tcp,
        };
        assert_eq!(
            snapshot.evaluate(&target).unwrap(),
            snapshot.evaluate_reference(&target).unwrap()
        );
        let mut values = Vec::with_capacity(20000);
        for _ in 0..20000 {
            let start = Instant::now();
            black_box(snapshot.evaluate(black_box(&target)).unwrap());
            values.push(start.elapsed().as_nanos());
        }
        let p50 = percentile(&mut values, 50);
        let p95 = percentile(&mut values, 95);
        let p99 = percentile(&mut values, 99);
        println!("case={case} p50_ns={p50} p95_ns={p95} p99_ns={p99}");
    }
    let target = Target {
        domain: Some("s14.example.org".into()),
        address: Some("192.0.2.1".into()),
        port: 443,
        transport: Transport::Tcp,
    };
    let start = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let snapshot = &snapshot;
            let target = &target;
            scope.spawn(move || {
                for _ in 0..100000 {
                    black_box(snapshot.evaluate(black_box(target)).unwrap());
                }
            });
        }
    });
    println!(
        "threads=4 queries=400000 throughput_per_second={:.0}",
        400000.0 / start.elapsed().as_secs_f64()
    );
}

fn print_update(number: usize, commit: &fluxa_core::routing::store::Commit) {
    let t = &commit.timings;
    println!(
        "update={number} compile_us={} encode_us={} validate_us={} write_sync_us={} switch_us={} directory_sync_us={}",
        t.compile.as_micros(),
        t.encode.as_micros(),
        t.validate.as_micros(),
        t.write_sync.as_micros(),
        t.switch.as_micros(),
        t.directory_sync.as_micros()
    );
}
