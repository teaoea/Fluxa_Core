//! 六项索引缺陷修复的可运行示例：合法哈希碰撞、规则来源和缓存恢复。
//! 仅创建独占临时规则目录，不监听网络，不接管 VPN，不修改用户配置。
use nodus_core::routing::{
    Config, Match, Policy, Rule, Target, Transport,
    runtime::{Layer, Source},
    store::Store,
};
use std::{collections::HashMap, path::PathBuf};

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const COLLISION_MASK: usize = 255;
const DOMAIN_COUNT: usize = 65;

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("临时规则目录清理失败：{error}");
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut buckets: HashMap<usize, Vec<String>> = HashMap::new();
    let mut names = None;
    for id in 0..100_000 {
        let name = format!("h{id}.slot.example");
        let hash = name.bytes().fold(FNV_OFFSET_BASIS, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
        });
        let bucket = buckets.entry(hash as usize & COLLISION_MASK).or_default();
        bucket.push(name);
        if bucket.len() == DOMAIN_COUNT {
            names = Some(std::mem::take(bucket));
            break;
        }
    }
    let names = names.ok_or("示例未构造到所需碰撞")?;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "nodus-flat-example-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&path)?;
    let directory = Directory(path);
    let store = Store::memory();
    store.update(
        Config::default(),
        vec![Source {
            id: "collision-example".into(),
            layer: Layer::Local,
            enabled: true,
            rules: names
                .iter()
                .map(|name| Rule {
                    r#type: Match::Domain,
                    value: name.clone(),
                    protocol: None,
                    policy: Policy::Reject,
                    outbound: None,
                })
                .collect(),
        }],
    )?;
    // CLI 宿主保存内存编码，Core 不持久化。
    let file = directory.0.join("rules.bin");
    std::fs::write(&file, store.snapshot()?.to_binary()?.as_ref())?;
    store.close()?;
    let restored = Store::open(&file)?;
    let snapshot = restored.snapshot()?;
    for (id, domain) in names.iter().enumerate() {
        let decision = snapshot.evaluate(&Target {
            domain: Some(domain.clone()),
            address: None,
            port: 443,
            transport: Transport::Tcp,
        })?;
        assert_eq!(decision.policy, Policy::Reject);
        assert_eq!(decision.rule_index, Some(id));
    }
    restored.close()?;
    println!("65 个合法碰撞域名全部构建、恢复并命中原规则；未开启网络监听或系统 VPN。");
    Ok(())
}
