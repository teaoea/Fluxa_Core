//! 接收宿主明确指定的现有配置和HTTPS目标，只运行前置检查；无默认外部网址。
//! 报告包含平台子检查诊断；缺真实平台证据时仍错误退出，不接管VPN。
use nodus_core::{
    error::Error,
    storage::{self, Resources},
    vpn::transaction::{Controller, Phase},
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let file = args.next().ok_or(Error::InvalidInput)?;
    let probes: Vec<_> = args.collect();
    let input = storage::read_configuration(&file)?;
    let controller = Controller::default();
    let id = controller.begin_preflight(
        input.as_str(),
        Resources::default(),
        &probes,
        std::time::Duration::from_secs(30),
    )?;
    loop {
        let status = controller.status(id)?;
        if matches!(
            status.phase,
            Phase::Failed | Phase::RollbackFailed | Phase::Stopped
        ) {
            let report = controller
                .preflight_status(id)?
                .ok_or(Error::InvalidState)?;
            println!("{}", serde_json::to_string(&report)?);
            if let Some(error) = status.cleanup_failure.or(status.failure) {
                return Err(error.into());
            }
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
