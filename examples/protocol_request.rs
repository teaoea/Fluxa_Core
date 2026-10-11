//! 真实节点 HTTPS 探针：凭据从有界临时文件读取，仅报告状态与长度。
use libfluxa_core::{
    dns,
    error::{Error, Result},
    https::HttpsClient,
    outbound, protocol, routing,
};
use std::{io::Read, sync::Arc};
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err(Error::InvalidInput);
    }
    let mut input = Vec::new();
    std::fs::File::open(&args[1])?
        .take(8193)
        .read_to_end(&mut input)?;
    if input.len() > 8192 {
        return Err(Error::ResourceLimit);
    }
    let profile: protocol::config::Profile =
        serde_json::from_slice(&input).map_err(|_| Error::InvalidInput)?;
    let node = profile.validate()?;
    let client = HttpsClient::system()?;
    let resolver = Arc::new(dns::Resolver::new(dns::Config {
        servers: vec![dns::Server::Doh {
            url: "https://cloudflare-dns.com/dns-query".into(),
            bootstrap: "1.1.1.1:443".into(),
        }],
        timeout_ms: 5000,
        dnssec: false,
        fake_ip: None,
        service: None,
    })?);
    let router = Arc::new(routing::Router::compile(routing::Config {
        mode: routing::Mode::Global,
        outbound: Some(profile.id.clone()),
        ..Default::default()
    })?);
    client.network().configure_network_with_outbounds(
        Some(resolver),
        Some(router),
        outbound::Limits::default(),
        vec![profile],
    )?;
    let response = client.execute("GET", &args[2], &[], &[]).await?;
    println!(
        "指纹={}，HTTPS 状态={}，响应正文={} 字节",
        node.fingerprint.version(),
        response.status,
        response.body.len()
    );
    Ok(())
}
