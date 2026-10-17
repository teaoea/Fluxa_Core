//! 真实 UDP 首包纯裁决 + macOS OS 进程规则出站示例；仅本机回环，不接管 VPN。
use libfluxa_core::{
    configuration::Document,
    routing::{Policy, Target, Transport},
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let snapshot =
        Document::from_yaml("rules: ['protocol,quic,example.com,direct', 'final,reject']")?
            .compile_local_rules(1)?;
    let target = Target {
        domain: Some("a.example.com".into()),
        address: None,
        port: 5555,
        transport: Transport::Udp,
    };
    // 满足识别器边界的最小首部样本，仅演示裁决，不宣称完成 QUIC/TLS 握手。
    let mut packet = vec![0xc0, 0, 0, 0, 1, 1, 1, 1, 2, 0, 17];
    packet.extend([0; 17]);
    assert_eq!(
        snapshot.evaluate_datagram(&target, &packet)?.policy,
        Policy::Direct
    );
    assert_eq!(
        snapshot.evaluate_datagram(&target, b"plain UDP")?.policy,
        Policy::Reject
    );
    println!(
        "{}",
        serde_json::to_string(&snapshot.evaluate_datagram(&target, &packet)?)?
    );
    #[cfg(target_os = "macos")]
    {
        use libfluxa_core::outbound::{Family, Manager, RoutedTarget};
        let name = std::env::current_exe()?
            .file_name()
            .ok_or("可执行文件名缺失")?
            .to_str()
            .ok_or("非 UTF-8 名称")?
            .to_owned();
        let scalar = serde_json::to_string(&format!("process-name,{name},direct"))?;
        let manager = Manager::default();
        manager.apply_configuration(&format!("rules: [{scalar}, 'final,reject']"))?;
        let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
        let opened = manager
            .connect_configured(RoutedTarget {
                target: Target {
                    domain: None,
                    address: Some("127.0.0.1".into()),
                    port: peer.local_addr()?.port(),
                    transport: Transport::Udp,
                },
                timeout_ms: 1000,
                family: Family::Ipv4,
                mapping_generation: None,
            })
            .await?;
        manager
            .write(opened.connection.id, b"OS process identity", 1000)
            .await?;
        let mut bytes = [0; 64];
        let (length, _) = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            peer.recv_from(&mut bytes),
        )
        .await??;
        assert_eq!(&bytes[..length], b"OS process identity");
        manager.close(opened.connection.id).await?;
        manager.stop().await?;
    }
    Ok(())
}
