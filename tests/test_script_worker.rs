//! automatic_expiry_rejects_new_jobs_but_does_not_interrupt_running_js：到期不打断在途脚本，并拒绝新任务。
//!
//! repeated_compilation_evicts_only_idle_threads_instead_of_exhausting_cache：验证同名业务契约。
//!
//! one_resource_uses_one_binding_with_shortest_configured_ttl：验证同名业务契约。
//!
//! - exact_name_without_extension_is_bound_and_source_is_loaded_lazily：验证本轮已登记契约。
//! - missing_or_unregistered_resource_is_excluded_without_reinterpreting_url：验证本轮已登记契约。
//! - ttl_comes_from_configuration_and_expiry_releases_the_thread：验证本轮已登记契约。
//! - clear_interrupts_active_script_and_joins_before_returning：验证本轮已登记契约。
//!
//! 测试文件：tests/test_script_worker.rs
//! 测试类型：单元测试、真实 QuickJS、存储隔离与线程生命周期测试。
//! 测试对象：src/script/worker.rs、src/storage.rs、src/script.rs。
//! 测试内容：命名绑定、懒加载、缺失/不匹配排除、源缓存与清理、自动到期及活动取消。
//! 测试依据：用户指定 scripts 单对象、name 精确引用、js-ttl 与独立线程约束。
//! 前置条件：私有临时目录仅作内部存储夹具，不冒充真实 App Group；不访问网络。
use super::*;
use crate::storage::NamedFile;
use serde_json::json;
use std::path::PathBuf;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fluxa-js-test-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn bind(&self, name: &str, ttl: u32) -> Arc<Bindings> {
        Bindings::from_file(
            Some(name),
            Some(&NamedFile {
                name: name.into(),
                path: self.0.join(name),
            }),
            vec![(name.into(), ttl)],
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[tokio::test]
async fn exact_name_without_extension_is_bound_and_source_is_loaded_lazily() {
    let root = Fixture::new();
    root.write("response", "() => 1");
    let bindings = root.bind("response", 3600);
    let binding = bindings.get("response", 3600).unwrap();
    assert_eq!(binding.path, root.0.join("response"));
    root.write("response", "() => 2");
    let cache = Arc::new(Cache::default());
    assert_eq!(
        cache.execute(binding.clone(), json!({})).await.unwrap(),
        json!(2)
    );
    root.write("response", "() => 3");
    assert_eq!(
        cache.execute(binding.clone(), json!({})).await.unwrap(),
        json!(2)
    );
    cache.clear().unwrap();
    assert!(cache.workers.lock().unwrap().is_empty());
    assert_eq!(cache.execute(binding, json!({})).await.unwrap(), json!(3));
    cache.close().unwrap();
}
#[test]
fn missing_or_unregistered_resource_is_excluded_without_reinterpreting_url() {
    let root = Fixture::new();
    root.write("response.js", "input=>input");
    let bindings = root.bind("response", 3600);
    assert!(!bindings.contains("response", 3600));
    let bindings = Bindings::from_file(
        Some("response.js"),
        Some(&NamedFile {
            name: "response.js".into(),
            path: root.0.join("response.js"),
        }),
        vec![("response".into(), 3600)],
    )
    .unwrap();
    assert!(!bindings.contains("response", 3600));
    let bindings = root.bind("response.js", 3600);
    assert!(bindings.contains("response.js", 3600));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.0.join("response.js"), root.0.join("link")).unwrap();
        assert!(!root.bind("link", 3600).contains("link", 3600));
    }
}
#[tokio::test]
async fn ttl_comes_from_configuration_and_expiry_releases_the_thread() {
    let root = Fixture::new();
    root.write("response", "// js-ttl: 0\n() => 1");
    let binding = root.bind("response", 1).get("response", 1).unwrap();
    let cache = Arc::new(Cache::default());
    assert_eq!(
        cache.execute(binding.clone(), json!(null)).await.unwrap(),
        json!(1)
    );
    root.write("response", "() => 2");
    assert_eq!(
        cache.execute(binding.clone(), json!(null)).await.unwrap(),
        json!(1)
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        cache
            .workers
            .lock()
            .unwrap()
            .values()
            .all(|w| w.closed.load(Ordering::Acquire))
    );
    assert_eq!(cache.execute(binding, json!(null)).await.unwrap(), json!(2));
    cache.close().unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clear_interrupts_active_script_and_joins_before_returning() {
    let root = Fixture::new();
    root.write("response", "() => { while(true) {} }");
    let binding = root.bind("response", 3600).get("response", 3600).unwrap();
    let cache = Arc::new(Cache::default());
    let running = cache.clone();
    let held = binding.clone();
    let task = tokio::spawn(async move { running.execute(held, json!(null)).await });
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if !cache.workers.lock().unwrap().is_empty() {
            break;
        }
        assert!(Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    cache.clear().unwrap();
    assert_eq!(task.await.unwrap(), Err(Error::Cancelled));
    assert!(cache.workers.lock().unwrap().is_empty());
    cache.close().unwrap();
    assert_eq!(
        cache.execute(binding, json!(null)).await,
        Err(Error::InvalidState)
    );
}

#[test]
fn one_resource_uses_one_binding_with_shortest_configured_ttl() {
    let root = Fixture::new();
    root.write("response", "input=>input");
    let bindings = Bindings::from_file(
        Some("response"),
        Some(&NamedFile {
            name: "response".into(),
            path: root.0.join("response"),
        }),
        vec![("response".into(), 3600), ("response".into(), 1)],
    )
    .unwrap();
    let first = bindings.get("response", 3600).unwrap();
    let second = bindings.get("response", 1).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.ttl, 1);
}

#[tokio::test]
async fn repeated_compilation_evicts_only_idle_threads_instead_of_exhausting_cache() {
    let root = Fixture::new();
    root.write("response", "input=>input");
    let cache = Arc::new(Cache::default());
    for index in 0..12 {
        let binding = root.bind("response", 3600).get("response", 3600).unwrap();
        assert_eq!(
            cache.execute(binding, json!(index)).await.unwrap(),
            json!(index)
        );
        assert!(cache.workers.lock().unwrap().len() <= 4);
    }
    cache.close().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_expiry_rejects_new_jobs_but_does_not_interrupt_running_js() {
    let root = Fixture::new();
    root.write("response", "() => {while(true) {}}");
    let binding = root.bind("response", 1).get("response", 1).unwrap();
    let cache = Arc::new(Cache::default());
    assert_eq!(
        cache.execute(binding.clone(), json!(null)).await,
        Err(Error::ScriptTimeout)
    );
    let expires = cache
        .workers
        .lock()
        .unwrap()
        .get(&binding.id)
        .unwrap()
        .expires;
    tokio::time::sleep_until(tokio::time::Instant::from_std(
        expires - Duration::from_millis(100),
    ))
    .await;
    let held = binding.clone();
    let running = cache.clone();
    let task = tokio::spawn(async move { running.execute(held, json!(null)).await });
    tokio::time::sleep_until(tokio::time::Instant::from_std(
        expires + Duration::from_millis(10),
    ))
    .await;
    assert_eq!(
        cache.execute(binding.clone(), json!(null)).await,
        Err(Error::Cancelled)
    );
    assert_eq!(
        task.await.unwrap(),
        Err(Error::ScriptTimeout),
        "TTL 不能冒充手动取消，必须让本次运行按自己的执行预算结束"
    );
    root.write("response", "() => 7");
    for _ in 0..100 {
        if cache
            .workers
            .lock()
            .unwrap()
            .get(&binding.id)
            .unwrap()
            .closed
            .load(Ordering::Acquire)
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(cache.execute(binding, json!(null)).await.unwrap(), json!(7));
    cache.close().unwrap();
}
