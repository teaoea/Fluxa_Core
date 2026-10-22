//! 测试文件：tests/test_fixture_geoip.rs
//! 测试类型：单元/集成测试共享夹具，非独立测试目标
//! 测试对象：共享协议夹具
//! 测试内容：夹具支持函数，不计独立断言覆盖
//!
//! 测试依据：《测试文件与报告统一管理原则》及原文件所列公开契约；不反推业务预期。
//!
//! 前置条件：Swift 目标需 macOS/固定二进制或源码静态库；夹具需本地测试权限，详见实际执行命令。
//! 自有最小 MMDB 测试生成器；仅供测试，不包含第三方地理数据。
use std::net::IpAddr;
fn text(value: &str) -> Vec<u8> {
    assert!(value.len() < 29);
    let mut bytes = vec![0x40 | value.len() as u8];
    bytes.extend_from_slice(value.as_bytes());
    bytes
}
fn map(values: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut bytes = vec![0xe0 | values.len() as u8];
    for (key, value) in values {
        bytes.extend(text(key));
        bytes.extend(value);
    }
    bytes
}
fn integer(value: u32, kind: u8) -> Vec<u8> {
    let raw = value.to_be_bytes();
    let start = raw.iter().position(|value| *value != 0).unwrap_or(4);
    let mut bytes = vec![(kind << 5) | (4 - start) as u8];
    bytes.extend_from_slice(&raw[start..]);
    bytes
}
fn location(country: &str, city: &str, region: &str) -> Vec<u8> {
    map(vec![
        // MMDB 标准允许 uint128；附加字段合法，校验器不能误报损坏。
        ("test_u128", {
            let mut value = vec![0x10, 3];
            value.extend([0xff; 16]);
            value
        }),
        ("country", map(vec![("iso_code", text(country))])),
        ("city", map(vec![("names", map(vec![("en", text(city))]))])),
        ("subdivisions", {
            let mut values = vec![1, 4]; // 扩展类型 11：单元素数组。
            values.extend(map(vec![("names", map(vec![("en", text(region))]))]));
            values
        }),
    ])
}
pub fn fixture() -> Vec<u8> {
    let entries = [
        ("8.8.8.8", location("US", "Los Angeles", "California")),
        ("2001:4860:4860::8888", location("TW", "Taipei", "Taiwan")),
    ];
    let mut nodes = vec![[None::<usize>; 2]];
    let mut leaves = Vec::new();
    for (address, record) in entries {
        let number = match address.parse::<IpAddr>().unwrap() {
            IpAddr::V4(ip) => u128::from(u32::from(ip)),
            IpAddr::V6(ip) => u128::from(ip),
        };
        let mut node = 0;
        for bit in (0..128).rev() {
            let branch = ((number >> bit) & 1) as usize;
            if bit == 0 {
                leaves.push((node, branch, record.clone()));
                break;
            }
            node = if let Some(next) = nodes[node][branch] {
                next
            } else {
                let next = nodes.len();
                nodes.push([None; 2]);
                nodes[node][branch] = Some(next);
                next
            };
        }
    }
    let count = nodes.len();
    let mut pointers: Vec<[usize; 2]> = nodes
        .iter()
        .map(|node| [node[0].unwrap_or(count), node[1].unwrap_or(count)])
        .collect();
    let mut data = Vec::new();
    for (node, branch, record) in leaves {
        pointers[node][branch] = count + 16 + data.len();
        data.extend(record);
    }
    let mut output = Vec::new();
    for node in pointers {
        for pointer in node {
            output.extend_from_slice(&(pointer as u32).to_be_bytes()[1..]);
        }
    }
    output.extend([0; 16]);
    output.extend(data);
    output.extend_from_slice(b"\xab\xcd\xefMaxMind.com");
    output.extend(map(vec![
        ("node_count", integer(count as u32, 6)),
        ("record_size", integer(24, 5)),
        ("ip_version", integer(6, 5)),
        ("database_type", text("Nodus-Test-City")),
        ("binary_format_major_version", integer(2, 5)),
        ("binary_format_minor_version", integer(0, 5)),
        ("build_epoch", vec![1, 2, 1]), // 扩展类型 9：uint64。
        ("languages", {
            let mut list = vec![1, 4];
            list.extend(text("en"));
            list
        }),
        (
            "description",
            map(vec![("en", text("Nodus owned test fixture"))]),
        ),
    ]));
    output
}
