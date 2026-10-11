# Fluxa Network Core

Author: 可莉唯一的狗.

[中文](README.md)

`libfluxa_core` is a cross-platform network core written in Rust. Network operations share connection management, bounded resources, and explicit failure handling.

## Capabilities

- Native VLESS + TCP + REALITY + Vision outbound, fixed Chrome/Firefox/Safari/iOS/Android Chrome/Edge ClientHello templates, and X25519MLKEM768 for modern Chrome. See [Protocols.md](Protocols.md) for verified versions and unsupported handshake behavior.

- HTTP request forwarding, CONNECT tunnels, and HTTP/1.1 framing.
- HTTPS with TLS 1.2/1.3 and upstream certificate verification.
- MITM for explicitly selected domains, certificate generation, and bounded request/response capture.
- Header and response rewriting, bounded JavaScript execution, and isolated JSON transformations.
- UDP/TCP DNS, DoT/DoH, configurable upstreams, positive/negative caching, DNSSEC, Fake-IP mappings, and access-controlled LAN DNS service.
- Runtime rule generation, normalization, deduplication, proven full-coverage pruning, and indexed matching. Domain targets use domain rules first; a domain match skips IP rules. Missing or unsupported proxy outbounds fail explicitly.
- Shared TCP/UDP outbounds, cancellation, half-close, idle expiry, and atomic configuration snapshots.
- Daily remote subscription schedules in UTC and bounded HTTPS subscription downloads. A missing schedule means manual updates only. On resume, only the most recent missed occurrence is handled; missed days are not replayed as separate jobs.
- TUN packet inspection and checksum validation. Packet forwarding is not implemented yet.

## Configuration examples

The complete host configuration examples are in the repository root:

- [fluxa.yaml](fluxa.yaml): Chinese display names and Chinese comments.
- [fluxa_english.yaml](fluxa_english.yaml): English display names and Chinese comments.

Use `update-time: "12:00"` for a daily update at 12:00 UTC. Omit the field to disable automatic updates. Individual updates and Update All are manual actions. The removed `update-interval-hours` field is rejected. Failed attempts retain the previous content and retry with a minimum 60-second backoff; a successful catch-up is not repeated. Schedules require a running host and network access; they do not promise operating-system wakeup while the host is closed or suspended.

Configuration fields and both complete examples are updated together. See [Subscriptions.md](Subscriptions.md), [RuntimeRules.md](RuntimeRules.md), [DNS.md](DNS.md), and [Outbound.md](Outbound.md) for semantics and current limitations.

## Roadmap and license

TUN forwarding, gateway operation, and additional proxy protocols remain in development. VLESS + TCP + REALITY + Vision is implemented for TCP outbounds; UDP/XUDP/Mux and TUN forwarding remain unsupported.

Original code is licensed under MIT; see [LICESE.md](LICESE.md). Third-party dependencies retain their upstream licenses.

Rule subscriptions use Clash classical `payload` with uppercase DOMAIN, DOMAIN-SUFFIX, DOMAIN-KEYWORD and IP-CIDR (IPv4/IPv6). Subscription policy/outbound selects the action. Effective merged rules are persisted in SQLite; matching uses immutable memory indexes. See [rules.yaml](rules.yaml).

Offline GeoIP uses bounded MMDB loading and IPv4/IPv6 lookup, with fixed free public-IP echo endpoints and TCP connection timing. Databases are supplied separately by the host. Failures are explicit; these features do not imply system TUN support. See GeoIP.md for resource limits, privacy and licensing.
