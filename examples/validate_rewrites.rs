//! 只验证新复写声明；不访问共享目录、不执行 JS、不启动监听或解密。
use libfluxa_core::configuration::Document;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = r#"
rules: ['final,reject']

rewrites:
  - url: https://example.com/ads/*
    action: reject
    status: 404
  - url: https://example.com/images/**
    action: modify-response
    response:
      body:
        preset: transparent-png
  - url: https://example.com/api/**
    action: modify-request
    request:
      headers:
        set: {X-Custom-Header: AAA}
        remove: [X-Tracking-ID]
      method: POST
      query:
        set: {source: fluxa}
        remove: [tracking_id]
      body:
        text: '{"enabled":true}'
  - url: https://example.com/api/user
    action: script
    phase: response
    js: response.js
"#;
    let configuration = Document::from_yaml(yaml)?;
    println!(
        "复写字段校验通过；脚本路径/缓存执行仍属后续任务。摘要：{}",
        serde_json::to_string(&configuration.summary())?
    );
    Ok(())
}
