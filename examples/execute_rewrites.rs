//! 本地去广告响应示例：不触网，不读取脚本，不启动系统代理或 VPN。
use fluxa_core::{https::HttpsClient, outbound::Manager};
use std::sync::Arc;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = Arc::new(Manager::default());
    manager.apply_configuration(
        r#"
rules: ['final,reject']

rewrites:
  - url: http://127.0.0.1:9/ads/**
    enable: true
    phase: request
    when:
      method: GET
      query:
        exists: [ad_id]
    action: respond
    response:
      status: 200
      body:
        preset: minimal-mp4
"#,
    )?;
    let client =
        HttpsClient::with_roots(rustls::RootCertStore::empty())?.with_network(manager.clone());
    let response = client
        .execute_request(
            "GET",
            "http://127.0.0.1:9/ads/video?ad_id=1",
            &[],
            &[],
            None,
        )
        .await?;
    assert_eq!(response.status, 200);
    assert!(response.body.windows(4).any(|b| b == b"ftyp"));
    assert!(
        response
            .headers
            .iter()
            .any(|h| h.name == "Content-Type" && h.value == "video/mp4")
    );
    manager.stop().await?;
    println!(
        "PASS：条件命中后本地返回 0 毫秒无音频 MP4，未建立上游连接；播放器去广告效果仍需客户端验证。"
    );
    Ok(())
}
