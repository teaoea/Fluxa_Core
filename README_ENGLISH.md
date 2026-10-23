# Nodus Core

[中文](README.md)

Nodus Core is a Rust networking engine for embedding in proxy, VPN, and network debugging tools. It provides configuration validation, traffic rules, name resolution, managed connections, and traffic rewriting through shared networking components.

Use it as a library through its Rust API or C ABI. The integrating program supplies configuration files and resources such as plugins, scripts, rules, and GeoIP databases; Core reads, validates, and executes them without managing their storage.

## Features

- **Configuration validation:** check YAML syntax, fields, duplicate keys, and references; receive error codes, field paths, and source locations.
- **Rules and routing:** select DIRECT, PROXY, or REJECT using local, plugin, and remote rules, including domain, IP, port, and protocol conditions.
- **DNS and GeoIP:** DNS resolution, Fake-IP mapping, and offline IP geolocation.
- **Connections and protocols:** managed connection lifecycles and cancellation, HTTP/HTTPS requests, and implemented VLESS, REALITY, Vision, and XUDP combinations.
- **Traffic rewriting:** HTTPS MITM, conditional rewrites, declarative plugins, and bounded JavaScript execution.
- **Packet processing:** bounded IPv4/IPv6 and TCP/UDP packet sessions for programs that need a packet data plane.

System VPN permissions, interfaces, and route takeover require integration with the relevant platform facilities. Linking the library does not establish a system VPN automatically.

## Usage

### From Rust

Add a Git dependency to the integrating project's `Cargo.toml`:

```toml
[dependencies]
nodus_core = { package = "Nodus-Core", git = "https://github.com/teaoea/Nodus-Core", branch = "main" }
```

Validate a configuration that uses direct connections by default, and inspect the field and source location if it fails:

```rust
use nodus_core::configuration::check_yaml;

fn main() {
    let yaml = "rules:\n  - final,direct\n";
    match check_yaml(yaml) {
        Ok(summary) => println!("Valid configuration; {} rules", summary.rule_count),
        Err(issue) => {
            eprintln!("Error {} in {}", issue.error_info.code.value(), issue.field);
            if let Some(position) = &issue.position {
                eprintln!("Line {}, column {}", position.line, position.column);
            }
        }
    }
}
```

A successful check validates the configuration without applying it or starting networking. Start with the [English default template](default_config_en.yaml) or [Chinese default template](default_config.yaml).

### Through the C ABI

Link the native static library for the target platform and its matching [public header](include/nodus_core.h). Use the declared interfaces to create handles, invoke operations, copy results, and release resources. See the [configuration-check C ABI](Documentation/API/ffi_api.md) for request and response examples.

### Run the configuration-check example

```sh
git clone https://github.com/teaoea/Nodus-Core.git
cd Nodus-Core
cargo run --locked --example validate_main_configuration -- default_config_en.yaml
```

The example reads the specified YAML file and prints a validation summary. Invalid input produces an error; the example does not start a listener or VPN.

## Documentation

- [API documentation](Documentation/API/README.md)
- [Configuration-check Rust API](Documentation/API/configuration/check_api.md)
- [Error codes and parameters](Documentation/API/error_api.md)

## License

Author: 可莉唯一的狗. Core source is licensed under [MIT](LICENSE). See [LICENSE.md](LICENSE.md) for third-party dependency notices.
