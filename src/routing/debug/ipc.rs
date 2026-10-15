//! 独立 Unix IPC：系统对端身份、签名凭证、逐操作权限、防重放和内核内存会话。
use super::{DebugRule, Session, Status};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, ErrorInfo, Result},
    outbound::Manager,
    platform::debug_identity::{self, Identity},
    routing::{Rule, Transport},
};
use ring::{
    rand::{SecureRandom, SystemRandom},
    signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::watch,
    task::JoinHandle,
};
use zeroize::Zeroizing;
const VERSION: u32 = 1;
const MAX_FRAME: usize = 64 * 1024;
const MAX_CONNECTIONS: usize = 8;
const MAX_AUDIT: usize = 256;
const CONNECTION_BYTES: usize = 512 * 1024;
const MAX_POLICY_BYTES: usize = 32 * 1024;
static BUDGET: ByteBudget = ByteBudget::new(8 * CONNECTION_BYTES);
const MAX_SERVICES: usize = 4;
const SERVICE_BYTES: usize = 512 * 1024;
static SERVERS: ByteBudget = ByteBudget::new(MAX_SERVICES);
static SERVICE_MEMORY: ByteBudget = ByteBudget::new(MAX_SERVICES * SERVICE_BYTES);
/// 权限独立登记，不将整数编号或客户端声明作为授权结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Replace,
    Clear,
    Status,
    Close,
    Audit,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Principal {
    uid: u32,
    requirement: String,
    public_key: [u8; 32],
    permissions: Vec<Permission>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    version: u32,
    principals: Vec<Principal>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireRule {
    pub rule: Rule,
    pub protocol: Option<Transport>,
    pub lifetime_seconds: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Replace {
        rules: Vec<WireRule>,
        outbound: Option<String>,
    },
    Clear {},
    Status {},
    Close {},
    Audit {
        maximum: usize,
    },
}
impl Request {
    fn permission(&self) -> Permission {
        match self {
            Self::Replace { .. } => Permission::Replace,
            Self::Clear {} => Permission::Clear,
            Self::Status {} => Permission::Status,
            Self::Close {} => Permission::Close,
            Self::Audit { .. } => Permission::Audit,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hello {
    version: u32,
    session: u64,
    challenge: [u8; 32],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Proof {
    version: u32,
    public_key: [u8; 32],
    lifetime_seconds: u64,
    signature: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    sequence: u64,
    payload: Vec<u8>,
    signature: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub uid: Option<u32>,
    pub pid: Option<u32>,
    pub permission: Option<Permission>,
    pub code: Option<u16>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub success: bool,
    pub error_info: Option<ErrorInfo>,
    pub status: Option<Status>,
    pub audit: Option<Vec<AuditEntry>>,
}
impl Reply {
    fn from_result(result: Result<Option<Status>>) -> Self {
        match result {
            Ok(status) => Self {
                success: true,
                error_info: None,
                status,
                audit: None,
            },
            Err(error) => Self {
                success: false,
                error_info: Some(error.public_info()),
                status: None,
                audit: None,
            },
        }
    }
}
struct Control {
    manager: Arc<Manager>,
    owner: Mutex<Option<u64>>,
    audit: Mutex<VecDeque<AuditEntry>>,
}
impl Control {
    fn record(
        &self,
        peer: Option<&Identity>,
        permission: Option<Permission>,
        code: Option<u16>,
    ) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Internal)?
            .as_secs();
        let mut audit = self.audit.lock().map_err(|_| Error::Internal)?;
        if audit.len() == MAX_AUDIT {
            audit.pop_front();
        }
        audit.push_back(AuditEntry {
            timestamp,
            uid: peer.map(|p| p.uid),
            pid: peer.map(|p| p.pid),
            permission,
            code,
        });
        Ok(())
    }
    fn release(&self, id: u64, session: &Session) -> Result<()> {
        session.close()?;
        let mut owner = self.owner.lock().map_err(|_| Error::Internal)?;
        if *owner == Some(id) {
            self.manager.unbind_debug(session.identity())?;
            *owner = None;
        }
        Ok(())
    }
    fn dispatch(&self, id: u64, session: &Arc<Session>, request: Request) -> Result<Reply> {
        let mut owner = self.owner.lock().map_err(|_| Error::Internal)?;
        let result = match request {
            Request::Replace { rules, outbound } => {
                if owner.is_some_and(|active| active != id) {
                    return Err(Error::DebugUnauthorized);
                }
                if rules.len() > super::MAX_DEBUG_RULES {
                    return Err(Error::ResourceLimit);
                }
                self.manager.replace_debug(
                    session.clone(),
                    rules
                        .into_iter()
                        .map(|row| DebugRule {
                            rule: row.rule,
                            protocol: row.protocol,
                            lifetime_seconds: row.lifetime_seconds,
                        })
                        .collect(),
                    outbound,
                )?;
                *owner = Some(id);
                Ok(Some(session.status()?))
            }
            Request::Clear {} => {
                if owner.is_some_and(|active| active != id) {
                    return Err(Error::DebugUnauthorized);
                }
                session.clear()?;
                self.manager.unbind_debug(session.identity())?;
                *owner = None;
                Ok(Some(session.status()?))
            }
            Request::Status {} => Ok(Some(session.status()?)),
            Request::Close {} => {
                if owner.is_some_and(|active| active != id) {
                    return Err(Error::DebugUnauthorized);
                }
                session.close()?;
                self.manager.unbind_debug(session.identity())?;
                *owner = None;
                Ok(None)
            }
            Request::Audit { maximum } => {
                if !(1..=128).contains(&maximum) {
                    return Err(Error::InvalidInput);
                }
                let audit = self.audit.lock().map_err(|_| Error::Internal)?;
                return Ok(Reply {
                    success: true,
                    error_info: None,
                    status: None,
                    audit: Some(audit.iter().rev().take(maximum).cloned().collect()),
                });
            }
        };
        Ok(Reply::from_result(result))
    }
}
struct Grant {
    id: u64,
    session: Arc<Session>,
    control: Arc<Control>,
}
impl Drop for Grant {
    fn drop(&mut self) {
        let _ = crate::error::diagnostics::observe(
            self.control.release(self.id, &self.session),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
}
/// IPC 实际监听与授权会话由内核托管；外部仅发送启动/关闭意图。
pub struct Service {
    cancel: watch::Sender<bool>,
    task: Option<JoinHandle<Result<()>>>,
    completion: Option<Result<()>>,
}
impl Service {
    pub async fn start(
        policy_file: &Path,
        directory: &Path,
        manager: Arc<Manager>,
    ) -> Result<Self> {
        // 预算先于文件读取和反序列化，不能用大量并发启动绕过中间分配限制。
        let memory = SERVICE_MEMORY.reserve(SERVICE_BYTES)?;
        let bytes = crate::platform::debug_identity::read_policy(policy_file, MAX_POLICY_BYTES)?;
        let policy: Policy = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidInput)?;
        Self::start_reserved(policy, directory, manager, memory).await
    }
    #[cfg(test)]
    async fn start_policy(policy: Policy, directory: &Path, manager: Arc<Manager>) -> Result<Self> {
        Self::start_reserved(
            policy,
            directory,
            manager,
            SERVICE_MEMORY.reserve(SERVICE_BYTES)?,
        )
        .await
    }
    async fn start_reserved(
        policy: Policy,
        directory: &Path,
        manager: Arc<Manager>,
        memory: ByteLease,
    ) -> Result<Self> {
        validate_policy(&policy)?;
        crate::platform::debug_identity::private_directory(directory)?;
        let lease = SERVERS.reserve(1)?;
        let path = directory.join("debug.sock");
        // 不删除已存在端点，防止接管另一个服务或通过符号链接篡改文件。
        let listener = std::os::unix::net::UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        let runtime = crate::lifecycle::runtime()?;
        // 实际监听的 I/O 注册属于内核运行时，不能依赖调用方的异步执行器继续驱动。
        let listener = {
            let _kernel = runtime.enter();
            UnixListener::from_std(listener)?
        };
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
        let (cancel, mut canceled) = watch::channel(false);
        let control = Arc::new(Control {
            manager,
            owner: Mutex::new(None),
            audit: Mutex::new(VecDeque::with_capacity(MAX_AUDIT)),
        });
        let runtime = crate::lifecycle::runtime()?;
        let handle = runtime.handle().clone();
        let policy = Arc::new(policy);
        let task=runtime.spawn(async move {
   let _lease=lease;
   let _memory=memory;
   let mut children=tokio::task::JoinSet::new();
   loop {tokio::select! {biased; _=canceled.changed()=>break,
    Some(joined)=children.join_next(),if !children.is_empty()=>{crate::error::diagnostics::observe_join(joined,crate::error::diagnostics::DiagnosticSite::Cleanup);},
    accepted=listener.accept()=>{let (stream,_)=match accepted {Ok(v)=>v,Err(error)=>{crate::error::diagnostics::record(error.into(),crate::error::diagnostics::DiagnosticSite::Cleanup);break;}};
     if children.len()>=MAX_CONNECTIONS {drop(stream);continue;}
     let control=control.clone();let policy=policy.clone();
     children.spawn_on(async move {let result=serve(stream,policy,control).await;let _=crate::error::diagnostics::observe(result,crate::error::diagnostics::DiagnosticSite::Cleanup);},&handle);
    }
   }}
   children.abort_all();while let Some(joined)=children.join_next().await {crate::error::diagnostics::observe_join(joined,crate::error::diagnostics::DiagnosticSite::Cleanup);}
   control.manager.close_revoked_debug().await?;
   drop(listener);
   std::fs::remove_file(&path)?;
   Ok(())
  });
        Ok(Self {
            cancel,
            task: Some(task),
            completion: None,
        })
    }
    pub async fn close(&mut self) -> Result<()> {
        if let Some(result) = self.completion {
            return result;
        }
        self.cancel.send_replace(true);
        let result = if let Some(task) = self.task.as_mut() {
            task.await.unwrap_or(Err(Error::Internal))
        } else {
            Err(Error::Internal)
        };
        self.task.take();
        self.completion = Some(result);
        result
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}
fn validate_policy(policy: &Policy) -> Result<()> {
    if policy.version != VERSION || !(1..=16).contains(&policy.principals.len()) {
        return Err(Error::InvalidInput);
    }
    for (index, p) in policy.principals.iter().enumerate() {
        debug_identity::validate_requirement(&p.requirement)?;
        if policy.principals[..index]
            .iter()
            .any(|other| other.uid == p.uid && other.public_key == p.public_key)
        {
            return Err(Error::InvalidInput);
        }
        if p.requirement.is_empty()
            || p.requirement.len() > 1024
            || p.permissions.is_empty()
            || p.permissions.len() > 5
            || p.permissions
                .iter()
                .enumerate()
                .any(|(i, v)| p.permissions[..i].contains(v))
        {
            return Err(Error::InvalidInput);
        }
    }
    Ok(())
}
fn transcript(hello: &Hello, sequence: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64 + payload.len());
    bytes.extend_from_slice(b"FLUXA_DEBUG_IPC_V1");
    bytes.extend_from_slice(&hello.session.to_le_bytes());
    bytes.extend_from_slice(&hello.challenge);
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}
async fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>> {
    let size = stream.read_u32().await? as usize;
    if size == 0 || size > MAX_FRAME {
        return Err(Error::ResourceLimit);
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes).await?;
    Ok(bytes)
}
async fn write_frame<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Internal)?;
    if bytes.len() > MAX_FRAME {
        return Err(Error::ResourceLimit);
    }
    stream.write_u32(bytes.len() as u32).await?;
    stream.write_all(&bytes).await?;
    Ok(())
}
fn verify_signature(
    key: &[u8; 32],
    hello: &Hello,
    sequence: u64,
    payload: &[u8],
    signature: &[u8],
) -> Result<()> {
    if signature.len() != 64 {
        return Err(Error::DebugAuthenticationFailed);
    }
    UnparsedPublicKey::new(&ED25519, key)
        .verify(&transcript(hello, sequence, payload), signature)
        .map_err(|_| Error::DebugAuthenticationFailed)
}
async fn serve(mut stream: UnixStream, policy: Arc<Policy>, control: Arc<Control>) -> Result<()> {
    let result = serve_authenticated(&mut stream, &policy, &control).await;
    if let Err(error) = result {
        let peer = debug_identity::inspect(&stream).ok();
        control.record(peer.as_ref(), None, Some(error.code().value()))?;
        control.manager.close_revoked_debug().await?;
        // 错误仍是错误，返回固定受限的公开信息，不输出签名、规则原文或底层错误链。
        let reply = Reply::from_result(Err(error));
        let sent =
            tokio::time::timeout(Duration::from_secs(1), write_frame(&mut stream, &reply)).await;
        if let Ok(Err(error)) = sent {
            crate::error::diagnostics::record(
                error,
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
    }
    result
}
async fn serve_authenticated(
    stream: &mut UnixStream,
    policy: &Policy,
    control: &Arc<Control>,
) -> Result<()> {
    let _budget = BUDGET.reserve(CONNECTION_BYTES)?;
    let peer = match debug_identity::inspect(stream) {
        Ok(peer) => peer,
        Err(error) => {
            control.record(None, None, Some(error.code().value()))?;
            return Err(error);
        }
    };
    let allowed: Vec<_> = policy
        .principals
        .iter()
        .filter(|p| p.uid == peer.uid && debug_identity::verify(&peer, &p.requirement).is_ok())
        .collect();
    if allowed.is_empty() {
        control.record(
            Some(&peer),
            None,
            Some(Error::DebugIdentityRejected.code().value()),
        )?;
        return Err(Error::DebugIdentityRejected);
    }
    let mut challenge = [0; 32];
    SystemRandom::new()
        .fill(&mut challenge)
        .map_err(|_| Error::Internal)?;
    let hello = Hello {
        version: VERSION,
        session: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
        challenge,
    };
    write_frame(stream, &hello).await?;
    let proof_bytes = tokio::time::timeout(Duration::from_secs(3), read_frame(stream))
        .await
        .map_err(|_| Error::DebugAuthenticationFailed)??;
    let proof: Proof = serde_json::from_slice(&proof_bytes).map_err(|_| Error::InvalidInput)?;
    #[cfg(test)]
    eprintln!(
        "凭证证明字段：版本 {}，期限 {}，签名长度 {}",
        proof.version,
        proof.lifetime_seconds,
        proof.signature.len()
    );
    let principal = allowed
        .into_iter()
        .find(|p| p.public_key == proof.public_key)
        .ok_or(Error::DebugAuthenticationFailed)?;
    if proof.version != VERSION
        || !(1..=super::MAX_LIFETIME_SECONDS).contains(&proof.lifetime_seconds)
    {
        return Err(Error::DebugAuthenticationFailed);
    }
    #[cfg(test)]
    eprintln!("凭证主体已匹配，验证会话签名");
    verify_signature(
        &principal.public_key,
        &hello,
        0,
        &proof.lifetime_seconds.to_le_bytes(),
        &proof.signature,
    )?;
    let expires = Instant::now()
        .checked_add(Duration::from_secs(proof.lifetime_seconds))
        .ok_or(Error::ResourceLimit)?;
    let grant = Grant {
        id: hello.session,
        session: Arc::new(Session::new()?),
        control: control.clone(),
    };
    control.record(Some(&peer), None, None)?;
    write_frame(stream, &Reply::from_result(Ok(None))).await?;
    let mut sequence = 1u64;
    loop {
        let bytes =
            tokio::time::timeout_at(tokio::time::Instant::from_std(expires), read_frame(stream))
                .await
                .map_err(|_| Error::DebugAuthenticationFailed)??;
        if debug_identity::inspect(stream)? != peer {
            return Err(Error::DebugIdentityRejected);
        }
        debug_identity::verify(&peer, &principal.requirement)?;
        let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidInput)?;
        if envelope.sequence != sequence || envelope.payload.len() > MAX_FRAME {
            return Err(Error::DebugAuthenticationFailed);
        }
        verify_signature(
            &principal.public_key,
            &hello,
            sequence,
            &envelope.payload,
            &envelope.signature,
        )?;
        sequence = sequence.checked_add(1).ok_or(Error::ResourceLimit)?;
        let request: Request =
            serde_json::from_slice(&envelope.payload).map_err(|_| Error::InvalidInput)?;
        // 规则授权不能超过当前 IPC grant；关闭/到期无须依赖调用方释放引用。
        if principal.permissions.contains(&Permission::Replace)
            && let Request::Replace { rules, .. } = &request
        {
            let remaining = expires.saturating_duration_since(Instant::now()).as_secs();
            if rules.iter().any(|rule| {
                rule.lifetime_seconds.is_some_and(|ttl| {
                    (1..=super::MAX_LIFETIME_SECONDS).contains(&ttl) && ttl > remaining
                })
            }) {
                return Err(Error::DebugAuthenticationFailed);
            }
        }
        let operation = request.permission();
        let closing = matches!(request, Request::Close {});
        let result = if principal.permissions.contains(&operation) {
            control.dispatch(grant.id, &grant.session, request)
        } else {
            Err(Error::DebugUnauthorized)
        };
        let reply = result.unwrap_or_else(|error| Reply::from_result(Err(error)));
        if reply.success {
            control.manager.close_revoked_debug().await?;
        }
        control.record(
            Some(&peer),
            Some(operation),
            reply.error_info.as_ref().map(|e| e.code.value()),
        )?;
        tokio::time::timeout_at(
            tokio::time::Instant::from_std(expires),
            write_frame(stream, &reply),
        )
        .await
        .map_err(|_| Error::DebugAuthenticationFailed)??;
        if closing && reply.success {
            break;
        }
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Greeting {
    Hello(Hello),
    Denied(Reply),
}
fn validate_reply(reply: &Reply) -> Result<()> {
    if (reply.success && reply.error_info.is_some())
        || (!reply.success
            && (reply.error_info.is_none() || reply.status.is_some() || reply.audit.is_some()))
    {
        return Err(Error::DebugAuthenticationFailed);
    }
    Ok(())
}
fn handshake_failure(info: Option<ErrorInfo>) -> Error {
    use crate::error::ErrorCode;
    // 明确白名单限定认证阶段的合法失败；其他业务响应不构成有效认证协议。
    match info.map(|info| info.code) {
        Some(ErrorCode::DebugIdentityRejected) => Error::DebugIdentityRejected,
        Some(ErrorCode::DebugAuthenticationFailed) => Error::DebugAuthenticationFailed,
        Some(ErrorCode::DebugSessionClosed) => Error::DebugSessionClosed,
        Some(ErrorCode::ResourceLimit) => Error::ResourceLimit,
        Some(ErrorCode::InvalidInput) => Error::InvalidInput,
        Some(ErrorCode::Unsupported) => Error::Unsupported,
        Some(ErrorCode::Internal) => Error::Internal,
        Some(ErrorCode::Network) => Error::Network,
        Some(ErrorCode::NetworkTimeout) => Error::NetworkTimeout,
        _ => Error::DebugAuthenticationFailed,
    }
}
/// Rust/C ABI 客户端只封装凭证证明和 IPC；不持有内核规则/快照/资源管理权。
pub struct Client {
    stream: UnixStream,
    hello: Hello,
    key: Ed25519KeyPair,
    sequence: u64,
    failed: bool,
    _seed: Zeroizing<Vec<u8>>,
    _lease: ByteLease,
}
impl Client {
    pub async fn connect(
        endpoint: &Path,
        credential: &Path,
        server_uid: u32,
        server_requirement: &str,
        lifetime_seconds: u64,
    ) -> Result<Self> {
        let lease = BUDGET.reserve(CONNECTION_BYTES)?;
        let seed = crate::platform::debug_identity::read_credential(credential)?;
        let key = Ed25519KeyPair::from_seed_unchecked(&seed)
            .map_err(|_| Error::DebugAuthenticationFailed)?;
        let mut stream =
            tokio::time::timeout(Duration::from_secs(3), UnixStream::connect(endpoint))
                .await
                .map_err(|_| Error::NetworkTimeout)??;
        let peer = debug_identity::inspect(&stream)?;
        if peer.uid != server_uid {
            return Err(Error::DebugIdentityRejected);
        }
        debug_identity::verify(&peer, server_requirement)?;
        let greeting = tokio::time::timeout(Duration::from_secs(3), read_frame(&mut stream))
            .await
            .map_err(|_| Error::DebugAuthenticationFailed)??;
        // 认证前也允许受控拒绝响应；不能把真实预算/身份失败误报为 JSON 错误。
        let hello =
            match serde_json::from_slice::<Greeting>(&greeting).map_err(|_| Error::InvalidInput)? {
                Greeting::Hello(hello) => hello,
                Greeting::Denied(reply) => {
                    validate_reply(&reply)?;
                    return Err(handshake_failure(reply.error_info));
                }
            };
        if hello.version != VERSION
            || hello.session == 0
            || !(1..=super::MAX_LIFETIME_SECONDS).contains(&lifetime_seconds)
        {
            return Err(Error::DebugAuthenticationFailed);
        }
        let public_key: key_array::PublicKey = key
            .public_key()
            .as_ref()
            .try_into()
            .map_err(|_| Error::Internal)?;
        let signature = key
            .sign(&transcript(&hello, 0, &lifetime_seconds.to_le_bytes()))
            .as_ref()
            .to_vec();
        write_frame(
            &mut stream,
            &Proof {
                version: VERSION,
                public_key,
                lifetime_seconds,
                signature,
            },
        )
        .await?;
        let reply: Reply = serde_json::from_slice(
            &tokio::time::timeout(Duration::from_secs(3), read_frame(&mut stream))
                .await
                .map_err(|_| Error::DebugAuthenticationFailed)??,
        )
        .map_err(|_| Error::InvalidInput)?;
        validate_reply(&reply)?;
        if !reply.success {
            return Err(handshake_failure(reply.error_info));
        }
        Ok(Self {
            stream,
            hello,
            key,
            sequence: 1,
            failed: false,
            _seed: seed,
            _lease: lease,
        })
    }
    pub async fn command(&mut self, request: &Request) -> Result<Reply> {
        if self.failed {
            return Err(Error::DebugSessionClosed);
        }
        let result = tokio::time::timeout(Duration::from_secs(3), self.command_inner(request))
            .await
            .map_err(|_| Error::NetworkTimeout)
            .and_then(|result| result);
        let terminal = result.as_ref().is_ok_and(|reply| {
            (reply.success && matches!(request, Request::Close {}))
                || reply.error_info.as_ref().is_some_and(|info| {
                    matches!(
                        info.code,
                        crate::error::ErrorCode::DebugAuthenticationFailed
                            | crate::error::ErrorCode::DebugIdentityRejected
                    )
                })
        });
        if result.is_err() || terminal {
            self.failed = true;
            let shutdown = self.stream.shutdown().await;
            if let Err(error) = shutdown {
                crate::error::diagnostics::record(
                    error.into(),
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            }
        }
        result
    }
    async fn command_inner(&mut self, request: &Request) -> Result<Reply> {
        let payload = serde_json::to_vec(request).map_err(|_| Error::InvalidInput)?;
        if payload.len() > MAX_FRAME / 2 {
            return Err(Error::ResourceLimit);
        }
        let sequence = self.sequence;
        self.sequence = sequence.checked_add(1).ok_or(Error::ResourceLimit)?;
        let signature = self
            .key
            .sign(&transcript(&self.hello, sequence, &payload))
            .as_ref()
            .to_vec();
        write_frame(
            &mut self.stream,
            &Envelope {
                sequence,
                payload,
                signature,
            },
        )
        .await?;
        serde_json::from_slice(&read_frame(&mut self.stream).await?)
            .map_err(|_| Error::InvalidInput)
    }
}
mod key_array {
    pub(super) type PublicKey = [u8; 32];
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handshake_preserves_real_failure_and_never_turns_error_into_success() {
        for error in [
            Error::ResourceLimit,
            Error::DebugIdentityRejected,
            Error::DebugAuthenticationFailed,
            Error::Internal,
            Error::InvalidInput,
        ] {
            let reply = Reply::from_result(Err(error));
            validate_reply(&reply).unwrap();
            assert_eq!(handshake_failure(reply.error_info), error);
        }
        let mut forged = Reply::from_result(Err(Error::DebugUnauthorized));
        forged.success = true;
        assert_eq!(
            validate_reply(&forged),
            Err(Error::DebugAuthenticationFailed)
        );
        assert_eq!(handshake_failure(None), Error::DebugAuthenticationFailed);
    }
    #[test]
    fn signature_is_bound_to_session_sequence_payload_and_deadline() {
        let key = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let public_key = key.public_key().as_ref().try_into().unwrap();
        let hello = Hello {
            version: VERSION,
            session: 17,
            challenge: [19; 32],
        };
        let payload = b"{\"operation\":\"status\"}";
        let signature = key.sign(&transcript(&hello, 1, payload));
        assert!(verify_signature(&public_key, &hello, 1, payload, signature.as_ref()).is_ok());
        for (sequence, body) in [
            (2, payload.as_slice()),
            (1, b"{\"operation\":\"clear\"}".as_slice()),
        ] {
            assert_eq!(
                verify_signature(&public_key, &hello, sequence, body, signature.as_ref()),
                Err(Error::DebugAuthenticationFailed)
            );
        }
        let other = Hello {
            session: 18,
            ..hello
        };
        assert_eq!(
            verify_signature(&public_key, &other, 1, payload, signature.as_ref()),
            Err(Error::DebugAuthenticationFailed)
        );
        assert_eq!(
            verify_signature(&[0; 32], &hello, 1, payload, signature.as_ref()),
            Err(Error::DebugAuthenticationFailed)
        );
    }
    #[test]
    fn control_fields_are_closed_and_permissions_are_separate() {
        for input in [
            r#"{"operation":"status","authorized":true}"#,
            r#"{"operation":"clear","uid":0}"#,
            r#"{"operation":"status","operation":"clear"}"#,
            r#"{"operation":"shell","command":"whoami"}"#,
        ] {
            assert!(serde_json::from_str::<Request>(input).is_err());
        }
        assert_eq!(Request::Status {}.permission(), Permission::Status);
        assert_ne!(Request::Clear {}.permission(), Permission::Status);
        let mut policy = Policy {
            version: VERSION,
            principals: vec![Principal {
                uid: 0,
                requirement: "identifier test".into(),
                public_key: [7; 32],
                permissions: vec![Permission::Status],
            }],
        };
        assert!(validate_policy(&policy).is_ok());
        policy.principals[0].permissions.push(Permission::Status);
        assert_eq!(validate_policy(&policy), Err(Error::InvalidInput));
    }
    #[tokio::test]
    async fn oversized_frame_rejected_before_reading_body() {
        let (mut sender, mut receiver) = UnixStream::pair().unwrap();
        sender.write_u32((MAX_FRAME + 1) as u32).await.unwrap();
        assert_eq!(
            read_frame(&mut receiver).await.unwrap_err(),
            Error::ResourceLimit
        );
    }
    #[test]
    fn failed_binding_does_not_activate_debug_and_audit_is_bounded() {
        let manager = Arc::new(Manager::default());
        let control = Arc::new(Control {
            manager,
            owner: Mutex::new(None),
            audit: Mutex::new(VecDeque::new()),
        });
        let session = Arc::new(Session::new().unwrap());
        let error = control
            .dispatch(
                9,
                &session,
                Request::Replace {
                    rules: vec![],
                    outbound: None,
                },
            )
            .err();
        assert_eq!(error, Some(Error::InvalidState));
        assert!(!session.status().unwrap().active);
        assert_eq!(*control.owner.lock().unwrap(), None);
        for _ in 0..MAX_AUDIT + 3 {
            control
                .record(None, Some(Permission::Status), Some(4004))
                .unwrap();
        }
        assert_eq!(control.audit.lock().unwrap().len(), MAX_AUDIT);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod signed_tests {
    use super::*;
    async fn own_peer() -> (UnixStream, UnixStream) {
        // socketpair 没有 accept 身份上下文，使用真实监听/接受产生 OS 对端凭据。
        let path = std::env::temp_dir().join(format!(
            "fluxa-peer-{}",
            crate::lifecycle::identifier().unwrap()
        ));
        let listener = UnixListener::bind(&path).unwrap();
        let client = UnixStream::connect(&path).await.unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        drop(listener);
        std::fs::remove_file(path).unwrap();
        (stream, client)
    }
    #[tokio::test]
    async fn signed_os_peer_and_operation_permissions_use_real_unix_socket() {
        // 测试启动器给本测试二进制加 Hardened Runtime 签名；生产不存在 env 绕过入口。
        let requirement = match std::env::var("FLUXA_DEBUG_TEST_REQUIREMENT") {
            Ok(value) => value,
            Err(_) => {
                let (stream, _client) = own_peer().await;
                let peer = debug_identity::inspect(&stream).unwrap();
                assert_eq!(
                    debug_identity::verify(&peer, "identifier fluxa-debug-test"),
                    Err(Error::DebugIdentityRejected)
                );
                return;
            }
        };
        let (stream, _client) = own_peer().await;
        let peer = debug_identity::inspect(&stream).unwrap();
        debug_identity::verify(&peer, &requirement).unwrap();
        let key = Ed25519KeyPair::from_seed_unchecked(&[11; 32]).unwrap();
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "fluxa-debug-auth-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(
            &directory,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let credential = directory.join("test-only-seed");
        std::fs::write(&credential, [11; 32]).unwrap();
        std::fs::set_permissions(
            &credential,
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )
        .unwrap();
        let manager = Arc::new(Manager::default());
        manager
            .configure_routing(Some(Arc::new(
                crate::routing::Router::compile(crate::routing::Config::default()).unwrap(),
            )))
            .unwrap();
        let second_key = Ed25519KeyPair::from_seed_unchecked(&[17; 32]).unwrap();
        let mut policy = Policy {
            version: VERSION,
            principals: vec![
                Principal {
                    uid: peer.uid,
                    requirement: requirement.clone(),
                    public_key: key.public_key().as_ref().try_into().unwrap(),
                    permissions: vec![Permission::Status],
                },
                Principal {
                    uid: peer.uid,
                    requirement: requirement.clone(),
                    public_key: second_key.public_key().as_ref().try_into().unwrap(),
                    permissions: vec![
                        Permission::Replace,
                        Permission::Clear,
                        Permission::Status,
                        Permission::Close,
                        Permission::Audit,
                    ],
                },
            ],
        };
        let swift = std::env::var("FLUXA_DEBUG_TEST_SWIFT").ok();
        if swift.is_some() {
            let swift_requirement = std::env::var("FLUXA_DEBUG_TEST_SWIFT_REQUIREMENT").unwrap();
            policy.principals.push(Principal {
                uid: peer.uid,
                requirement: swift_requirement,
                public_key: Ed25519KeyPair::from_seed_unchecked(&[23; 32])
                    .unwrap()
                    .public_key()
                    .as_ref()
                    .try_into()
                    .unwrap(),
                permissions: vec![
                    Permission::Replace,
                    Permission::Clear,
                    Permission::Status,
                    Permission::Close,
                    Permission::Audit,
                ],
            });
        }
        let mut service = Service::start_policy(policy, &directory, manager.clone())
            .await
            .unwrap();
        let mut client = Client::connect(
            &directory.join("debug.sock"),
            &credential,
            peer.uid,
            &requirement,
            60,
        )
        .await
        .unwrap();
        assert!(client.command(&Request::Status {}).await.unwrap().success);
        let reply = client
            .command(&Request::Replace {
                rules: vec![],
                outbound: None,
            })
            .await
            .unwrap();
        assert!(!reply.success);
        assert_eq!(
            reply.error_info.unwrap().code,
            crate::error::ErrorCode::DebugUnauthorized
        );
        assert_eq!(
            manager
                .network_snapshot()
                .unwrap()
                .router
                .as_ref()
                .unwrap()
                .evaluate(crate::routing::Target {
                    domain: Some("example.test".into()),
                    address: None,
                    port: 443,
                    transport: Transport::Tcp
                })
                .unwrap()
                .policy,
            crate::routing::Policy::Reject
        );
        drop(client);
        std::fs::write(&credential, [17; 32]).unwrap();
        let mut authorized = Client::connect(
            &directory.join("debug.sock"),
            &credential,
            peer.uid,
            &requirement,
            60,
        )
        .await
        .unwrap();
        let row = WireRule {
            rule: crate::routing::Rule {
                protocol: None,
                r#type: crate::routing::Match::IpCidr,
                value: "127.0.0.1/32".into(),
                policy: crate::routing::Policy::Direct,
                outbound: None,
            },
            protocol: None,
            lifetime_seconds: Some(30),
        };
        assert!(
            authorized
                .command(&Request::Replace {
                    rules: vec![row.clone()],
                    outbound: None
                })
                .await
                .unwrap()
                .success
        );
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let target = crate::routing::Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port: socket.local_addr().unwrap().port(),
            transport: Transport::Udp,
        };
        let snapshot = manager.network_snapshot().unwrap();
        let request = crate::outbound::RoutedTarget {
            target: target.clone(),
            timeout_ms: 1000,
            family: crate::outbound::Family::Auto,
            mapping_generation: None,
        };
        let opened = manager.connect_configured(request.clone()).await.unwrap();
        assert_eq!(
            manager
                .write(opened.connection.id, b"authorized", 1000)
                .await
                .unwrap(),
            10
        );
        let mut received = [0; 16];
        assert_eq!(socket.recv(&mut received).await.unwrap(), 10);
        assert!(
            authorized
                .command(&Request::Clear {})
                .await
                .unwrap()
                .success
        );
        assert_eq!(
            snapshot
                .router
                .as_ref()
                .unwrap()
                .evaluate(target.clone())
                .unwrap()
                .policy,
            crate::routing::Policy::Reject
        );
        assert_eq!(
            manager
                .write(opened.connection.id, b"revoked", 1000)
                .await
                .err(),
            Some(Error::InvalidState)
        );
        // clear 成功意味着旧连接已经关闭并释放句柄名额。
        assert!(manager.status().unwrap().sessions.is_empty());
        assert!(
            authorized
                .command(&Request::Replace {
                    rules: vec![row],
                    outbound: None
                })
                .await
                .unwrap()
                .success
        );
        assert!(
            authorized
                .command(&Request::Clear {})
                .await
                .unwrap()
                .success
        );
        if let Some(swift) = swift {
            std::fs::write(&credential, [23; 32]).unwrap();
            let output = std::process::Command::new(swift)
                .arg(directory.join("debug.sock"))
                .arg(&credential)
                .arg(peer.uid.to_string())
                .arg(&requirement)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "Swift 签名 IPC 夹具失败：{}；{}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            );
            eprintln!("{}", String::from_utf8_lossy(&output.stdout));
            std::fs::write(&credential, [17; 32]).unwrap();
        }
        assert!(
            authorized
                .command(&Request::Replace {
                    rules: vec![WireRule {
                        rule: crate::routing::Rule {
                            protocol: None,
                            r#type: crate::routing::Match::IpCidr,
                            value: "127.0.0.1/32".into(),
                            policy: crate::routing::Policy::Direct,
                            outbound: None
                        },
                        protocol: None,
                        lifetime_seconds: Some(30)
                    }],
                    outbound: None
                })
                .await
                .unwrap()
                .success
        );
        // 正确签名但重复序号仍必须拒绝并撤销当前连接授权。
        let payload = serde_json::to_vec(&Request::Status {}).unwrap();
        let sequence = authorized.sequence - 1;
        let signature = authorized
            .key
            .sign(&transcript(&authorized.hello, sequence, &payload))
            .as_ref()
            .to_vec();
        write_frame(
            &mut authorized.stream,
            &Envelope {
                sequence,
                payload,
                signature,
            },
        )
        .await
        .unwrap();
        let reply: Reply =
            serde_json::from_slice(&read_frame(&mut authorized.stream).await.unwrap()).unwrap();
        assert_eq!(
            reply.error_info.unwrap().code,
            crate::error::ErrorCode::DebugAuthenticationFailed
        );
        drop(authorized);
        service.close().await.unwrap();
        service.close().await.unwrap();
        assert!(!directory.join("debug.sock").exists());
        manager.stop().await.unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }
}
