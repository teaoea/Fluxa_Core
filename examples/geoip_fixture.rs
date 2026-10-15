//! 生成实际 Swift FFI 使用的自有小型数据库。
#[path = "../tests/test_fixture_geoip.rs"]
mod fixture;
fn main() {
    let path = std::env::args().nth(1).expect("需要输出路径");
    std::fs::write(path, fixture::fixture()).expect("写入测试数据库");
}
