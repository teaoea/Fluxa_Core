//! UDP/TCP DNS 监听：显式网段授权、查询/连接/来源上限、有限报文和可等待的停止。
use super::Resolver;
use crate::error::{Error, Result};
use crate::network::Network;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Semaphore, watch},
    task::{JoinHandle, JoinSet},
};
static SERVICES: Semaphore = Semaphore::const_new(4);
static CONNECTIONS: Semaphore = Semaphore::const_new(16);
static REQUESTS: Semaphore = Semaphore::const_new(16);
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen: String,
    #[serde(default)]
    pub allow_lan: bool,
    #[serde(default)]
    pub allowed_clients: Vec<String>,
}
fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_unique_local() || ip.is_loopback() || ip.is_unicast_link_local(),
    }
}
fn mapped(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ip)),
        other => other,
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        self.build().map(|_| ())
    }
    fn build(&self) -> Result<(SocketAddr, Vec<Network>)> {
        if self.listen.len() > 128 || self.allowed_clients.len() > 32 {
            return Err(Error::InvalidInput);
        }
        let address: SocketAddr = self.listen.parse().map_err(|_| Error::InvalidInput)?;
        if address.ip().is_multicast() || matches!(address.ip(),IpAddr::V4(ip) if ip.is_broadcast())
        {
            return Err(Error::InvalidInput);
        }
        if self.allow_lan {
            if self.allowed_clients.is_empty()
                || (!address.ip().is_unspecified() && !private(mapped(address.ip())))
            {
                return Err(Error::InvalidInput);
            }
        } else if !address.ip().is_loopback() {
            return Err(Error::InvalidInput);
        }
        let allowed = self
            .allowed_clients
            .iter()
            .map(|s| {
                let network = Network::parse(s)?;
                if !private(network.first()) || !private(network.last()) {
                    return Err(Error::InvalidInput);
                }
                Ok(network)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((address, allowed))
    }
}
#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    pub running: bool,
    pub address: String,
    pub port: u16,
    pub queries: u64,
    pub rejected: u64,
}
#[derive(Default)]
struct Stats {
    running: AtomicBool,
    queries: AtomicU64,
    rejected: AtomicU64,
}
fn increment(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
        Some(n.saturating_add(1))
    });
}
struct Bucket {
    started: Instant,
    count: u32,
    last: Instant,
}
struct Access {
    config: Config,
    allowed: Vec<Network>,
    sources: Mutex<BTreeMap<IpAddr, Bucket>>,
    stats: Arc<Stats>,
}
impl Access {
    fn permitted(&self, ip: IpAddr) -> bool {
        let ip = mapped(ip);
        if !private(ip) || (!self.config.allow_lan && !ip.is_loopback()) {
            return false;
        }
        self.allowed.is_empty() || self.allowed.iter().any(|net| net.includes(ip))
    }
    fn allow(&self, ip: IpAddr) -> bool {
        if !self.permitted(ip) {
            increment(&self.stats.rejected);
            return false;
        }
        let Ok(mut sources) = self.sources.lock() else {
            crate::error::diagnostics::record(
                Error::Internal,
                crate::error::diagnostics::DiagnosticSite::DnsService,
            );
            return false;
        };
        let now = Instant::now();
        sources.retain(|_, b| now.duration_since(b.last) < Duration::from_secs(60));
        if !sources.contains_key(&ip) && sources.len() >= 128 {
            increment(&self.stats.rejected);
            return false;
        }
        let bucket = sources.entry(ip).or_insert(Bucket {
            started: now,
            count: 0,
            last: now,
        });
        bucket.last = now;
        if now.duration_since(bucket.started) >= Duration::from_secs(1) {
            bucket.started = now;
            bucket.count = 0;
        }
        if bucket.count >= 32 {
            increment(&self.stats.rejected);
            return false;
        }
        bucket.count += 1;
        true
    }
}
pub struct Service {
    address: SocketAddr,
    control: Arc<Control>,
}
/// 内核拥有的确定性关闭状态；外部 Service 仅提供受控停止意图和只读统计。
pub(crate) struct Control {
    cancel: watch::Sender<bool>,
    task: tokio::sync::Mutex<Option<JoinHandle<()>>>,
    stats: Arc<Stats>,
}
impl Control {
    pub(crate) fn running(&self) -> bool {
        self.stats.running.load(Ordering::Acquire)
    }
    pub(crate) async fn stop(&self) -> Result<()> {
        self.cancel.send_replace(true);
        let mut task = self.task.lock().await;
        // 不提前移走 JoinHandle；等待者取消不应让其他关闭调用误报完成。
        if let Some(worker) = task.as_mut() {
            worker.await.map_err(|_| Error::Internal)?;
        }
        task.take();
        Ok(())
    }
}
impl Service {
    pub async fn start(config: Config, resolver: Arc<Resolver>) -> Result<Self> {
        let _operation = resolver.operation()?;
        let (address, allowed) = config.build()?;
        let permit = SERVICES.try_acquire().map_err(|_| Error::ResourceLimit)?;
        let (tcp, udp) = bind_listeners(address).await?;
        let address = tcp.local_addr()?;
        let udp = Arc::new(udp);
        let stats = Arc::new(Stats::default());
        stats.running.store(true, Ordering::Release);
        let access = Arc::new(Access {
            config,
            allowed,
            sources: Mutex::new(BTreeMap::new()),
            stats: stats.clone(),
        });
        let (cancel, mut canceled) = watch::channel(false);
        let own_stats = stats.clone();
        let owner = resolver.clone();
        let task = crate::lifecycle::runtime()?.spawn(async move {
            // 配额由工作任务持有，析构清理完成前不释放监听实例名额。
            let _permit = permit;
            let mut children = JoinSet::new();
            let mut bytes = [0; 4097];
            loop {
                if *canceled.borrow() {
                    break;
                }
                while let Some(result) = children.try_join_next() {
                    crate::error::diagnostics::observe_join(
                        result,
                        crate::error::diagnostics::DiagnosticSite::DnsService,
                    );
                }
                tokio::select! {
                    _=canceled.changed()=>{break;}
                    received=udp.recv_from(&mut bytes)=>{
                        let (len,peer)=match received { Ok(value)=>value, Err(error)=>{crate::error::diagnostics::record(error.into(),crate::error::diagnostics::DiagnosticSite::DnsService);break;} };
                        if len>4096 || !access.allow(peer.ip()) {continue;}
                        let Ok(permit)=REQUESTS.try_acquire() else {crate::error::diagnostics::record(Error::ResourceLimit,crate::error::diagnostics::DiagnosticSite::DnsService);increment(&own_stats.rejected);continue;};
                        let packet=bytes[..len].to_vec();let udp=udp.clone();let resolver=resolver.clone();let stats=own_stats.clone();
                        children.spawn(async move {let _permit=permit;increment(&stats.queries);
                            match resolver.respond(&packet,true).await {
                                Ok(reply) => { if let Err(error) = udp.send_to(&reply,peer).await { crate::error::diagnostics::record(error.into(), crate::error::diagnostics::DiagnosticSite::DnsService); } },
                                Err(error) => { crate::error::diagnostics::record(error, crate::error::diagnostics::DiagnosticSite::DnsService); increment(&stats.rejected); }
                            }
                        });
                    }
                    accepted=tcp.accept()=>{
                        let (socket,peer)=match accepted {Ok(value)=>value,Err(error)=>{crate::error::diagnostics::record(error.into(),crate::error::diagnostics::DiagnosticSite::DnsService);break;} };
                        if !access.permitted(peer.ip()) {increment(&own_stats.rejected);continue;}
                        let Ok(permit)=CONNECTIONS.try_acquire() else {crate::error::diagnostics::record(Error::ResourceLimit,crate::error::diagnostics::DiagnosticSite::DnsService);increment(&own_stats.rejected);continue;};
                        let resolver=resolver.clone();let access=access.clone();
                        children.spawn(async move {let _permit=permit;let _=crate::error::diagnostics::observe(tcp_session(socket,peer,resolver,access).await,crate::error::diagnostics::DiagnosticSite::DnsService);});
                    }
                    joined=children.join_next(),if !children.is_empty()=>{if let Some(result)=joined {crate::error::diagnostics::observe_join(result,crate::error::diagnostics::DiagnosticSite::DnsService);}}
                }
            }
            children.abort_all();
            while let Some(result) = children.join_next().await {
                crate::error::diagnostics::observe_join(
                    result,
                    crate::error::diagnostics::DiagnosticSite::DnsService,
                );
            }
            own_stats.running.store(false, Ordering::Release);
        });
        let control = Arc::new(Control {
            cancel,
            task: tokio::sync::Mutex::new(Some(task)),
            stats,
        });
        // 真实双协议绑定成功后登记；关闭竞争导致登记失败时等待回滚完成。
        if let Err(error) = owner.register_service(control.clone()) {
            control.stop().await?;
            return Err(error);
        }
        Ok(Self { address, control })
    }
    pub fn status(&self) -> Status {
        Status {
            running: self.control.stats.running.load(Ordering::Acquire),
            address: self.address.to_string(),
            port: self.address.port(),
            queries: self.control.stats.queries.load(Ordering::Relaxed),
            rejected: self.control.stats.rejected.load(Ordering::Relaxed),
        }
    }
    pub async fn stop(&mut self) -> Result<()> {
        self.control.stop().await
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.control.cancel.send_replace(true);
    }
}
/// TCP 分配的临时端口可能被独立 UDP socket 占用；只在自动端口下有界重选。
async fn bind_listeners(address: SocketAddr) -> Result<(TcpListener, UdpSocket)> {
    for _ in 0..8 {
        let tcp = TcpListener::bind(address).await?;
        match UdpSocket::bind(tcp.local_addr()?).await {
            Ok(udp) => return Ok((tcp, udp)),
            Err(error) if address.port() == 0 && error.kind() == std::io::ErrorKind::AddrInUse => {
                drop(tcp);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(Error::Network)
}

async fn tcp_session(
    mut socket: TcpStream,
    peer: SocketAddr,
    resolver: Arc<Resolver>,
    access: Arc<Access>,
) -> Result<()> {
    // 空闲与持续请求都设置期限，防止缓慢客户端无限占用连接。
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let len = tokio::time::timeout(Duration::from_secs(15), socket.read_u16())
                .await
                .map_err(|_| Error::DnsTimeout)??;
            if !(12..=4096).contains(&len) || !access.allow(peer.ip()) {
                return Err(Error::InvalidInput);
            }
            let mut bytes = vec![0; usize::from(len)];
            tokio::time::timeout(Duration::from_secs(5), socket.read_exact(&mut bytes))
                .await
                .map_err(|_| Error::DnsTimeout)??;
            let _permit = REQUESTS.try_acquire().map_err(|_| Error::ResourceLimit)?;
            increment(&access.stats.queries);
            let reply = resolver.respond(&bytes, false).await?;
            tokio::time::timeout(Duration::from_secs(5), async {
                socket
                    .write_all(&(reply.len() as u16).to_be_bytes())
                    .await?;
                socket.write_all(&reply).await
            })
            .await
            .map_err(|_| Error::DnsTimeout)??;
        }
    })
    .await
    .map_err(|_| Error::DnsTimeout)?
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    #[tokio::test]
    async fn fixed_conflict_is_not_silently_moved_and_ephemeral_pair_agrees() {
        let occupied = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = occupied.local_addr().unwrap();
        assert!(bind_listeners(address).await.is_err());
        for _ in 0..100 {
            let (tcp, udp) = bind_listeners("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            assert_eq!(tcp.local_addr().unwrap(), udp.local_addr().unwrap());
        }
    }
}
