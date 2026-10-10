//! Rust HTTPS 解密客户端：系统根证书校验、有限 HTTP/1.1 分帧和明确错误。
pub use crate::network::normalize_host;
use crate::{
    error::{Error, Result},
    http::{self, Header},
};
use rustls::{ClientConfig, RootCertStore, pki_types::ServerName};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{io::BufReader, time::timeout};
use tokio_rustls::TlsConnector;
use url::Url;

pub(crate) trait Duplex:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send
{
}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> Duplex for T {}
pub(crate) type SecureStream = Box<dyn Duplex>;
#[derive(Clone)]
pub struct HttpsClient {
    config: Arc<ClientConfig>,
    network: Arc<crate::outbound::Manager>,
}
pub struct Response {
    pub status: u16,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
    pub duration_ms: u64,
}
#[derive(Default)]
struct ConnectionOptions {
    stream: Option<SecureStream>,
    proxy_port: Option<u16>,
    allow_http: bool,
}
impl HttpsClient {
    /// 复用受验证的根证书配置供加密 DNS；不提供跳过证书校验入口。
    pub(crate) fn tls_config(&self) -> ClientConfig {
        self.config.as_ref().clone()
    }
    pub fn system() -> Result<Self> {
        Self::system_with_additional_roots(Vec::new())
    }
    pub fn system_with_additional_roots(
        additional: Vec<rustls::pki_types::CertificateDer<'static>>,
    ) -> Result<Self> {
        let certificates = rustls_native_certs::load_native_certs();
        let mut roots = RootCertStore::empty();
        for certificate in certificates.certs.into_iter().chain(additional) {
            roots.add(certificate).map_err(|_| Error::Certificate)?;
        }
        if roots.is_empty() {
            return Err(Error::Certificate);
        }
        Self::with_roots(roots)
    }
    /// 可注入企业或测试根证书，仍执行完整证书、域名和有效期校验。
    pub fn with_roots(roots: RootCertStore) -> Result<Self> {
        let mut config =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .map_err(|_| Error::Tls)?
                .with_root_certificates(roots)
                .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Self {
            config: Arc::new(config),
            network: Arc::new(crate::outbound::Manager::default()),
        })
    }
    /// 所有克隆共用连接管理；必须显式提供 DNS 与路由，不采用系统解析回退。
    pub fn with_network(mut self, network: Arc<crate::outbound::Manager>) -> Self {
        self.network = network;
        self
    }
    pub fn network(&self) -> Arc<crate::outbound::Manager> {
        Arc::clone(&self.network)
    }
    pub(crate) async fn connect(&self, host: &str, port: u16) -> Result<SecureStream> {
        let name = ServerName::try_from(normalize_host(host)?).map_err(|_| Error::InvalidInput)?;
        let socket = self.network.connect_stream(host, port, None).await?;
        TlsConnector::from(Arc::clone(&self.config))
            .connect(name, socket)
            .await
            .map(|stream| Box::new(stream) as SecureStream)
            .map_err(tls_error)
    }
    pub async fn execute(
        &self,
        method: &str,
        target: &str,
        headers: &[Header],
        body: &[u8],
    ) -> Result<Response> {
        timeout(
            Duration::from_secs(60),
            self.execute_inner(method, target, headers, body, ConnectionOptions::default()),
        )
        .await
        .map_err(|_| Error::Network)?
    }
    /// 通过本机 CONNECT 入口执行 HTTPS；不会失败后回退直连。
    pub async fn execute_via_proxy(
        &self,
        method: &str,
        target: &str,
        headers: &[Header],
        body: &[u8],
        proxy_port: u16,
    ) -> Result<Response> {
        timeout(
            Duration::from_secs(60),
            self.execute_inner(
                method,
                target,
                headers,
                body,
                ConnectionOptions {
                    proxy_port: Some(proxy_port),
                    ..ConnectionOptions::default()
                },
            ),
        )
        .await
        .map_err(|_| Error::Network)?
    }
    /// HTTP 与 HTTPS 共用分帧；明确给定本地上游时不回退直连。
    pub async fn execute_request(
        &self,
        method: &str,
        target: &str,
        headers: &[Header],
        body: &[u8],
        proxy_port: Option<u16>,
    ) -> Result<Response> {
        timeout(
            Duration::from_secs(60),
            self.execute_inner(
                method,
                target,
                headers,
                body,
                ConnectionOptions {
                    allow_http: true,
                    proxy_port,
                    stream: None,
                },
            ),
        )
        .await
        .map_err(|_| Error::Network)?
    }
    pub(crate) async fn connect_tls(
        &self,
        host: &str,
        port: u16,
        proxy_port: Option<u16>,
    ) -> Result<SecureStream> {
        match proxy_port {
            Some(proxy_port) => self.connect_via_proxy(host, port, proxy_port).await,
            None => self.connect(host, port).await,
        }
    }
    pub(crate) async fn connect_plain(
        &self,
        host: &str,
        port: u16,
        proxy_port: Option<u16>,
    ) -> Result<SecureStream> {
        match proxy_port {
            Some(proxy_port) => Ok(Box::new(self.open_tunnel(host, port, proxy_port).await?)),
            None => Ok(Box::new(
                self.network.connect_stream(host, port, None).await?,
            )),
        }
    }
    async fn connect_via_proxy(
        &self,
        host: &str,
        port: u16,
        proxy_port: u16,
    ) -> Result<SecureStream> {
        let socket = self.open_tunnel(host, port, proxy_port).await?;
        let name = ServerName::try_from(host.to_owned()).map_err(|_| Error::InvalidInput)?;
        TlsConnector::from(Arc::clone(&self.config))
            .connect(name, socket)
            .await
            .map(|stream| Box::new(stream) as SecureStream)
            .map_err(tls_error)
    }
    async fn open_tunnel(
        &self,
        host: &str,
        port: u16,
        proxy_port: u16,
    ) -> Result<BufReader<crate::outbound::ManagedStream>> {
        use tokio::io::AsyncWriteExt;
        if proxy_port == 0 {
            return Err(Error::InvalidInput);
        }
        let mut socket = BufReader::new(
            self.network
                .connect_stream(host, port, Some(proxy_port))
                .await?,
        );
        let authority = if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        socket
            .get_mut()
            .write_all(
                format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes(),
            )
            .await?;
        let head = http::read_head(&mut socket).await?;
        let parts: Vec<_> = head.first.splitn(3, ' ').collect();
        if parts.len() < 2
            || parts[0] != "HTTP/1.1"
            || parts[1] != "200"
            || head.value("transfer-encoding").is_some()
            || head.value("content-length").is_some_and(|v| v != "0")
        {
            return Err(Error::Tls);
        }
        Ok(socket)
    }
    pub(crate) async fn execute_on_stream(
        &self,
        method: &str,
        target: &str,
        headers: &[Header],
        body: &[u8],
        stream: SecureStream,
    ) -> Result<Response> {
        self.execute_inner(
            method,
            target,
            headers,
            body,
            ConnectionOptions {
                stream: Some(stream),
                ..ConnectionOptions::default()
            },
        )
        .await
    }
    async fn execute_inner(
        &self,
        method: &str,
        target: &str,
        headers: &[Header],
        body: &[u8],
        options: ConnectionOptions,
    ) -> Result<Response> {
        let started = Instant::now();
        if target.bytes().any(|b| b < 32 || b == 127) {
            return Err(Error::InvalidInput);
        }
        let url = Url::parse(target).map_err(|_| Error::InvalidInput)?;
        if !(url.scheme() == "https" || options.allow_http && url.scheme() == "http")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::InvalidInput);
        }
        if method.is_empty()
            || !method.bytes().all(is_token)
            || method.eq_ignore_ascii_case("CONNECT")
        {
            return Err(Error::InvalidInput);
        }
        if body.len() > http::MAX_BODY || headers.len() > 128 {
            return Err(Error::ResourceLimit);
        }
        let host = normalize_host(url.host_str().ok_or(Error::InvalidInput)?)?;
        let port = url
            .port_or_known_default()
            .filter(|p| *p != 0)
            .ok_or(Error::InvalidInput)?;
        let authority = if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        let mut forwarded = Vec::new();
        let mut header_bytes: usize = 0;
        for header in headers {
            header_bytes = header_bytes
                .checked_add(header.name.len() + header.value.len() + 4)
                .ok_or(Error::ResourceLimit)?;
            if header_bytes > http::MAX_HEADER
                || header.name.is_empty()
                || !header.name.bytes().all(is_token)
                || header.value.bytes().any(|b| b < 32 || b == 127)
            {
                return Err(Error::InvalidInput);
            }
            let name = header.name.to_ascii_lowercase();
            if [
                "host",
                "content-length",
                "transfer-encoding",
                "connection",
                "proxy-connection",
                "proxy-authorization",
                "upgrade",
                "expect",
                "trailer",
            ]
            .contains(&name.as_str())
            {
                return Err(Error::InvalidInput);
            }
            if name != "accept-encoding" {
                forwarded.push(header.clone());
            }
        }
        forwarded.push(Header {
            name: "Host".into(),
            value: authority,
        });
        forwarded.push(Header {
            name: "Accept-Encoding".into(),
            value: "identity".into(),
        });
        if options.proxy_port == Some(0) {
            return Err(Error::InvalidInput);
        }
        let http_proxy =
            url.scheme() == "http" && options.proxy_port.is_some() && options.stream.is_none();
        let mut path = url.path().to_owned();
        if let Some(query) = url.query() {
            path.push('?');
            path.push_str(query);
        }
        if http_proxy {
            path = url.as_str().to_owned();
        }
        if path.len() + header_bytes > http::MAX_HEADER {
            return Err(Error::ResourceLimit);
        }
        let stream = match options.stream {
            Some(stream) => stream,
            None => {
                if let Some(proxy_port) = options.proxy_port.filter(|_| http_proxy) {
                    Box::new(
                        self.network
                            .connect_stream(&host, port, Some(proxy_port))
                            .await?,
                    ) as SecureStream
                } else if url.scheme() == "http" {
                    self.connect_plain(&host, port, None).await?
                } else {
                    self.connect_tls(&host, port, options.proxy_port).await?
                }
            }
        };
        let mut stream = BufReader::new(stream);
        http::write_message(
            stream.get_mut(),
            &format!("{method} {path} HTTP/1.1"),
            &forwarded,
            body,
            false,
        )
        .await?;
        let mut informational = 0;
        let (head, status) = loop {
            let head = http::read_head(&mut stream).await?;
            let mut parts = head.first.splitn(3, ' ');
            if !matches!(parts.next(), Some("HTTP/1.1" | "HTTP/1.0")) {
                return Err(Error::InvalidInput);
            }
            let code = parts
                .next()
                .filter(|c| c.len() == 3 && c.bytes().all(|b| b.is_ascii_digit()))
                .ok_or(Error::InvalidInput)?;
            let status: u16 = code.parse().map_err(|_| Error::InvalidInput)?;
            if !(100..=599).contains(&status) || status == 101 {
                return Err(Error::Unsupported);
            }
            if status >= 200 {
                break (head, status);
            }
            informational += 1;
            if informational > 4 {
                return Err(Error::ResourceLimit);
            }
        };
        let response_body = if method.eq_ignore_ascii_case("HEAD") || matches!(status, 204 | 304) {
            Vec::new()
        } else {
            http::read_body(&mut stream, &head, true).await?
        };
        Ok(Response {
            status,
            headers: head.headers,
            body: response_body,
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        })
    }
}
pub(crate) fn is_token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn tls_error(error: std::io::Error) -> Error {
    match Error::from(error) {
        Error::Cancelled => Error::Cancelled,
        _ => Error::Tls,
    }
}
