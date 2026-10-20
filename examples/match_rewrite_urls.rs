//! 只演示已编译 URL 匹配，不执行复写动作、不访问网络或脚本文件。
use libfluxa_core::rewrite::UrlPattern;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let one = UrlPattern::compile("https://example.com/api/*")?;
    let many = UrlPattern::compile("https://example.com/api/**")?;
    for (target, expected_one, expected_many) in [
        ("https://example.com/api/a", true, true),
        ("https://example.com/api/aa/a", false, true),
        ("https://example.com/api", false, true),
        ("https://example.com/api/", false, true),
        ("https://example.com/API/a", false, false),
        ("https://example.com/api/a?source=fluxa", true, true),
    ] {
        let actual_one = one.matches(target)?;
        let actual_many = many.matches(target)?;
        assert_eq!((actual_one, actual_many), (expected_one, expected_many));
        println!("{target}：单段={actual_one}，多段={actual_many}");
    }
    Ok(())
}
