//! Swift FFI 测试用回环 TLS 服务；只输出公开根证书与端口。
#[path = "../Tests/support/dns.rs"]
mod dns_support;
#[path = "../Tests/support/mod.rs"]
mod support;
#[tokio::main]
async fn main() {
    let fixture = support::fixture().await;
    let dns = dns_support::fixture().await;
    let plain = support::plain_fixture().await;
    println!(
        "{}",
        serde_json::json!({ "port": fixture.port, "http_port": plain.port, "dns_port": dns.port, "dot_port": dns.dot_port, "doh_port": dns.doh_port, "dns_certificate_pem": dns.pem, "dns_certificate_bytes": dns.root.len(), "dns_queries": dns.count.load(std::sync::atomic::Ordering::SeqCst), "certificate_pem": fixture.pem, "certificate_bytes": fixture.root.len() })
    );
    std::future::pending::<()>().await;
}
