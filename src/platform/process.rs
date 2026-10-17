//! 进程归属只能来自操作系统；请求不能自报名称/PID，无法确定时返回 Unknown。
use crate::{
    error::{Error, Result},
    routing::Transport,
};
use std::net::SocketAddr;

#[derive(Clone, Copy)]
pub(crate) struct Flow {
    pub local: SocketAddr,
    pub remote: SocketAddr,
    pub transport: Transport,
}
pub(crate) struct Identity {
    pub name: Option<String>,
    pub token: Option<Token>,
}
#[derive(Clone)]
pub(crate) struct Token {
    #[cfg(target_os = "macos")]
    inner: macos::SocketOwner,
}
impl Token {
    /// 旧 PID/FD 不能在复用后继续充当原归属；不把引用计数视为占用权。
    pub(crate) fn validate(&self) -> Result<bool> {
        #[cfg(target_os = "macos")]
        {
            self.inner.validate()
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(Error::Unsupported)
        }
    }
}
/// 每个新流最多一次有界阻塞扫描；不阻塞 Tokio 工作线程，不缓存可复用的端口归属。
pub(crate) async fn identify(flow: Option<Flow>) -> Result<Identity> {
    #[cfg(target_os = "macos")]
    {
        let lease = macos::SCANS.reserve(macos::SCAN_BYTES)?;
        tokio::task::spawn_blocking(move || {
            let _lease = lease;
            macos::identify(flow)
        })
        .await
        .map_err(|_| Error::Internal)?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = flow;
        Err(Error::Unsupported)
    }
}
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use libproc::net_info::{InSockInfo, SocketFDInfo, SocketInfoKind};
    use std::{
        mem::{MaybeUninit, size_of},
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
        time::{Duration, Instant},
    };
    // PID 16 KiB + FD 16 KiB + 路径 4 KiB + 系统记录/栈余量，总共 64 KiB，最多两次并发扫描。
    pub(super) const SCAN_BYTES: usize = 64 * 1024;
    pub(super) static SCANS: crate::budget::ByteBudget =
        crate::budget::ByteBudget::new(2 * SCAN_BYTES);
    const MAX_PIDS: usize = 4096;
    const MAX_FDS_PER_PID: usize = 2048;
    const MAX_INSPECTED_FDS: usize = 65536;
    const MAX_PATH_BYTES: usize = 4096; // PROC_PIDPATHINFO_MAXSIZE。
    const SCAN_TIMEOUT: Duration = Duration::from_secs(1);
    #[derive(Clone)]
    pub(super) struct SocketOwner {
        pid: i32,
        fd: i32,
        started: (u64, u64),
        generation: u64,
        flow: Flow,
    }
    fn unknown() -> Identity {
        Identity {
            name: None,
            token: None,
        }
    }
    pub(super) fn unavailable(errno: Option<i32>) -> Result<()> {
        match errno {
            Some(libc::EPERM | libc::EACCES | libc::ESRCH | libc::EBADF | libc::ENOENT) => Ok(()),
            _ => Err(Error::Internal),
        }
    }
    fn missing() -> Result<()> {
        unavailable(std::io::Error::last_os_error().raw_os_error())
    }
    fn process_start(pid: i32) -> Result<Option<(u64, u64)>> {
        // SAFETY: libc 的 proc_bsdinfo 是 C 整数/字节数组组成的 POD，零值有效；独占缓冲区容量精确传入。
        let mut info = unsafe { MaybeUninit::<libc::proc_bsdinfo>::zeroed().assume_init() };
        // SAFETY: 指针仅在同步系统调用期间有效；内核写入不超过所声明大小，之后核对完整返回长度。
        let count = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size_of::<libc::proc_bsdinfo>() as i32,
            )
        };
        if count <= 0 {
            missing()?;
            return Ok(None);
        }
        if count as usize != size_of::<libc::proc_bsdinfo>() {
            return Err(Error::Internal);
        }
        Ok((info.pbi_pid == pid as u32).then_some((info.pbi_start_tvsec, info.pbi_start_tvusec)))
    }
    fn process_name(pid: i32) -> Result<Option<String>> {
        let mut path = [0u8; MAX_PATH_BYTES];
        // SAFETY: 固定可写字节数组独占且在调用期有效；传入实际容量，不借用系统返回的指针。
        let count = unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
        if count <= 0 {
            missing()?;
            return Ok(None);
        }
        if count as usize >= path.len() {
            return Err(Error::ResourceLimit);
        }
        let end = path.iter().position(|b| *b == 0).ok_or(Error::Internal)?;
        let Ok(path) = std::str::from_utf8(&path[..end]) else {
            return Ok(None);
        };
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str());
        match name {
            Some(name) if name.len() <= crate::routing::MAX_PROCESS_NAME_BYTES => {
                Ok(Some(name.to_owned()))
            }
            Some(_) => Err(Error::ResourceLimit),
            None => Ok(None),
        }
    }
    fn socket_info(pid: i32, fd: i32) -> Result<Option<SocketFDInfo>> {
        // SAFETY: 上游 repr(C) 系统类型仅含整数、C 数组与整数联合体，零字节有效；不解释其中的内核地址字段。
        let mut info = unsafe { MaybeUninit::<SocketFDInfo>::zeroed().assume_init() };
        // SAFETY: 类型来自固定版本 libproc 的系统封装；独占实际大小缓冲区，必须完整返回才允许读取。
        let count = unsafe {
            libc::proc_pidfdinfo(
                pid,
                fd,
                libproc::file_info::PIDFDInfoFlavor::SocketInfo as i32,
                (&mut info as *mut SocketFDInfo).cast(),
                size_of::<SocketFDInfo>() as i32,
            )
        };
        if count <= 0 {
            missing()?;
            return Ok(None);
        }
        if count as usize != size_of::<SocketFDInfo>() {
            return Err(Error::Internal);
        }
        Ok(Some(info))
    }
    fn internet(info: &SocketFDInfo, transport: Transport) -> Option<InSockInfo> {
        if info.psi.soi_protocol
            != match transport {
                Transport::Tcp => libc::IPPROTO_TCP,
                Transport::Udp => libc::IPPROTO_UDP,
            }
        {
            return None;
        }
        match (SocketInfoKind::from(info.psi.soi_kind), transport) {
            // SAFETY: 完整内核记录的判别字段确认了联合体活动类型；仅复制整数/POD，不解引用任何内核指针。
            (SocketInfoKind::Tcp, Transport::Tcp) => {
                // SAFETY: 上方已核对 SocketInfoKind::Tcp 与 TCP 协议，读取对应 POD 联合体成员。
                Some(unsafe { info.psi.soi_proto.pri_tcp.tcpsi_ini })
            }
            // SAFETY: 同上；UDP 的 In 类型与固定系统协议编号同时匹配后才读取对应成员。
            (SocketInfoKind::In, Transport::Udp) => Some(unsafe { info.psi.soi_proto.pri_in }),
            _ => None,
        }
    }
    fn address(info: &InSockInfo, family: i32, local: bool) -> Option<IpAddr> {
        let value = if local {
            info.insi_laddr
        } else {
            info.insi_faddr
        };
        if family == libc::AF_INET {
            // SAFETY: 完整内核 socket 记录的 family 已确认为 IPv4，读取对应地址联合体成员；不读填充字节。
            Some(IpAddr::V4(Ipv4Addr::from(
                unsafe { value.ina_46.i46a_addr4.s_addr }.to_ne_bytes(),
            )))
        } else if family == libc::AF_INET6 {
            // SAFETY: family 为 IPv6，成员为完整 16 字节系统地址；仅复制固定数组。
            Some(Ipv6Addr::from(unsafe { value.ina_6.s6_addr }).to_canonical())
        } else {
            None
        }
    }
    fn matches(info: &SocketFDInfo, flow: Flow) -> Option<u64> {
        let internet = internet(info, flow.transport)?;
        let local = address(&internet, info.psi.soi_family, true)?;
        let remote = address(&internet, info.psi.soi_family, false)?;
        let lport = u16::from_be(internet.insi_lport as u16);
        let rport = u16::from_be(internet.insi_fport as u16);
        if lport != flow.local.port()
            || (!local.is_unspecified() && local != flow.local.ip().to_canonical())
        {
            return None;
        }
        if (remote != flow.remote.ip().to_canonical() || rport != flow.remote.port())
            && (flow.transport != Transport::Udp || !remote.is_unspecified() || rport != 0)
        {
            return None;
        }
        Some(internet.insi_gencnt)
    }
    impl SocketOwner {
        pub(super) fn validate(&self) -> Result<bool> {
            if process_start(self.pid)? != Some(self.started) {
                return Ok(false);
            }
            Ok(socket_info(self.pid, self.fd)?
                .as_ref()
                .and_then(|info| matches(info, self.flow))
                == Some(self.generation))
        }
    }
    pub(super) fn identify(flow: Option<Flow>) -> Result<Identity> {
        let Some(flow) = flow else {
            let pid = std::process::id() as i32;
            let started = process_start(pid)?;
            let name = process_name(pid)?;
            return Ok(if started.is_some() && started == process_start(pid)? {
                Identity { name, token: None }
            } else {
                unknown()
            });
        };
        if flow.local.port() == 0
            || flow.remote.port() == 0
            || flow.local.is_ipv4() != flow.remote.is_ipv4()
        {
            return Err(Error::InvalidInput);
        }
        let mut pids = [0i32; MAX_PIDS];
        // SAFETY: 连续固定 i32 数组独占可写，传入真实字节容量，不信任返回的元素数量。
        let count =
            unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size_of_val(&pids) as i32) };
        if count <= 0 {
            missing()?;
            return Ok(unknown());
        }
        if count as usize >= pids.len() {
            return Err(Error::ResourceLimit);
        }
        let deadline = Instant::now() + SCAN_TIMEOUT;
        let mut inspected = 0;
        let mut found: Option<Identity> = None;
        for pid in pids[..count as usize]
            .iter()
            .copied()
            .filter(|pid| *pid > 0)
        {
            if Instant::now() >= deadline {
                return Err(Error::ResourceLimit);
            }
            let Some(started) = process_start(pid)? else {
                continue;
            };
            let mut fds = [libc::proc_fdinfo {
                proc_fd: 0,
                proc_fdtype: 0,
            }; MAX_FDS_PER_PID];
            // SAFETY: __error 返回当前线程有效 errno 槽；同步清零后立即调用，区分空 FD 表与返回 0 的系统错误。
            unsafe {
                *libc::__error() = 0;
            }
            // SAFETY: 固定系统 FD 数组容量真实，独占调用；返回长度必须整除记录宽度且不超容量。
            let length = unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDLISTFDS,
                    0,
                    fds.as_mut_ptr().cast(),
                    size_of_val(&fds) as i32,
                )
            };
            if length <= 0 {
                if length < 0 || std::io::Error::last_os_error().raw_os_error() != Some(0) {
                    missing()?;
                }
                continue;
            }
            if length as usize >= size_of_val(&fds) {
                return Err(Error::ResourceLimit);
            }
            if !(length as usize).is_multiple_of(size_of::<libc::proc_fdinfo>()) {
                return Err(Error::Internal);
            }
            for fd in &fds[..length as usize / size_of::<libc::proc_fdinfo>()] {
                inspected += 1;
                if inspected > MAX_INSPECTED_FDS || Instant::now() >= deadline {
                    return Err(Error::ResourceLimit);
                }
                if fd.proc_fdtype != libc::PROX_FDTYPE_SOCKET as u32 {
                    continue;
                }
                let Some(info) = socket_info(pid, fd.proc_fd)? else {
                    continue;
                };
                let Some(generation) = matches(&info, flow) else {
                    continue;
                };
                let Some(name) = process_name(pid)? else {
                    return Ok(unknown());
                };
                let owner = SocketOwner {
                    pid,
                    fd: fd.proc_fd,
                    started,
                    generation,
                    flow,
                };
                if !owner.validate()? {
                    return Ok(unknown());
                }
                // 多个系统对象声明同一元组时不可猜测实际发送者，包括共享/继承 FD。
                if found.is_some() {
                    return Ok(unknown());
                }
                found = Some(Identity {
                    name: Some(name),
                    token: Some(Token { inner: owner }),
                });
            }
        }
        Ok(found.unwrap_or_else(unknown))
    }
}
#[cfg(test)]
#[path = "../../tests/test_platform_process.rs"]
mod test_platform_process;
