//! Swift FFI 测试用回环 TLS 服务；只输出公开根证书与端口。
#[path = "../Tests/support/mod.rs"]
mod support;
#[tokio::main]
async fn main() {
    let fixture = support::fixture().await;
    let plain = support::plain_fixture().await;
    println!(
        "{}",
        serde_json::json!({ "port": fixture.port, "http_port": plain.port, "certificate_pem": fixture.pem, "certificate_bytes": fixture.root.len() })
    );
    std::future::pending::<()>().await;
}
