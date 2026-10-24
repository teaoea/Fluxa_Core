//! 演示经 C ABI 受理 VPN 前置检查、查询报告并停止；不接管系统 VPN、不改系统设置。
//! 探测目标是本进程内只接受 TCP、从不回应 TLS 的回环端点，检查停留在探测阶段直至停止。
use nodus_core::ffi;

#[allow(unsafe_code)]
fn command(handle: u64, request: &serde_json::Value) -> serde_json::Value {
    let bytes = serde_json::to_vec(request).expect("请求可编码");
    // SAFETY: 不可变请求字节在同步调用期间有效，内核不保存借用。
    let output = unsafe { ffi::nodus_core_command_v2(handle, bytes.as_ptr(), bytes.len()) };
    assert_ne!(output, 0, "响应句柄必须有效");
    let length = ffi::nodus_core_buffer_length(output);
    let mut reply = vec![0u8; length];
    // SAFETY: 目标缓冲区独占可写且容量等于内核报告的长度，与内核缓冲区不重叠。
    let copied = unsafe { ffi::nodus_core_buffer_copy(output, reply.as_mut_ptr(), reply.len()) };
    assert_eq!(copied, length);
    assert_eq!(ffi::nodus_core_destroy(output), 0);
    serde_json::from_slice(&reply).expect("响应为 JSON")
}

fn main() {
    assert_ne!(ffi::nodus_core_capabilities() & 1 << 32, 0, "需要能力位 32");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("回环端口");
    let probe = format!(
        "https://127.0.0.1:{}/health",
        listener.local_addr().unwrap().port()
    );
    let handle = ffi::nodus_core_create();
    assert_ne!(handle, 0);
    let begin = command(
        handle,
        &serde_json::json!({
            "operation": "vpn_preflight_begin",
            "yaml": "rules: ['final,direct']\n",
            "probes": [probe],
            "deadline_seconds": 30,
        }),
    );
    let transaction = begin["vpn_transaction"]["transaction"]
        .as_u64()
        .expect("受理返回事务 ID");
    println!(
        "受理事务 {transaction}，阶段 {}",
        begin["vpn_transaction"]["phase"]
    );
    let report = command(
        handle,
        &serde_json::json!({"operation": "vpn_preflight_status", "transaction": transaction}),
    );
    println!(
        "检查报告：activation_permitted={}",
        report["vpn_preflight"]["activation_permitted"]
    );
    let stopped = command(
        handle,
        &serde_json::json!({"operation": "vpn_transaction_stop", "transaction": transaction}),
    );
    println!(
        "停止后阶段 {}，原因代码 {}，系统状态 {}",
        stopped["vpn_transaction"]["phase"],
        stopped["vpn_transaction"]["failure"]["code"],
        stopped["vpn_transaction"]["system"]
    );
    assert_eq!(ffi::nodus_core_destroy(handle), 0);
}
