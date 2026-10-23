//! 只读查询四阶段驱动；未注册生产步骤时没有活动事务，不启动网络。
use nodus_core::vpn::transaction::Controller;
fn main() -> Result<(), nodus_core::error::Error> {
    let controller = Controller::default();
    match controller.current_status()? {
        Some(status) => println!(
            "事务 {}：{:?}，系统状态 {:?}",
            status.transaction, status.phase, status.system
        ),
        None => println!("没有活动的四阶段事务；当前不代表系统 VPN 已启用。"),
    }
    Ok(())
}
