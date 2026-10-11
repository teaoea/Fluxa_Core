//! 独立协议互通探针；测试凭据仅从指定临时文件读取，不输出凭据。
use libfluxa_core::{
    error::{Error, Result},
    protocol,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err(Error::InvalidInput);
    }
    let input = std::fs::read(&args[1])?;
    if input.len() > 8192 {
        return Err(Error::ResourceLimit);
    }
    let profile: protocol::config::Profile =
        serde_json::from_slice(&input).map_err(|_| Error::InvalidInput)?;
    let node = profile.validate()?;
    let port: u16 = args[2].parse().map_err(|_| Error::InvalidInput)?;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let socket = tokio::net::TcpStream::connect((node.host.as_str(), node.port)).await?;
        let mut stream = protocol::connect(socket, &node, "127.0.0.1", port).await?;
        let data: Vec<u8> = (0..65536).map(|n| (n % 251) as u8).collect();
        stream.write_all(&data).await?;
        let mut received = vec![0; data.len()];
        stream.read_exact(&mut received).await?;
        if received != data {
            return Err(Error::InvalidState);
        }
        stream.shutdown().await?;
        println!("REALITY 认证、VLESS TCP 与 Vision 64 KiB 回环转发通过");
        Ok(())
    })
    .await
    .map_err(|_| Error::NetworkTimeout)?
}
