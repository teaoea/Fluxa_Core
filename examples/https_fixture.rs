//! Swift FFI 测试用回环 TLS 服务；只输出公开根证书与端口。
#[path = "../Tests/support/dns.rs"]
mod dns_support;
#[path = "../Tests/support/mod.rs"]
mod support;
#[tokio::main]
async fn main() {
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let outbound_tcp_port = tcp.local_addr().unwrap().port();
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Ok((mut socket, _)) = tcp.accept().await {
            tokio::spawn(async move {
                let mut bytes = [0; 65536];
                while let Ok(size) = socket.read(&mut bytes).await {
                    if size == 0 {
                        break;
                    }
                    if socket.write_all(&bytes[..size]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    let udp = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let outbound_udp_port = udp.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut bytes = [0; 65536];
        while let Ok((size, peer)) = udp.recv_from(&mut bytes).await {
            let _ = udp.send_to(&bytes[..size], peer).await;
        }
    });
    let fixture = support::fixture().await;
    let dns = dns_support::fixture().await;
    let plain = support::plain_fixture().await;
    println!(
        "{}",
        serde_json::json!({ "outbound_tcp_port": outbound_tcp_port, "outbound_udp_port": outbound_udp_port, "port": fixture.port, "http_port": plain.port, "dns_port": dns.port, "dot_port": dns.dot_port, "doh_port": dns.doh_port, "dns_certificate_pem": dns.pem, "dns_certificate_bytes": dns.root.len(), "dns_queries": dns.count.load(std::sync::atomic::Ordering::SeqCst), "certificate_pem": fixture.pem, "certificate_bytes": fixture.root.len() })
    );
    std::future::pending::<()>().await;
}
