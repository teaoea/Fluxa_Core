//! 测试文件：tests/test_logging.rs
//! 测试类型：单元测试、实时输出及背压边界
//! 测试对象：src/logging.rs、src/outbound/configuration.rs
//! 测试内容：levels_keep_exact_existing_semantics、no_consumer_or_disconnection_never_accumulates_history、full_receiver_and_busy_output_never_wait、structured_fields_are_transferred_without_redaction、configuration_failure_keeps_previous_level_and_revision、unix_output_is_live_datagrams_only、encryption_authenticates_and_survives_missing_packets、encryption_bounds_and_exhaustion_never_emit_plaintext、log_schema_defaults_and_strict_public_key_validation、encrypted_unix_transport_round_trip_and_rotation、connection_events_share_id_and_close_once、dns_records_use_existing_encrypted_datagram_protocol。
//! 测试依据：用户要求Core仅生成并实时输出、不脱敏/收集/存储/查询历史，不阻塞代理线程；接收方持有缓冲。
//! 前置条件：本地接收方自建有界队列/Unix数据报，不写文件、不联网或接管VPN。
use super::*;
fn emit(journal: &Journal) {
    journal.emit(Event::ConfigurationCommitted, None, Some(7), None, None);
}
#[test]
fn levels_keep_exact_existing_semantics() {
    for (filter, expected) in [
        (LogFilter::Silent, 0),
        (LogFilter::Info, 3),
        (LogFilter::Debug, 1),
        (LogFilter::Warn, 0),
        (LogFilter::Error, 1),
    ] {
        let journal = Journal::default();
        let (tx, rx) = std::sync::mpsc::sync_channel(8);
        journal.attach(tx).unwrap();
        journal.configure(filter);
        journal.emit(Event::ConfigurationRequested, None, None, None, None);
        emit(&journal);
        journal.emit(
            Event::ConfigurationFailed,
            None,
            None,
            None,
            Some(Error::InvalidInput),
        );
        assert_eq!(rx.try_iter().count(), expected);
    }
    assert!(accepts(filter_code(LogFilter::Warn), Level::Warn));
    assert!(!accepts(filter_code(LogFilter::Debug), Level::Error));
}
#[test]
fn no_consumer_or_disconnection_never_accumulates_history() {
    let journal = Journal::default();
    journal.configure(LogFilter::Info);
    for _ in 0..100 {
        emit(&journal);
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(2);
    journal.attach(tx).unwrap();
    assert!(rx.try_recv().is_err());
    emit(&journal);
    assert_eq!(rx.recv().unwrap().sequence, 101);
    journal.detach().unwrap();
    emit(&journal);
    assert!(rx.try_recv().is_err());
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    drop(rx);
    journal.attach(tx).unwrap();
    for _ in 0..1000 {
        emit(&journal);
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    journal.attach(tx).unwrap();
    assert!(rx.try_recv().is_err());
}
#[test]
fn full_receiver_and_busy_output_never_wait() {
    let journal = std::sync::Arc::new(Journal::default());
    journal.configure(LogFilter::Info);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    journal.attach(tx).unwrap();
    emit(&journal);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let other = journal.clone();
    let worker = std::thread::spawn(move || {
        for _ in 0..1000 {
            emit(&other);
        }
        done_tx.send(()).unwrap();
    });
    done_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("满接收队列不得等待消费者");
    worker.join().unwrap();
    assert_eq!(rx.recv().unwrap().sequence, 1);
    emit(&journal);
    assert_eq!(rx.recv().unwrap().sequence, 1002);
    let held = journal.output.lock().unwrap();
    let other = journal.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        emit(&other);
        tx.send(()).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(2))
        .expect("输出锁竞争不能阻塞事件线程");
    drop(held);
    worker.join().unwrap();
}
#[test]
fn structured_fields_are_transferred_without_redaction() {
    let journal = Journal::default();
    journal.configure(LogFilter::Info);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    journal.attach(tx).unwrap();
    let name = "原始测试节点";
    journal.event(
        Event::NodeFailed,
        None,
        Some(8),
        None,
        Some(Error::ConfigUnknownField(crate::error::FieldName::new(
            "test_field",
        ))),
        Context {
            connection_id: Some(42),
            node: NodeName::new(name),
            ..Context::default()
        },
    );
    let value = serde_json::to_value(rx.recv().unwrap()).unwrap();
    assert_eq!(value["node"], name);
    assert_eq!(value["connection_id"], 42);
    assert_eq!(value["error_info"]["params"]["field"], "test_field");
    assert!(NodeName::new(&"x".repeat(129)).is_none());
}
#[test]
fn configuration_failure_keeps_previous_level_and_revision() {
    let manager = crate::outbound::Manager::default();
    let (tx, rx) = std::sync::mpsc::sync_channel(16);
    manager.set_log_output(tx).unwrap();
    let revision = manager
        .apply_configuration("log: {level: info}\nrules: ['final,direct']")
        .unwrap();
    let record = rx.recv().unwrap();
    assert_eq!(record.event, Event::ConfigurationCommitted);
    assert_eq!(record.revision, Some(revision));
    assert!(
        manager
            .apply_configuration("log: {level: silent}\nrules: ['final,missing']")
            .is_err()
    );
    let records: Vec<_> = rx.try_iter().collect();
    assert!(
        records
            .iter()
            .any(|v| v.event == Event::ConfigurationFailed)
    );
    assert_eq!(manager.configuration_status().unwrap().revision, revision);
    manager.set_log_level(LogFilter::Error).unwrap();
    manager.configure_limits(Default::default()).unwrap();
    assert!(
        manager
            .apply_configuration("rules: ['final,missing']")
            .is_err()
    );
    assert_eq!(rx.recv().unwrap().level, Level::Error);
    assert!(rx.try_recv().is_err());
}
#[cfg(unix)]
#[test]
fn unix_output_is_live_datagrams_only() {
    let journal = Journal::default();
    journal.configure(LogFilter::Info);
    let (sender, receiver) = UnixDatagram::pair().unwrap();
    receiver
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    journal.attach_unix(sender).unwrap();
    emit(&journal);
    let mut bytes = [0u8; MAX_EVENT_BYTES];
    let size = receiver.recv(&mut bytes).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes[..size]).unwrap();
    assert_eq!(value["event"], "configuration-committed");
    // 真实数据报背压：保留但不读取接收端，发送缓冲耗尽后也必须立即返回。
    for _ in 0..10000 {
        emit(&journal);
    }
    journal.detach().unwrap();
    let (sender, receiver) = UnixDatagram::pair().unwrap();
    journal.attach_unix(sender).unwrap();
    drop(receiver);
    for _ in 0..1000 {
        emit(&journal);
    } // 对端关闭不重试、不保存当前事件。
}

// 接收方独立按公开线协议恢复exporter密钥；不调用生产seal的逆函数。
fn decrypt_packet(
    secret: &<hpke::kem::DhP256HkdfSha256 as hpke::Kem>::PrivateKey,
    packet: &[u8],
) -> std::result::Result<Vec<u8>, ()> {
    use hpke::Deserializable;
    type Kem = hpke::kem::DhP256HkdfSha256;
    if packet.len() < 101 || &packet[..8] != b"NCLOG001" {
        return Err(());
    }
    let enc = <Kem as hpke::Kem>::EncappedKey::from_bytes(&packet[8..73]).map_err(|_| ())?;
    let ctx = hpke::setup_receiver::<hpke::aead::AesGcm256, hpke::kdf::HkdfSha256, Kem>(
        &hpke::OpModeR::Base,
        secret,
        &enc,
        b"Nodus Core log transport v1",
    )
    .map_err(|_| ())?;
    let mut raw = zeroize::Zeroizing::new([0; 32]);
    ctx.export(b"Nodus Core log AES-256-GCM record key v1", raw.as_mut())
        .map_err(|_| ())?;
    let key = ring::aead::LessSafeKey::new(
        ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, raw.as_ref()).map_err(|_| ())?,
    );
    let mut body = packet[85..].to_vec();
    let plain = key
        .open_in_place(
            ring::aead::Nonce::try_assume_unique_for_key(&packet[73..85]).map_err(|_| ())?,
            ring::aead::Aad::from(&packet[..85]),
            &mut body,
        )
        .map_err(|_| ())?;
    Ok(plain.to_vec())
}
fn test_key() -> (
    <hpke::kem::DhP256HkdfSha256 as hpke::Kem>::PrivateKey,
    String,
) {
    use base64::Engine;
    use hpke::{Kem, Serializable};
    let mut rng = LogRng { failed: false };
    let (secret, public) = hpke::kem::DhP256HkdfSha256::gen_keypair(&mut rng);
    assert!(!rng.failed);
    (
        secret,
        base64::engine::general_purpose::STANDARD.encode(public.to_bytes()),
    )
}
#[test]
fn encryption_authenticates_and_survives_missing_packets() {
    let (secret, public) = test_key();
    let cipher = Cipher::new(&public).unwrap();
    let mut packets = Vec::new();
    for n in 1..=3 {
        let mut p = [0u8; 512];
        let size = cipher
            .seal(format!("record-{n}").as_bytes(), &mut p)
            .unwrap();
        packets.push(p[..size].to_vec());
    }
    // 消费者丢失第二包并倒序解密仍成功，无隐式HPKE序号漂移。
    assert_eq!(decrypt_packet(&secret, &packets[2]).unwrap(), b"record-3");
    assert_eq!(decrypt_packet(&secret, &packets[0]).unwrap(), b"record-1");
    assert_ne!(&packets[0][73..85], &packets[1][73..85]);
    let (wrong, _) = test_key();
    assert!(decrypt_packet(&wrong, &packets[0]).is_err());
    for index in [0, 12, 74, 86, packets[0].len() - 1] {
        let mut p = packets[0].clone();
        p[index] ^= 1;
        assert!(decrypt_packet(&secret, &p).is_err());
    }
    assert!(decrypt_packet(&secret, &packets[0][..90]).is_err());
    let rotated = Cipher::new(&public).unwrap();
    let mut p = [0u8; 512];
    let size = rotated.seal(b"rotated", &mut p).unwrap();
    assert_ne!(&packets[0][8..73], &p[8..73]);
    assert_eq!(decrypt_packet(&secret, &p[..size]).unwrap(), b"rotated");
}
#[test]
fn encryption_bounds_and_exhaustion_never_emit_plaintext() {
    let (_, public) = test_key();
    let cipher = Cipher::new(&public).unwrap();
    assert!(
        cipher
            .seal(&[0; MAX_EVENT_BYTES + 1], &mut [0; MAX_EVENT_BYTES + 128])
            .is_err()
    );
    assert!(cipher.seal(b"data", &mut [0; 10]).is_err());
    cipher.next_nonce.store(u64::MAX, Ordering::Relaxed);
    assert!(cipher.seal(b"data", &mut [0; 512]).is_err());
    let journal = Journal::default();
    journal.configure(LogFilter::Info);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    journal.attach(tx).unwrap();
    let mut settings = crate::configuration::models::Log {
        encryption: true,
        public_key: public,
        level: LogFilter::Info,
        ..Default::default()
    };
    settings.validate().unwrap();
    journal.apply_settings(&settings);
    emit(&journal);
    assert!(rx.try_recv().is_err(), "加密启用时禁止类型化明文旁路");
    settings.encryption = false;
    settings.validate().unwrap();
    journal.apply_settings(&settings);
    emit(&journal);
    assert!(rx.try_recv().is_ok());
}
#[test]
fn log_schema_defaults_and_strict_public_key_validation() {
    let document = crate::configuration::Document::from_yaml("rules: ['final,direct']").unwrap();
    assert!(!document.log_settings().encryption);
    assert!(document.log_settings().public_key.is_empty());
    assert_eq!(document.log_filter(), LogFilter::Silent);
    for log in [
        "{enabled: true}",
        "{encryption: true}",
        "{encryption: true, public-key: not-a-key}",
        "{encryption: true, public-key: BASE64_PUBLIC_KEY}",
    ] {
        assert!(
            crate::configuration::Document::from_yaml(&format!(
                "log: {log}\nrules: ['final,direct']"
            ))
            .is_err()
        );
    }
    assert!(
        crate::configuration::Document::from_yaml("log-level: info\nrules: ['final,direct']")
            .is_err()
    );
    let (_, public) = test_key();
    let document = crate::configuration::Document::from_yaml(&format!(
        "log: {{encryption: true, public-key: '{public}', level: info}}\nrules: ['final,direct']"
    ))
    .unwrap();
    assert!(document.log_settings().cipher.is_some());
}
#[cfg(unix)]
#[test]
fn encrypted_unix_transport_round_trip_and_rotation() {
    let (secret, public) = test_key();
    let journal = Journal::default();
    journal.configure(LogFilter::Info);
    let (tx, rx) = UnixDatagram::pair().unwrap();
    rx.set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    journal.attach_unix(tx).unwrap();
    let mut settings = crate::configuration::models::Log {
        encryption: true,
        public_key: public,
        level: LogFilter::Info,
        ..Default::default()
    };
    settings.validate().unwrap();
    journal.apply_settings(&settings);
    emit(&journal);
    let mut p = [0u8; MAX_EVENT_BYTES + 128];
    let size = rx.recv(&mut p).unwrap();
    assert!(serde_json::from_slice::<serde_json::Value>(&p[..size]).is_err());
    let decoded: serde_json::Value =
        serde_json::from_slice(&decrypt_packet(&secret, &p[..size]).unwrap()).unwrap();
    assert_eq!(decoded["event"], "configuration-committed");
    let (next_secret, next_public) = test_key();
    settings.public_key = next_public;
    settings.validate().unwrap();
    journal.apply_settings(&settings);
    emit(&journal);
    let size = rx.recv(&mut p).unwrap();
    assert!(decrypt_packet(&secret, &p[..size]).is_err());
    assert!(decrypt_packet(&next_secret, &p[..size]).is_ok());
}

#[tokio::test]
async fn connection_events_share_id_and_close_once() {
    use crate::outbound::{Manager, RoutedTarget};
    use crate::routing::{Target, Transport};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let manager = Manager::default();
    let (tx, rx) = std::sync::mpsc::sync_channel(64);
    manager.set_log_output(tx).unwrap();
    manager
        .apply_configuration("log: {level: info}\nrules: ['final,direct']")
        .unwrap();
    let request = || RoutedTarget {
        target: Target {
            domain: None,
            address: Some("127.0.0.1".parse().unwrap()),
            port: listener.local_addr().unwrap().port(),
            transport: Transport::Tcp,
        },
        timeout_ms: 1000,
        family: crate::outbound::Family::Ipv4,
        mapping_generation: None,
    };
    let opened = manager.connect_configured(request()).await.unwrap();
    let (_peer, _) = listener.accept().await.unwrap();
    manager
        .write(opened.connection.id, b"hello", 1000)
        .await
        .unwrap();
    manager.close(opened.connection.id).await.unwrap();
    assert!(manager.close(opened.connection.id).await.is_err());
    let rows: Vec<_> = rx
        .try_iter()
        .filter(|v| v.context.connection_id == Some(opened.connection.connection_id))
        .collect();
    for event in [
        Event::ConnectionCreated,
        Event::RouteMatched,
        Event::RouteSelected,
        Event::ConnectionEstablished,
        Event::ConnectionClosed,
    ] {
        assert!(rows.iter().any(|v| v.event == event), "missing {event:?}");
    }
    assert_eq!(
        rows.iter()
            .filter(|v| v.event == Event::ConnectionClosed)
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .find(|v| v.event == Event::ConnectionClosed)
            .unwrap()
            .context
            .uploaded,
        Some(5)
    );
    let second = manager.connect_configured(request()).await.unwrap();
    assert_ne!(
        second.connection.connection_id,
        opened.connection.connection_id
    );
    manager.close(second.connection.id).await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn dns_records_use_existing_encrypted_datagram_protocol() {
    let (secret, public) = test_key();
    let manager = crate::outbound::Manager::default();
    let (tx, rx) = UnixDatagram::pair().unwrap();
    rx.set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    manager.logs.attach_unix(tx).unwrap();
    manager.apply_configuration(&format!("log: {{encryption: true, public-key: '{public}', level: info}}\nrules: ['final,direct']")).unwrap();
    let mut packet = [0u8; MAX_EVENT_BYTES + 128];
    rx.recv(&mut packet).unwrap();
    let settings: crate::configuration::models::Dns = crate::routing::runtime::yaml::decode(
        "{enable: true, server: ['127.0.0.1:9'], hosts: {hosts.secret.test: 192.0.2.8}}",
    )
    .unwrap();
    let resolver = crate::dns::Resolver::from_main(
        &settings,
        Default::default(),
        crate::https::HttpsClient::system().unwrap().tls_config(),
        None,
    )
    .await
    .unwrap()
    .unwrap();
    manager
        .resolve_dns_query(
            &resolver,
            crate::dns::Query {
                name: "hosts.secret.test".into(),
                query_type: "A".into(),
            },
            true,
        )
        .await
        .unwrap();
    let mut id = None;
    let mut events = Vec::new();
    for _ in 0..3 {
        let size = rx.recv(&mut packet).unwrap();
        assert!(
            !packet[..size]
                .windows(b"hosts.secret.test".len())
                .any(|v| v == b"hosts.secret.test")
        );
        let value: serde_json::Value =
            serde_json::from_slice(&decrypt_packet(&secret, &packet[..size]).unwrap()).unwrap();
        assert_eq!(value["module"], "dns");
        assert_eq!(value["dns"]["name"], "hosts.secret.test");
        if let Some(id) = id {
            assert_eq!(value["dns"]["query_id"], id);
        } else {
            id = Some(value["dns"]["query_id"].as_u64().unwrap());
        }
        events.push(value["event"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        events,
        ["dns-query-started", "dns-hosts-hit", "dns-query-completed"]
    );
    resolver.close().await.unwrap();
}
