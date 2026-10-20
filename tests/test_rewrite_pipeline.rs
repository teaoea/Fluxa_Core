//! 测试文件：tests/test_rewrite_pipeline.rs
//! 测试类型：集成测试、原生复写与真实 HTTP 数据路径回归。
//! 测试对象：src/configuration.rs、src/configuration/rewrites/、src/https.rs、src/outbound/stream.rs、src/routing.rs。
//! 测试内容：local_actions_never_dial、request_and_response_use_real_wire_data、response_rules_wait_for_upstream、direct_overrides_formal_reject、named_proxy_dials_only_selected_node、stopped_manager_cannot_construct_responses、https_rewrites_after_verified_tls。
//! 测试依据：用户 2026-10-20 示例注释、原始响应条件、proxy 忽略与 proxy(节点名称) 指定节点要求。
//! 前置条件：仅临时回环端口；不接管系统 VPN、不修改代理和系统证书、不访问外网。
#[path = "test_fixture_https.rs"]
mod support;
use libfluxa_core::{
    error::Error,
    http::{self, Header},
    https::HttpsClient,
    outbound::Manager,
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::Notify,
    time::{sleep, timeout},
};
struct Server {
    port: u16,
    count: Arc<AtomicUsize>,
    release: Arc<Notify>,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn start(delayed: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let count = Arc::new(AtomicUsize::new(0));
        let observed = count.clone();
        let release = Arc::new(Notify::new());
        let gate = release.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    break;
                };
                observed.fetch_add(1, Ordering::SeqCst);
                let mut socket = BufReader::new(socket);
                let Ok(head) = http::read_head(&mut socket).await else {
                    continue;
                };
                let Ok(body) = http::read_body(&mut socket, &head, false).await else {
                    continue;
                };
                if delayed {
                    gate.notified().await;
                }
                let payload = if head.first.contains("/echo") {
                    serde_json::to_vec(&json!({"first":head.first,"headers":head.headers,"body":String::from_utf8(body).unwrap()})).unwrap()
                } else {
                    b"original".to_vec()
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nX-Resource-Type: image\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                let _ = socket.get_mut().write_all(response.as_bytes()).await;
                let _ = socket.get_mut().write_all(&payload).await;
                let _ = socket.get_mut().shutdown().await;
            }
        });
        Self {
            port,
            count,
            release,
            task,
        }
    }
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn make_client(config: Value) -> (HttpsClient, Arc<Manager>) {
    let manager = Arc::new(Manager::default());
    manager.apply_configuration(&config.to_string()).unwrap();
    (
        HttpsClient::with_roots(rustls::RootCertStore::empty())
            .unwrap()
            .with_network(manager.clone()),
        manager,
    )
}
fn config(rules: Value) -> Value {
    json!({"rules":["final,reject"],"rewrites":rules})
}
#[tokio::test]
async fn local_actions_never_dial() {
    let server = Server::start(false).await;
    let url = server.url("/ads");
    for action in [
        json!({"action":"reject","status":404}),
        json!({"action":"respond","response":{"status":200,"body":{"text":"local"}}}),
        json!({"action":"redirect","status":307,"location":"https://example.org/new"}),
        json!({"action":"abort"}),
    ] {
        let mut rule = action.clone();
        rule["url"] = json!(url);
        rule["when"] = json!({"method":"GET","query":{"exists":["ad"]}});
        let (client, manager) = make_client(config(json!([rule])));
        let reply = client
            .execute_request("GET", &format!("{url}?ad=1"), &[], &[], None)
            .await;
        match action["action"].as_str().unwrap() {
            "reject" => assert_eq!(reply.unwrap().status, 404),
            "respond" => assert_eq!(reply.unwrap().body, b"local"),
            "redirect" => assert_eq!(reply.unwrap().status, 307),
            "abort" => assert!(matches!(reply, Err(Error::Cancelled))),
            _ => unreachable!(),
        }
        assert!(matches!(
            client
                .execute_request("POST", &format!("{url}?ad=1"), &[], &[], None)
                .await,
            Err(Error::Rejected)
        ));
        manager.stop().await.unwrap();
    }
    assert_eq!(server.count.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn request_and_response_use_real_wire_data() {
    let server = Server::start(false).await;
    let url = server.url("/echo");
    let mut cfg = config(
        json!([{"url":url,"action":"modify-request","request":{"method":"POST","headers":{"set":{"X-New":"yes"},"remove":["X-Old"]},"query":{"set":{"source":"fluxa"},"remove":["ad"]},"body":{"text":"changed"}}}]),
    );
    cfg["rules"] = json!(["final,direct"]);
    let (client, manager) = make_client(cfg);
    let reply = client
        .execute_request(
            "GET",
            &format!("{url}?ad=1"),
            &[Header {
                name: "X-Old".into(),
                value: "x".into(),
            }],
            &[],
            None,
        )
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&reply.body).unwrap();
    assert_eq!(payload["first"], "POST /echo?source=fluxa HTTP/1.1");
    assert_eq!(payload["body"], "changed");
    assert!(
        payload["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["name"] == "X-New" && h["value"] == "yes")
    );
    assert!(
        !payload["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["name"] == "X-Old")
    );
    manager.stop().await.unwrap();
    let url = server.url("/image");
    let mut cfg = config(
        json!([{"url":url,"phase":"response","when":{"status":200,"response-headers":{"equals":{"X-Resource-Type":"image"}},"content-type":"image/png","content-length":{"min":8,"max":8}},"action":"modify-response","response":{"status":201,"body":{"preset":"transparent-png"}}}]),
    );
    cfg["rules"] = json!(["final,direct"]);
    let (client, manager) = make_client(cfg);
    let reply = client
        .execute_request("GET", &url, &[], &[], None)
        .await
        .unwrap();
    assert_eq!(reply.status, 201);
    assert!(reply.body.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(server.count.load(Ordering::SeqCst), 2);
    manager.stop().await.unwrap();
}
#[tokio::test]
async fn response_rules_wait_for_upstream() {
    let server = Server::start(true).await;
    let url = server.url("/image");
    let mut cfg = config(
        json!([{"url":url,"phase":"response","when":{"status":200},"action":"modify-response","response":{"body":{"text":"after"}}}]),
    );
    cfg["rules"] = json!(["final,direct"]);
    let (client, manager) = make_client(cfg);
    let request =
        tokio::spawn(async move { client.execute_request("GET", &url, &[], &[], None).await });
    timeout(Duration::from_secs(2), async {
        while server.count.load(Ordering::SeqCst) == 0 {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(!request.is_finished());
    server.release.notify_one();
    let result = timeout(Duration::from_secs(2), request)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(result.body, b"after");
    manager.stop().await.unwrap();
}
#[tokio::test]
async fn direct_overrides_formal_reject() {
    let server = Server::start(false).await;
    let url = server.url("/");
    let (client, manager) = make_client(config(json!([{"url":url,"action":"direct"}])));
    assert_eq!(
        client
            .execute_request("GET", &url, &[], &[], None)
            .await
            .unwrap()
            .body,
        b"original"
    );
    assert_eq!(server.count.load(Ordering::SeqCst), 1);
    manager.stop().await.unwrap();
}
#[tokio::test]
async fn named_proxy_dials_only_selected_node() {
    let origin = Server::start(false).await;
    let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = proxy.local_addr().unwrap().port();
    let url = origin.url("/");
    let mut cfg = config(json!([{"url":url,"action":"proxy(Selected)"}]));
    cfg["proxies"] = json!([{"name":"Selected","type":"vless","server":"127.0.0.1","port":port,"uuid":"00000000-0000-0000-0000-000000000000","flow":"xtls-rprx-vision","security":"reality","sni":"example.com","pbk":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE","sid":"01"}]);
    let (client, manager) = make_client(cfg);
    let request =
        tokio::spawn(async move { client.execute_request("GET", &url, &[], &[], None).await });
    let (socket, _) = timeout(Duration::from_secs(2), proxy.accept())
        .await
        .unwrap()
        .unwrap();
    drop(socket);
    assert!(
        timeout(Duration::from_secs(3), request)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert_eq!(origin.count.load(Ordering::SeqCst), 0);
    manager.stop().await.unwrap();
}
#[tokio::test]
async fn stopped_manager_cannot_construct_responses() {
    let (client, manager) = make_client(config(
        json!([{"url":"http://example.com/**","action":"respond","response":{"body":{"text":"local"}}}]),
    ));
    manager.stop().await.unwrap();
    assert!(matches!(
        client
            .execute_request("GET", "http://example.com/a", &[], &[], None)
            .await,
        Err(Error::InvalidState)
    ));
}

#[tokio::test]
async fn https_rewrites_after_verified_tls() {
    let fixture = support::fixture().await;
    let url = format!("https://localhost:{}/json", fixture.port);
    let mut cfg = config(
        json!([{"url":url,"phase":"response","when":{"status":200,"content-type":"application/json"},"action":"modify-response","response":{"body":{"text":"replaced-after-tls"}}}]),
    );
    cfg["rules"] = json!(["final,direct"]);
    fixture
        .network
        .apply_configuration(&cfg.to_string())
        .unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(fixture.root.clone()).unwrap();
    let client = HttpsClient::with_roots(roots)
        .unwrap()
        .with_network(fixture.network.clone());
    let reply = client.execute("GET", &url, &[], &[]).await.unwrap();
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body, b"replaced-after-tls");
    assert!(
        !reply
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("etag"))
    );
    fixture.network.stop().await.unwrap();
}
