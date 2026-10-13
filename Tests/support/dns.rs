//! DNS 回环服务：UDP/TCP/DoT/HTTP2 DoH，独立端口和私有测试证书，不修改系统信任。
use hickory_resolver::proto::{
    op::{Message, MessageType, ResponseCode},
    rr::{
        Name, RData, Record, RecordType,
        rdata::{A, AAAA, CNAME, SOA, TXT},
    },
};
use rcgen::{CertificateParams, KeyPair};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
    task::{JoinHandle, JoinSet},
};
use tokio_rustls::TlsAcceptor;

// 共用夹具供独立 DNS 与 HTTP 测试目标复用；各目标只读取自身需要的协议信息。
#[allow(dead_code)]
pub struct Fixture {
    pub port: u16,
    pub dot_port: u16,
    pub doh_port: u16,
    pub root: CertificateDer<'static>,
    pub pem: String,
    pub count: Arc<AtomicUsize>,
    tasks: Vec<JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
// TCP 的临时端口不保证 UDP 同号端口空闲；只对端口占用进行有界重试。
async fn bind_pair() -> std::io::Result<(TcpListener, UdpSocket)> {
    for _ in 0..32 {
        let tcp = TcpListener::bind("127.0.0.1:0").await?;
        let port = tcp.local_addr()?.port();
        match UdpSocket::bind(("127.0.0.1", port)).await {
            Ok(udp) => return Ok((tcp, udp)),
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AddrInUse,
        "测试 DNS 同号 TCP/UDP 端口分配失败",
    ))
}
pub async fn fixture() -> Fixture {
    let (tcp, udp) = bind_pair().await.unwrap();
    let port = tcp.local_addr().unwrap().port();
    let dot = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dot_port = dot.local_addr().unwrap().port();
    let doh = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let doh_port = doh.local_addr().unwrap().port();
    let key = KeyPair::generate().unwrap();
    let cert = CertificateParams::new(vec!["localhost".into()])
        .unwrap()
        .self_signed(&key)
        .unwrap();
    let root = cert.der().clone();
    let pem = cert.pem();
    let mut config =
        ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![root.clone()],
                PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
            )
            .unwrap();
    let dot_accept = TlsAcceptor::from(Arc::new(config.clone()));
    config.alpn_protocols = vec![b"h2".to_vec()];
    let doh_accept = TlsAcceptor::from(Arc::new(config));
    let count = Arc::new(AtomicUsize::new(0));
    let counter = count.clone();
    let udp_task = tokio::spawn(async move {
        let mut buf = [0; 4096];
        loop {
            let Ok((len, peer)) = udp.recv_from(&mut buf).await else {
                break;
            };
            if let Some(reply) = answer(&buf[..len], true, &counter) {
                let _ = udp.send_to(&reply, peer).await;
            }
        }
    });
    let mut tasks = vec![udp_task];
    for (listener, acceptor, is_doh) in [
        (tcp, None, false),
        (dot, Some(dot_accept), false),
        (doh, Some(doh_accept), true),
    ] {
        let counter = count.clone();
        tasks.push(tokio::spawn(async move {let mut sessions=JoinSet::new();loop {tokio::select! {
            socket=listener.accept()=>{let Ok((socket,_))=socket else {break;};let acceptor=acceptor.clone();let counter=counter.clone();sessions.spawn(async move {
                if let Some(acceptor)=acceptor {let Ok(tls)=acceptor.accept(socket).await else{return;};if is_doh {serve_h2(tls,counter).await;}else{serve(tls,counter).await;}}
                else {serve(socket,counter).await;}
            });}
            _=sessions.join_next(),if !sessions.is_empty()=>{}
        }}}));
    }
    Fixture {
        port,
        dot_port,
        doh_port,
        root,
        pem,
        count,
        tasks,
    }
}
async fn serve<S: AsyncRead + AsyncWrite + Unpin>(mut socket: S, count: Arc<AtomicUsize>) {
    loop {
        let Ok(len) = socket.read_u16().await else {
            return;
        };
        if len > 4096 {
            return;
        }
        let mut bytes = vec![0; len as usize];
        if socket.read_exact(&mut bytes).await.is_err() {
            return;
        }
        let Some(reply) = answer(&bytes, false, &count) else {
            return;
        };
        if socket
            .write_all(&(reply.len() as u16).to_be_bytes())
            .await
            .is_err()
            || socket.write_all(&reply).await.is_err()
        {
            return;
        }
    }
}
async fn serve_h2<S: AsyncRead + AsyncWrite + Unpin>(socket: S, count: Arc<AtomicUsize>) {
    let Ok(mut connection) = h2::server::handshake(socket).await else {
        return;
    };
    while let Some(Ok((request, mut sender))) = connection.accept().await {
        let mut stream = request.into_body();
        let mut bytes = vec![];
        while let Some(Ok(chunk)) = stream.data().await {
            if bytes.len() + chunk.len() > 4096 {
                return;
            }
            bytes.extend(&chunk);
            let _ = stream.flow_control().release_capacity(chunk.len());
        }
        let Some(reply) = answer(&bytes, false, &count) else {
            return;
        };
        let response = http::Response::builder()
            .status(200)
            .header("content-type", "application/dns-message")
            .header("content-length", reply.len())
            .body(())
            .unwrap();
        if let Ok(mut stream) = sender.send_response(response, false) {
            let _ = stream.send_data(bytes::Bytes::from(reply), true);
        }
    }
}
fn answer(bytes: &[u8], udp: bool, count: &AtomicUsize) -> Option<Vec<u8>> {
    let query = Message::from_vec(bytes).ok()?;
    let question = query.queries.first()?.clone();
    count.fetch_add(1, Ordering::SeqCst);
    let name = question.name.to_ascii().to_ascii_lowercase();
    if name.starts_with("timeout.") {
        return None;
    }
    let mut reply = Message::query();
    reply.metadata.id = query.metadata.id;
    reply.metadata.message_type = MessageType::Response;
    reply.metadata.recursion_available = true;
    reply.metadata.authoritative = true;
    reply.add_query(question.clone());
    if (name.starts_with("tcp.")
        || (name.starts_with("large.") && question.query_type == RecordType::TXT))
        && udp
    {
        reply.metadata.truncation = true;
        return reply.to_vec().ok();
    }
    if name.starts_with("mismatch.") {
        reply.metadata.id ^= 1;
        return reply.to_vec().ok();
    }
    // 仅测试夹具的单 IPv4 目标，验证宿主没有偷偷替换候选地址。
    if name.starts_with("nxdomain.")
        || name.starts_with("nodata.")
        || (name.starts_with("connect.ipv4only.") && question.query_type == RecordType::AAAA)
    {
        if name.starts_with("nxdomain.") {
            reply.metadata.response_code = ResponseCode::NXDomain;
        }
        reply.add_authority(Record::from_rdata(
            Name::from_ascii("example.test.").unwrap(),
            30,
            RData::SOA(SOA::new(
                Name::from_ascii("ns.example.test.").unwrap(),
                Name::from_ascii("admin.example.test.").unwrap(),
                1,
                60,
                60,
                300,
                5,
            )),
        ));
    } else {
        let ttl = if name.starts_with("zero.") {
            0
        } else if name.starts_with("short.") {
            1
        } else {
            60
        };
        let mut owner = question.name;
        if name.starts_with("alias.") {
            let target = Name::from_ascii(if name.starts_with("alias.connect.") {
                "connect.example.test."
            } else {
                "target.example.test."
            })
            .unwrap();
            reply.add_answer(Record::from_rdata(
                owner,
                10,
                RData::CNAME(CNAME(target.clone())),
            ));
            owner = target;
        }
        let data = match question.query_type {
            RecordType::A => RData::A(
                if name == "localhost."
                    || name.starts_with("connect.")
                    || name.starts_with("alias.connect.")
                {
                    A::new(127, 0, 0, 1)
                } else {
                    A::new(192, 0, 2, 44)
                },
            ),
            RecordType::AAAA => RData::AAAA(AAAA(
                if name == "localhost."
                    || name.starts_with("connect.")
                    || name.starts_with("alias.connect.")
                {
                    "::1"
                } else {
                    "2001:db8::44"
                }
                .parse()
                .unwrap(),
            )),
            RecordType::TXT => RData::TXT(TXT::new(if name.starts_with("large.") {
                vec!["x".repeat(200); 10]
            } else {
                vec!["dns-fixture".into()]
            })),
            _ => return reply.to_vec().ok(),
        };
        reply.add_answer(Record::from_rdata(owner, ttl, data));
        // AD 只是上游声明；此无签名回复不能被客户端当作 DNSSEC secure。
        if name.starts_with("ad-only.") {
            reply.metadata.authentic_data = true;
        }
    }
    reply.to_vec().ok()
}
