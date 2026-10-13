//! 演示 FFI 响应句柄耗尽时，TCP 数据不会消费；只使用本地回环服务。
//! 运行：cargo run --example ffi_response_retry；不需要节点、证书或系统隧道。
use base64::{Engine as _, engine::general_purpose::STANDARD};
use libfluxa_core::ffi;
use serde_json::{Value, json};
use std::{io::Write, net::TcpListener};
#[allow(unsafe_code)]
fn call(engine: u64, value: Value) -> u64 {
    let bytes = serde_json::to_vec(&value).unwrap();
    // SAFETY: 输入在同步调用期间有效且只读，不保留跨语言指针。
    unsafe { ffi::fluxa_engine_command(engine, bytes.as_ptr(), bytes.len()) }
}
#[allow(unsafe_code)]
fn reply(handle: u64) -> Value {
    let mut bytes = vec![0; ffi::fluxa_engine_buffer_length(handle)];
    // SAFETY: 输出向量容量足够，调用期间独占且不与内核内存重叠。
    unsafe {
        ffi::fluxa_engine_buffer_copy(handle, bytes.as_mut_ptr(), bytes.len());
    }
    let reply = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(ffi::fluxa_engine_destroy(handle), 0);
    reply
}
fn main() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.write_all(b"FIRSTSECOND").unwrap();
    });
    let engine = ffi::fluxa_engine_create();
    let opened = reply(call(
        engine,
        json!({"operation":"outbound_open", "address":address.to_string(),
        "transport":"tcp", "policy":"direct", "timeout_ms":1000}),
    ));
    let id = opened["outbound_id"].as_u64().unwrap();
    peer.join().unwrap();
    let mut outputs = Vec::new();
    for _ in 0..63 {
        let handle = call(engine, json!({"operation":"outbound_status"}));
        assert_ne!(handle, 0);
        outputs.push(handle);
    }
    let read = json!({"operation":"outbound_read", "id":id, "maximum":11, "timeout_ms":1000});
    assert_eq!(call(engine, read.clone()), 0);
    assert_eq!(ffi::fluxa_engine_destroy(outputs.pop().unwrap()), 0);
    let response = reply(call(engine, read));
    assert_eq!(
        STANDARD.decode(response["body"].as_str().unwrap()).unwrap(),
        b"FIRSTSECOND"
    );
    for handle in outputs {
        assert_eq!(ffi::fluxa_engine_destroy(handle), 0);
    }
    assert_eq!(ffi::fluxa_engine_destroy(engine), 0);
    println!("响应名额不足在读取前拒绝；释放名额后重试，完整原始 TCP 数据保留。仅验证回环 ABI。");
}
