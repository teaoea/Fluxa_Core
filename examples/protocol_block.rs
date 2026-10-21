//! cargo run --example protocol_block：只编译配置与识别测试报文，不连接真实目标。
use fluxa_core::routing::{Config, Match, Policy, Router, Rule};
fn main() -> Result<(), fluxa_core::error::Error> {
    let router = Router::compile(Config {
        fallback: Policy::Direct,
        rules: vec![Rule {
            protocol: None,
            r#type: Match::Transport,
            value: "quic".into(),
            policy: Policy::Reject,
            outbound: None,
        }],
        ..Default::default()
    })?;
    let mut packet = vec![0xc0, 0, 0, 0, 1, 1, 1, 1, 2, 0, 17];
    packet.extend([0; 17]);
    let mut flow = fluxa_core::protocol::quic::Flow::default();
    println!(
        "配置启用 QUIC 拦截：{}；测试首包识别：{}",
        router.blocks_quic(),
        flow.observe(&packet)
    );
    Ok(())
}
