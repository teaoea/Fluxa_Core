//! Rust 调试代理：HTTP、CONNECT 透传和精确域名 MITM 共用会话与资源管理。
pub mod authority;
pub mod capture;
use crate::{
    error::{Error, Result},
    http::{self, Header},
    https::{HttpsClient, SecureStream, is_token, normalize_host},
    network::Duplex,
    rewrite::{self, Rewrite},
};
use authority::Authority;
use capture::{Capture, CaptureStore};
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::{JoinHandle, JoinSet},
    time::timeout,
};
use tokio_rustls::TlsAcceptor;
use url::Url;

#[derive(Default)]
pub struct ProxyOptions {
    pub port: u16,
    pub hosts: Vec<String>,
    pub capture_bodies: bool,
    pub allow_lan: bool,
    pub upstream_proxy_port: Option<u16>,
    pub rewrites: Vec<Rewrite>,
}
struct SessionState {
    hosts: BTreeSet<String>,
    authority: Arc<Authority>,
    upstream: HttpsClient,
    options: ProxyOptions,
}
pub struct MitmProxy {
    port: u16,
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
}
impl MitmProxy {
    pub async fn start(
        port: u16,
        hosts: &[String],
        authority: Arc<Authority>,
        upstream: HttpsClient,
        captures: Arc<CaptureStore>,
        capture_bodies: bool,
    ) -> Result<Self> {
        Self::start_with_options(
            ProxyOptions {
                port,
                hosts: hosts.to_vec(),
                capture_bodies,
                ..ProxyOptions::default()
            },
            authority,
            upstream,
            captures,
        )
        .await
    }
    pub async fn start_with_options(
        options: ProxyOptions,
        authority: Arc<Authority>,
        upstream: HttpsClient,
        captures: Arc<CaptureStore>,
    ) -> Result<Self> {
        // LAN 认证与网关尚未交付，不开放无认证的局域网监听，不静默改绑回环。
        if options.allow_lan {
            return Err(Error::Unsupported);
        }
        if options.upstream_proxy_port == Some(0) {
            return Err(Error::InvalidInput);
        }
        if options.hosts.len() > 128 || options.rewrites.len() > 256 {
            return Err(Error::ResourceLimit);
        }
        for rule in &options.rewrites {
            rule.validate()?;
        }
        let hosts = options
            .hosts
            .iter()
            .map(|h| normalize_host(h))
            .collect::<Result<BTreeSet<_>>>()?;
        if upstream.network().network_snapshot()?.router.is_none() {
            return Err(Error::InvalidState);
        }
        let listener = TcpListener::bind(("127.0.0.1", options.port)).await?;
        let port = listener.local_addr()?.port();
        if options.upstream_proxy_port == Some(port) {
            return Err(Error::InvalidInput);
        }
        let state = Arc::new(SessionState {
            hosts,
            authority,
            upstream,
            options,
        });
        let (stop, mut stopped) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut sessions = JoinSet::new();
            loop {
                tokio::select! {
                    _ = stopped.changed() => break,
                    accepted = listener.accept() => {
                        let (socket, _)=match accepted {Ok(value)=>value,Err(error)=>{crate::error::diagnostics::record(error.into(),crate::error::diagnostics::DiagnosticSite::MitmCapture);break;}};
                        static CONNECTIONS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8);
                        let Ok(permit) = CONNECTIONS.try_acquire() else { crate::error::diagnostics::record(Error::ResourceLimit, crate::error::diagnostics::DiagnosticSite::MitmCapture); drop(socket); continue; };
                        if sessions.len() >= 32 { crate::error::diagnostics::record(Error::ResourceLimit, crate::error::diagnostics::DiagnosticSite::MitmCapture); drop(socket); continue; }
                        let (state, captures) = (Arc::clone(&state), Arc::clone(&captures));
                        sessions.spawn(async move {
                            let _permit = permit;
                            let started = Instant::now(); let mut capture = Capture::new();
                            let result = timeout(Duration::from_secs(60), session(socket, &state, &mut capture)).await;
                            capture.duration_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
                            if let Err(error) = result.unwrap_or(Err(Error::NetworkTimeout)) { capture.error = Some(error.to_string()); capture.error_info = Some(error.public_info()); }
                            let _ = crate::error::diagnostics::observe(captures.push(capture), crate::error::diagnostics::DiagnosticSite::MitmCapture);
                        });
                    }
                    joined = sessions.join_next(), if !sessions.is_empty() => {if let Some(result)=joined {crate::error::diagnostics::observe_join(result,crate::error::diagnostics::DiagnosticSite::MitmCapture);}}
                }
            }
            sessions.abort_all();
            while let Some(result) = sessions.join_next().await {
                crate::error::diagnostics::observe_join(
                    result,
                    crate::error::diagnostics::DiagnosticSite::MitmCapture,
                );
            }
        });
        Ok(Self {
            port,
            stop,
            task: Some(task),
        })
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub async fn stop(mut self) -> Result<()> {
        let _ = self.stop.send(true);
        if let Some(task) = self.task.take() {
            task.await.map_err(|_| Error::Internal)?;
        }
        Ok(())
    }
}
impl Drop for MitmProxy {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}
fn parse_port(value: &str) -> Result<u16> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidInput);
    }
    value.parse().map_err(|_| Error::InvalidInput)
}
fn authority_target(value: &str, default_port: Option<u16>) -> Result<(String, u16)> {
    let (host, port) = if value.starts_with('[') {
        let close = value.find(']').ok_or(Error::InvalidInput)?;
        let host = &value[1..close];
        let rest = &value[close + 1..];
        let port = if rest.is_empty() {
            default_port.ok_or(Error::InvalidInput)?
        } else {
            parse_port(rest.strip_prefix(':').ok_or(Error::InvalidInput)?)?
        };
        (host, port)
    } else if let Some((host, port)) = value.rsplit_once(':') {
        if host.contains(':') {
            return Err(Error::InvalidInput);
        }
        (host, parse_port(port)?)
    } else {
        (value, default_port.ok_or(Error::InvalidInput)?)
    };
    if port == 0 {
        return Err(Error::InvalidInput);
    }
    Ok((normalize_host(host)?, port))
}
async fn session(socket: TcpStream, state: &SessionState, capture: &mut Capture) -> Result<()> {
    let mut socket = BufReader::new(socket);
    let head = http::read_head(&mut socket).await?;
    let parts = head.parts();
    if parts.len() != 3 || !matches!(parts[2], "HTTP/1.1" | "HTTP/1.0") {
        return Err(Error::InvalidInput);
    }
    if parts[0] != "CONNECT" {
        let target = Url::parse(parts[1]).map_err(|_| Error::InvalidInput)?;
        if target.scheme() != "http"
            || !target.username().is_empty()
            || target.password().is_some()
            || target.fragment().is_some()
        {
            return Err(Error::InvalidInput);
        }
        let host = normalize_host(target.host_str().ok_or(Error::InvalidInput)?)?;
        validate_host(
            &head,
            &host,
            target.port_or_known_default().ok_or(Error::InvalidInput)?,
            80,
        )?;
        return forward(&mut socket, &head, target.as_str(), None, state, capture).await;
    }
    if head.value("transfer-encoding").is_some()
        || head.value("content-length").is_some_and(|v| v != "0")
    {
        return Err(Error::InvalidInput);
    }
    let (host, port) = authority_target(parts[1], None)?;
    capture.url = format!("https://{}/", parts[1]);
    capture.request_headers = head.headers.clone();
    let snapshot = state.upstream.network().network_snapshot()?;
    let allowed = if let Some(main) = snapshot
        .configuration
        .as_ref()
        .filter(|_| snapshot.configuration_complete)
    {
        // 独立授权的显式调试目标仍受主配置禁止解密列表限制。
        !main.mitm_excluded(&host)
            && (state.hosts.contains(&host) || snapshot.plugins.decrypts(main, &host))
    } else {
        state.hosts.contains(&host)
    };
    if !allowed {
        let mut remote = state
            .upstream
            .connect_plain(&host, port, state.options.upstream_proxy_port)
            .await?;
        socket
            .get_mut()
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        capture.status = Some(200);
        tokio::io::copy_bidirectional(&mut socket, &mut remote).await?;
        return Ok(());
    }
    capture.intercepted = true;
    let remote = match state
        .upstream
        .connect_tls(&host, port, state.options.upstream_proxy_port)
        .await
    {
        Ok(stream) => stream,
        Err(error) => {
            return failure(
                &mut socket,
                capture,
                error,
                502,
                state.options.capture_bodies,
            )
            .await;
        }
    };
    let config = state.authority.server_config(&host)?;
    socket
        .get_mut()
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;
    let tls = TlsAcceptor::from(config)
        .accept(socket)
        .await
        .map_err(Error::from_tls)?;
    if tls
        .get_ref()
        .1
        .server_name()
        .is_some_and(|name| normalize_host(name).ok().as_ref() != Some(&host))
    {
        return Err(Error::InvalidInput);
    }
    let mut tls = BufReader::new(tls);
    let request = http::read_head(&mut tls).await?;
    let parts = request.parts();
    if parts.len() != 3
        || !parts[1].starts_with('/')
        || parts[1].starts_with("//")
        || parts[1].contains('#')
    {
        return Err(Error::InvalidInput);
    }
    validate_host(&request, &host, port, 443)?;
    let target_host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host
    };
    let target = format!("https://{target_host}:{port}{}", parts[1]);
    forward(&mut tls, &request, &target, Some(remote), state, capture).await
}
fn validate_host(head: &http::Head, host: &str, port: u16, default: u16) -> Result<()> {
    let headers: Vec<_> = head
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("host"))
        .collect();
    if headers.len() != 1
        || authority_target(&headers[0].value, Some(default))? != (host.to_owned(), port)
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
async fn failure<S: Duplex>(
    socket: &mut BufReader<S>,
    capture: &mut Capture,
    error: Error,
    status: u16,
    capture_bodies: bool,
) -> Result<()> {
    let body = error.to_string().into_bytes();
    capture.status = Some(status);
    capture.response_headers = vec![Header {
        name: "Content-Type".into(),
        value: "text/plain; charset=utf-8".into(),
    }];
    capture.response_body_length = body.len();
    if capture_bodies {
        capture.response_body = body.clone();
    }
    http::write_message(
        socket.get_mut(),
        &format!("HTTP/1.1 {status} Fluxa"),
        &capture.response_headers,
        &body,
        false,
    )
    .await?;
    socket.get_mut().shutdown().await?;
    Err(error)
}
async fn forward<S: Duplex>(
    socket: &mut BufReader<S>,
    head: &http::Head,
    target: &str,
    remote: Option<SecureStream>,
    state: &SessionState,
    capture: &mut Capture,
) -> Result<()> {
    let parts = head.parts();
    if parts.len() != 3
        || parts[0].is_empty()
        || !parts[0].bytes().all(is_token)
        || parts[0] == "CONNECT"
        || !matches!(parts[2], "HTTP/1.1" | "HTTP/1.0")
        || head.value("upgrade").is_some()
        || head.value("expect").is_some()
    {
        return Err(Error::Unsupported);
    }
    capture.method = parts[0].to_owned();
    capture.url = target.to_owned();
    capture.request_headers = head.headers.clone();
    let body = http::read_body(socket, head, false).await?;
    if !socket.buffer().is_empty() {
        return Err(Error::Unsupported);
    }
    capture.request_body_length = body.len();
    if state.options.capture_bodies {
        capture.request_body = body.clone();
    }
    let mut headers: Vec<Header> = http::forwarded_headers(&head.headers)
        .into_iter()
        .filter(|h| {
            ![
                "host",
                "content-length",
                "transfer-encoding",
                "proxy-authorization",
            ]
            .contains(&h.name.to_ascii_lowercase().as_str())
        })
        .collect();
    if let Err(error) = rewrite::request(target, &mut headers, &state.options.rewrites) {
        return failure(socket, capture, error, 400, state.options.capture_bodies).await;
    }
    let result = match remote {
        Some(remote) => {
            state
                .upstream
                .execute_on_stream(parts[0], target, &headers, &body, remote)
                .await
        }
        None => {
            state
                .upstream
                .execute_request(
                    parts[0],
                    target,
                    &headers,
                    &body,
                    state.options.upstream_proxy_port,
                )
                .await
        }
    };
    let response = match result {
        Ok(response) => response,
        Err(Error::Cancelled) => {
            socket.get_mut().shutdown().await?;
            return Err(Error::Cancelled);
        }
        Err(error) => {
            return failure(socket, capture, error, 502, state.options.capture_bodies).await;
        }
    };
    let response = match rewrite::process_response(target, response, &state.options.rewrites).await
    {
        Ok(response) => response,
        Err(Error::Cancelled) => {
            socket.get_mut().shutdown().await?;
            return Err(Error::Cancelled);
        }
        Err(error) => {
            return failure(socket, capture, error, 502, state.options.capture_bodies).await;
        }
    };
    capture.status = Some(response.status);
    capture.response_headers = response.headers.clone();
    capture.response_body_length = response.body.len();
    if state.options.capture_bodies {
        capture.response_body = response.body.clone();
    }
    let no_body = parts[0].eq_ignore_ascii_case("HEAD") || matches!(response.status, 204 | 304);
    http::write_message(
        socket.get_mut(),
        &format!("HTTP/1.1 {} Fluxa", response.status),
        &response.headers,
        &response.body,
        no_body,
    )
    .await?;
    socket.get_mut().shutdown().await?;
    Ok(())
}
