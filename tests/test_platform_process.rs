//! 测试文件：tests/test_platform_process.rs
//! 测试类型：单元测试、真实系统身份与生命周期
//! 测试对象：src/platform/process.rs
//! 测试内容：
//! - unexpected_system_errors_do_not_become_unknown：仅明确的不可识别状态允许跳过，其他系统失败不能静默放行。
//! - own_process_name_is_os_executable：名称来自操作系统可执行路径。
//! - tcp_owner_rejects_closed_socket_identity：真实 TCP 四元组归属及关闭后旧身份无效。
//! - udp_owner_is_exact_and_not_a_claimed_name：真实 UDP 元组、未知元组和代次失效。
//!
//! 测试依据：process-name 精确/大小写敏感、未知跳过；身份不可由请求伪造，Socket/PID 复用不延长原归属。
//! 前置条件：macOS 系统进程查询及本机回环套接字权限，不需要管理员、不接管 VPN。
use super::*;
#[cfg(target_os = "macos")]
#[tokio::test]
async fn own_process_name_is_os_executable() {
    let identity = identify(None).await.unwrap();
    assert_eq!(
        identity.name.as_deref(),
        std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
    );
    assert!(identity.token.is_none());
}
#[cfg(target_os = "macos")]
#[tokio::test]
async fn tcp_owner_rejects_closed_socket_identity() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (peer, _) = listener.accept().await.unwrap();
    let flow = Flow {
        local: client.local_addr().unwrap(),
        remote: client.peer_addr().unwrap(),
        transport: Transport::Tcp,
    };
    let identity = identify(Some(flow)).await.unwrap();
    assert_eq!(
        identity.name.as_deref(),
        std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
    );
    let token = identity.token.unwrap();
    assert!(token.validate().unwrap());
    drop(client);
    assert!(!token.validate().unwrap());
    assert!(identify(Some(flow)).await.unwrap().name.is_none());
    drop(peer);
}
#[cfg(target_os = "macos")]
#[tokio::test]
async fn udp_owner_is_exact_and_not_a_claimed_name() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    socket.connect(peer.local_addr().unwrap()).await.unwrap();
    let flow = Flow {
        local: socket.local_addr().unwrap(),
        remote: peer.local_addr().unwrap(),
        transport: Transport::Udp,
    };
    let identity = identify(Some(flow)).await.unwrap();
    assert_eq!(
        identity.name.as_deref(),
        std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
    );
    let token = identity.token.unwrap();
    assert!(token.validate().unwrap());
    drop(socket);
    assert!(!token.validate().unwrap());
    assert!(identify(Some(flow)).await.unwrap().name.is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn unexpected_system_errors_do_not_become_unknown() {
    for errno in [
        libc::EPERM,
        libc::EACCES,
        libc::ESRCH,
        libc::EBADF,
        libc::ENOENT,
    ] {
        assert!(macos::unavailable(Some(errno)).is_ok());
    }
    for errno in [None, Some(0), Some(libc::EINVAL), Some(libc::ENOMEM)] {
        assert!(matches!(macos::unavailable(errno), Err(Error::Internal)));
    }
}
