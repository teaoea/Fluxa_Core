//! GeoIP 配置与 Disabled 示例：不访问网络；实际下载使用 Service::refresh 与已配置 HttpsClient。
use libfluxa_core::{
    error::Result,
    geoip::{Config, Service},
};
fn main() -> Result<()> {
    let disabled = Service::new(Config::from_yaml("{}")?)?;
    let result = disabled.lookup("8.8.8.8", "zh-CN")?;
    println!(
        "{}",
        serde_json::to_string(&result.status).map_err(|_| libfluxa_core::error::Error::Internal)?
    );
    let configured = Config::from_yaml("geoip_url: https://ssss.aa.com")?;
    assert!(configured.geoip_url.is_some());
    // 占位 URL 只校验，不下载。真实调用需有效 MMDB HTTPS 地址及 DNS/路由。
    assert!(Service::new(configured)?.warning().is_none());
    Ok(())
}
