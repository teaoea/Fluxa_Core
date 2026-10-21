//! GeoIP 配置与 Disabled 示例：不访问网络；实际下载使用 Service::refresh 与已配置 HttpsClient。
use fluxa_core::{
    error::Result,
    geoip::{Config, Service},
};
fn main() -> Result<()> {
    let disabled = Service::new(Config::from_yaml("{}")?)?;
    let result = disabled.lookup("8.8.8.8", "zh-CN")?;
    println!(
        "{}",
        serde_json::to_string(&result.status).map_err(|_| fluxa_core::error::Error::Internal)?
    );
    let configured = Config::from_yaml("geoip_url: https://ssss.aa.com")?;
    assert!(configured.geoip_url.is_some());
    // 占位 URL 只校验，不下载。真实调用需有效 MMDB HTTPS 地址及 DNS/路由。
    assert!(Service::new(configured)?.warning().is_none());
    // 已配置但尚无 Ready 数据库时，不能把国家规则静默丢弃或返回兜底成功。
    // 真实来源须先 Service::refresh 成功，再使用同一入口编译。
    let service = Service::new(Config::from_yaml("geoip_url: https://ssss.aa.com")?)?;
    let document = fluxa_core::configuration::Document::from_yaml(
        "geoip: {enabled: true, url: https://ssss.aa.com}\nrules: ['geoip,US,direct', 'final,reject']",
    )?;
    assert!(matches!(
        document.compile_local_rules_with_geoip(1, &service),
        Err(fluxa_core::error::Error::GeoipInitializationFailed)
    ));
    Ok(())
}
