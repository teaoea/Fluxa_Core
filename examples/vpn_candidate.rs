//! 仅准备本地规则候选并读取摘要；不下载提供者、不修改缓存或系统网络。
use fluxa_core::vpn::{Phase, Preparation};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let candidate =
        Preparation::begin("tun: {enable: true, name: fluxa0}\nrules: ['final,direct']")?;
    let result = async {
        loop {
            let status = candidate.status()?;
            if status.phase != Phase::Preparing {
                println!("{}", serde_json::to_string(&status)?);
                if status.phase != Phase::CandidateReady {
                    return Err("候选未准备成功".into());
                }
                if status.activation_permitted {
                    return Err("候选不能取得系统接管权限".into());
                }
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }
    .await;
    candidate.close().await?;
    result
}
