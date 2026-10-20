//! 受限同步脚本执行：每次新建运行时，不提供网络、文件、模块或原生能力。
//! 公共同步门面不负责下载；声明式脚本由资源管线校验和保存，再经隔离工作线程执行。
pub(crate) mod worker;
use crate::{
    budget::ByteBudget,
    error::{Error, Result},
};
use rquickjs::{Context, Function, Runtime, Value};
use serde_json::Value as Json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub const MAX_SOURCE: usize = 256 * 1024;
pub const MAX_DATA: usize = 1024 * 1024;
pub const MAX_HEAP: usize = 32 * 1024 * 1024;
pub const MAX_TIME: Duration = Duration::from_millis(250);
// 跨实例最多四份运行时预算；并非操作系统级进程内存限制。
static HEAPS: ByteBudget = ByteBudget::new(4 * MAX_HEAP);

/// 输入、输出仅为 JSON；源码是同步函数表达式，例如 `(input) => input`。
/// 此接口阻塞调用线程，客户端应在后台调用，不执行 Promise 作业队列。
pub fn execute(source: &str, input: &Json) -> Result<Json> {
    execute_cancellable(source, input, Arc::new(AtomicBool::new(false)))
}
/// 仅内核脚本工作线程提供取消令牌；JS 无法修改权限或保留宿主引用。
pub(crate) fn execute_cancellable(
    source: &str,
    input: &Json,
    canceled: Arc<AtomicBool>,
) -> Result<Json> {
    if canceled.load(Ordering::Acquire) {
        return Err(Error::Cancelled);
    }

    if source.is_empty() || source.len() > MAX_SOURCE {
        return Err(Error::InvalidInput);
    }
    validate_input(input)?;
    let mut input_bytes = LimitedJson(Vec::new());
    serde_json::to_writer(&mut input_bytes, input).map_err(|_| Error::ResourceLimit)?;
    let input = input_bytes.0;
    let _lease = HEAPS.reserve(MAX_HEAP)?;
    let runtime = Runtime::new().map_err(|_| Error::ResourceLimit)?;
    runtime.set_memory_limit(MAX_HEAP);
    runtime.set_max_stack_size(512 * 1024);
    let deadline = Instant::now() + MAX_TIME;
    let interrupted = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&interrupted);
    let cancellation = canceled.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        let stop = Instant::now() >= deadline;
        if stop {
            flag.store(true, Ordering::Relaxed);
        }
        stop || cancellation.load(Ordering::Acquire)
    })));
    let context = Context::full(&runtime).map_err(|_| Error::ResourceLimit)?;
    let result = context.with(|ctx| -> rquickjs::Result<String> {
        // 先捕获并冻结序列化依赖，脚本不能通过篡改原型改变结果校验。
        let serialize: Function = ctx.eval(SERIALIZER)?;
        let function: Function = ctx.eval(format!("(\n{source}\n)"))?;
        let value: Value = function.call((ctx.json_parse(input)?,))?;
        serialize.call((value,))
    });
    if canceled.load(Ordering::Acquire) {
        return Err(Error::Cancelled);
    }
    if interrupted.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err(Error::ScriptTimeout);
    }
    let output = result.map_err(|error| match error {
        rquickjs::Error::Allocation => Error::ResourceLimit,
        _ => Error::Script,
    })?;
    if output.len() > MAX_DATA {
        return Err(Error::ResourceLimit);
    }
    // serde 的深度检查也保留；不将 JS 对象指针带出运行时。
    serde_json::from_str(&output).map_err(|_| Error::Script)
}

// 编码时限制写入长度，避免先分配整个超大编码结果再检查。
struct LimitedJson(Vec<u8>);
impl std::io::Write for LimitedJson {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_DATA.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("JSON 超过上限"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// 在递归 JSON 编码之前用显式栈限制深度，避免 Rust 调用栈被深层输入耗尽。
fn validate_input(input: &Json) -> Result<()> {
    let mut pending = vec![(input, 0)];
    let mut nodes = 0;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if nodes > 16384 || depth > 64 {
            return Err(Error::ResourceLimit);
        }
        match value {
            Json::Array(values) => {
                if values.len() + pending.len() > 16384 {
                    return Err(Error::ResourceLimit);
                }
                pending.extend(values.iter().map(|v| (v, depth + 1)));
            }
            Json::Object(values) => {
                if values.len() + pending.len() > 16384 {
                    return Err(Error::ResourceLimit);
                }
                pending.extend(values.values().map(|v| (v, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(())
}

// 不调用用户 toJSON 或 getter；拒绝循环、稀疏数组、Symbol、非有限数和非数据对象。
// 复制到无原型对象，避免 JSON.stringify 的隐式数据丢失或原型污染。
const SERIALIZER: &str = r#"
(() => {
  const ownKeys = Reflect.ownKeys, descriptor = Object.getOwnPropertyDescriptor;
  const prototype = Object.getPrototypeOf, create = Object.create;
  const define = Object.defineProperty, isArray = Array.isArray;
  const finite = Number.isFinite, stringify = JSON.stringify;
  const objectPrototype = Object.prototype, arrayPrototype = Array.prototype;
  for (const p of [Object.prototype, Array.prototype, Function.prototype,
                   Set.prototype, String.prototype, Number.prototype, Boolean.prototype]) {
    Object.freeze(p);
  }
  const SetType = Set, StringType = String;
  return value => {
    const active = new SetType();
    let nodes = 0;
    const copy = (v, depth) => {
      if (++nodes > 16384 || depth > 64) throw new Error('data limit');
      if (v === null || typeof v === 'string' || typeof v === 'boolean') return v;
      if (typeof v === 'number' && finite(v)) return v;
      if (typeof v !== 'object' || active.has(v)) throw new Error('non JSON');
      const array = isArray(v), p = prototype(v);
      if (array ? p !== arrayPrototype : p !== objectPrototype && p !== null) {
        throw new Error('non data object');
      }
      active.add(v);
      const result = array ? [] : create(null);
      const keys = ownKeys(v);
      let count = 0;
      for (const key of keys) {
        if (array && key === 'length') continue;
        const d = descriptor(v, key);
        if (typeof key !== 'string' || !d || !('value' in d) || !d.enumerable) {
          throw new Error('non data property');
        }
        if (array && key !== StringType(count++)) throw new Error('sparse array');
        define(result, key, {value: copy(d.value, depth + 1), enumerable: true, writable: true, configurable: true});
      }
      if (array && count !== v.length) throw new Error('sparse array');
      active.delete(v);
      return result;
    };
    return stringify(copy(value, 0));
  };
})()
"#;

#[cfg(test)]
#[path = "../tests/test_script.rs"]
mod test_script;
