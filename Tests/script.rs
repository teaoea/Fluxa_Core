//! 脚本引擎真实执行与失败恢复；不以模拟返回值替代运行时。
use libfluxa_core::{
    error::Error,
    script::{MAX_DATA, MAX_SOURCE, execute},
};
use serde_json::{Value, json};
use std::sync::{Mutex, MutexGuard};
static TESTS: Mutex<()> = Mutex::new(());
fn serial() -> MutexGuard<'static, ()> {
    TESTS.lock().unwrap()
}

#[test]
fn transforms_json_and_preserves_unicode_null_and_keys() {
    let _guard = serial();
    assert_eq!(execute("input => ({城市: input.city, status: 200, body: input.body.toUpperCase(), empty: null})", &json!({"city":"武汉市", "body":"hello"})).unwrap(), json!({"城市":"武汉市", "status":200,"body":"HELLO","empty":null}));
    assert_eq!(
        execute(
            "input => input",
            &json!({"__proto__":{"x":1},"constructor":2})
        )
        .unwrap(),
        json!({"__proto__":{"x":1},"constructor":2})
    );
    assert_eq!(execute("() => null", &Value::Null).unwrap(), Value::Null);
}
#[test]
fn rejects_lossy_or_executable_results() {
    let _guard = serial();
    for source in [
        "() => undefined",
        "() => NaN",
        "() => Infinity",
        "() => 1n",
        "() => (() => 1)",
        "() => Promise.resolve(1)",
        "() => new Date()",
        "() => ({a:undefined})",
        "() => [1,,3]",
        "() => ({get x(){return 1}})",
        "() => {let x={};x.self=x;return x}",
        "() => ({[Symbol()]:1})",
        "() => Object.defineProperty({}, 'x', {value:1})",
    ] {
        assert_eq!(
            execute(source, &Value::Null),
            Err(Error::Script),
            "{source}"
        );
    }
}
#[test]
fn timeout_and_exception_do_not_poison_next_execution() {
    let _guard = serial();
    assert_eq!(
        execute("() => {while(true){}}", &Value::Null),
        Err(Error::ScriptTimeout)
    );
    for source in [
        "() => {throw new Error('private payload')}",
        "not valid syntax",
        "() => {const f=()=>f();return f()}",
        "() => {const x=[];for(;;)x.push('x'.repeat(100000))}",
    ] {
        assert!(execute(source, &Value::Null).is_err());
        assert_eq!(execute("input => input + 1", &json!(1)).unwrap(), json!(2));
    }
}
#[test]
fn no_ambient_io_or_state_between_calls() {
    let _guard = serial();
    assert_eq!(execute("() => ['fetch','XMLHttpRequest','require','process','std','os','document','WebSocket','setTimeout'].map(k=>typeof globalThis[k])", &Value::Null).unwrap(), json!(["undefined","undefined","undefined","undefined","undefined","undefined","undefined","undefined","undefined"]));
    execute("() => {globalThis.secret=42;return 1}", &Value::Null).unwrap();
    assert_eq!(
        execute("() => typeof secret", &Value::Null).unwrap(),
        json!("undefined")
    );
    assert!(execute("() => import('file:///etc/passwd')", &Value::Null).is_err());
}
#[test]
fn bounds_source_input_output_depth_and_nodes() {
    let _guard = serial();
    assert_eq!(
        execute(&" ".repeat(MAX_SOURCE + 1), &Value::Null),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        execute("input=>input", &json!("x".repeat(MAX_DATA))),
        Err(Error::ResourceLimit)
    );
    assert_eq!(
        execute("()=>'x'.repeat(1048576)", &Value::Null),
        Err(Error::ResourceLimit)
    );
    assert!(
        execute(
            "()=>{let v=0;for(let i=0;i<70;i++)v={v};return v}",
            &Value::Null
        )
        .is_err()
    );
    assert!(execute("()=>Array(20000).fill(1)", &Value::Null).is_err());
}
#[test]
fn serializer_cannot_be_replaced_by_script() {
    assert_eq!(execute("()=>{JSON.stringify=()=> 'fake';Object.getOwnPropertyDescriptor=()=>null;return {value:7}}", &Value::Null).unwrap(), json!({"value":7}));
    assert!(execute("()=>{Object.prototype.toJSON=()=>1;return 2}", &Value::Null).is_err());
}
