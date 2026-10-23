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

pub(crate) use crate::network::SecureStream;
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
    maximum_body: Option<usize>,
    database_download: bool,
    skip_rewrites: bool,
    snapshot: Option<Arc<crate::outbound::NetworkSnapshot>>,
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
            .map_err(Error::from_tls)
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
        .map_err(|_| Error::NetworkTimeout)?
    }
    /// 订阅等小型内容复用 HTTPS 流程，在分配响应正文前执行更小的上限。
    pub async fn execute_limited(&self, target: &str, maximum: usize) -> Result<Response> {
        if maximum == 0 || maximum > http::MAX_BODY {
            return Err(Error::InvalidInput);
        }
        timeout(
            Duration::from_secs(60),
            self.execute_inner(
                "GET",
                target,
                &[],
                &[],
                ConnectionOptions {
                    maximum_body: Some(maximum),
                    skip_rewrites: true,
                    ..ConnectionOptions::default()
                },
            ),
        )
        .await
        .map_err(|_| Error::NetworkTimeout)?
    }
    /// 前置检查走真实路由、代理握手、目标 TLS 和 HTTP；不执行复写或跟随重定向。
    /// 返回实际连接决策，不能将另一次路由预测或监听端口存在当作代理成功。
    pub(crate) async fn probe(&self, target: &str) -> Result<crate::routing::Decision> {
        let url = Url::parse(target).map_err(|_| Error::InvalidInput)?;
        let host = url
            .host_str()
            .ok_or(Error::InvalidInput)?
            .trim_matches(['[', ']']);
        let socket = self
            .network
            .connect_stream(
                host,
                url.port_or_known_default().ok_or(Error::InvalidInput)?,
                None,
            )
            .await?;
        let decision = socket.decision();
        let name = ServerName::try_from(host.to_owned()).map_err(|_| Error::InvalidInput)?;
        let stream = TlsConnector::from(self.config.clone())
            .connect(name, socket)
            .await
            .map_err(Error::from_tls)?;
        let response = self
            .execute_wire(
                "GET",
                target,
                &[],
                &[],
                ConnectionOptions {
                    stream: Some(Box::new(stream)),
                    maximum_body: Some(4096),
                    skip_rewrites: true,
                    ..ConnectionOptions::default()
                },
            )
            .await?;
        if !(200..300).contains(&response.status) {
            return Err(Error::Network);
        }
        Ok(decision)
    }
    /// GeoIP 专用大文件入口；调用者必须先持有全量正文预算，证书/路由/超时仍共用原流程。
    pub(crate) async fn download_database(&self, target: &str) -> Result<Response> {
        timeout(
            Duration::from_secs(60),
            self.execute_inner(
                "GET",
                target,
                &[],
                &[],
                ConnectionOptions {
                    database_download: true,
                    skip_rewrites: true,
                    ..ConnectionOptions::default()
                },
            ),
        )
        .await
        .map_err(|_| Error::NetworkTimeout)?
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
        .map_err(|_| Error::NetworkTimeout)?
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
                    maximum_body: None,
                    database_download: false,
                    proxy_port,
                    stream: None,
                    skip_rewrites: false,
                    snapshot: None,
                },
            ),
        )
        .await
        .map_err(|_| Error::NetworkTimeout)?
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
            .map_err(Error::from_tls)
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
        mut options: ConnectionOptions,
    ) -> Result<Response> {
        if options.skip_rewrites {
            return self
                .execute_wire(method, target, headers, body, options)
                .await;
        }
        let snapshot = self.network.network_snapshot()?;
        let Some(document) = snapshot.configuration.as_ref().filter(|d| {
            snapshot.configuration_complete && (d.has_rewrites() || snapshot.plugins.has_rules())
        }) else {
            return self
                .execute_wire(method, target, headers, body, options)
                .await;
        };
        validate_rewrite_input(method, target, headers, body, options.allow_http)?;
        if options.proxy_port == Some(0) {
            return Err(Error::InvalidInput);
        }
        static MEMORY: crate::budget::ByteBudget =
            crate::budget::ByteBudget::new(256 * 1024 * 1024);
        let _memory = MEMORY.reserve(4 * http::MAX_BODY + 4 * http::MAX_HEADER)?;
        let owner = snapshot.session.clone().ok_or(Error::InvalidState)?;
        let _operation = owner.enter(owner.identity())?;
        let original = crate::configuration::rewrites::execution::Message {
            method: method.into(),
            url: target.into(),
            headers: headers.into(),
            body: body.into(),
        };
        let mut request = original.clone();
        let outcome = document.rewrite_request_bound(
            &original,
            &mut request,
            Some(&snapshot.scripts),
            &snapshot.plugins,
        )?;
        let route = match outcome {
            crate::configuration::rewrites::execution::Outcome::Response(mut value) => {
                if method == "HEAD" {
                    value.body.clear();
                }
                return owner.with_active(owner.identity(), || Ok(value));
            }
            crate::configuration::rewrites::execution::Outcome::Route(policy, node) => {
                Some((policy, node))
            }
            crate::configuration::rewrites::execution::Outcome::Continue => None,
            crate::configuration::rewrites::execution::Outcome::Script(binding) => {
                crate::configuration::rewrites::script_message::execute(
                    binding,
                    &self.network.scripts,
                    &mut request,
                    None,
                )
                .await?;
                None
            }
        };
        let mut execution = snapshot.as_ref().clone();
        if let Some((policy, node)) = route {
            // 既有连接和显式本地中转不能冒充本次新选出的出站。
            if options.proxy_port.is_some() || options.stream.is_some() {
                return Err(Error::Unsupported);
            }
            execution.router = Some(Arc::new(
                execution
                    .router
                    .as_ref()
                    .ok_or(Error::InvalidState)?
                    .for_http_rewrite(policy, node)?,
            ));
        }
        options.snapshot = Some(Arc::new(execution));
        let result = async {
            let mut response = self
                .execute_wire(
                    &request.method,
                    &request.url,
                    &request.headers,
                    &request.body,
                    options,
                )
                .await?;
            document
                .rewrite_response_bound(
                    &original,
                    &mut response,
                    &snapshot.scripts,
                    &self.network.scripts,
                    &snapshot.plugins,
                )
                .await?;
            owner.with_active(owner.identity(), || Ok(response))
        };
        tokio::select! {biased; _=owner.canceled()=>Err(Error::Cancelled), result=result=>result}
    }
    async fn connect_http_request(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        proxy_port: Option<u16>,
        snapshot: Option<Arc<crate::outbound::NetworkSnapshot>>,
    ) -> Result<SecureStream> {
        let snapshot = match snapshot {
            Some(snapshot) => snapshot,
            None => self.network.network_snapshot()?,
        };
        let socket = self
            .network
            .connect_stream_snapshot(host, port, proxy_port, snapshot)
            .await?;
        let mut socket = BufReader::new(socket);
        if tls && proxy_port.is_some() {
            use tokio::io::AsyncWriteExt;
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
        }
        if tls {
            let name = ServerName::try_from(host.to_owned()).map_err(|_| Error::InvalidInput)?;
            TlsConnector::from(self.config.clone())
                .connect(name, socket)
                .await
                .map(|s| Box::new(s) as SecureStream)
                .map_err(Error::from_tls)
        } else {
            Ok(Box::new(socket))
        }
    }
    /// 协议下载跳过业务复写，防止本地响应篡改提供者及数据库。
    async fn execute_wire(
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
                self.connect_http_request(
                    &host,
                    port,
                    url.scheme() == "https",
                    options.proxy_port,
                    options.snapshot,
                )
                .await?
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
            let head = if options.database_download {
                http::read_database_head(&mut stream).await?
            } else {
                http::read_head(&mut stream).await?
            };
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
        } else if options.database_download {
            if status != 200 {
                return Err(Error::GeoipDownloadFailed);
            }
            http::read_database_body(&mut stream, &head).await?
        } else {
            http::read_body_limited(
                &mut stream,
                &head,
                true,
                options.maximum_body.unwrap_or(http::MAX_BODY),
            )
            .await?
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

#[cfg(test)]
#[path = "../tests/test_https.rs"]
mod test_https;

/// 本地响应也必须先核验原始输入，不能利用规则命中绕过协议及资源限制。
fn validate_rewrite_input(
    method: &str,
    target: &str,
    headers: &[Header],
    body: &[u8],
    allow_http: bool,
) -> Result<()> {
    if target.len() > 4096 {
        return Err(Error::ResourceLimit);
    }
    let url = Url::parse(target).map_err(|_| Error::InvalidInput)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.port_or_known_default() == Some(0)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || target.bytes().any(|b| b <= 32 || b == 127 || b == b'\\')
    {
        return Err(Error::InvalidInput);
    }
    if !allow_http && !target.starts_with("https://")
        || method.is_empty()
        || method.len() > 32
        || !method.bytes().all(is_token)
        || method.eq_ignore_ascii_case("CONNECT")
    {
        return Err(Error::InvalidInput);
    }
    if headers.len() > 128 || body.len() > http::MAX_BODY {
        return Err(Error::ResourceLimit);
    }
    let mut total = 0;
    for header in headers {
        total += header.name.len() + header.value.len() + 4;
        if header.name.is_empty()
            || !header.name.bytes().all(is_token)
            || header.value.bytes().any(|b| b < 32 || b == 127)
            || [
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
            .iter()
            .any(|n| header.name.eq_ignore_ascii_case(n))
        {
            return Err(Error::InvalidInput);
        }
    }
    if total > http::MAX_HEADER {
        return Err(Error::ResourceLimit);
    }
    Ok(())
}
