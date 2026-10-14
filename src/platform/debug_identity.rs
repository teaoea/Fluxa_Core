//! 安全边界：从已连接 Unix IPC 取得不可由请求伪造的身份，按 audit token 动态验签。
use crate::error::{Error, Result};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub uid: u32,
    pub pid: u32,
    pub token: [u8; 32],
}
#[cfg(target_os = "macos")]
mod apple {
    use super::*;
    use core_foundation::{
        base::{CFTypeRef, TCFType},
        data::CFData,
        dictionary::CFDictionary,
        string::{CFString, CFStringRef},
    };
    use security_framework::os::macos::code_signing::{
        Flags, GuestAttributes, SecCode, SecRequirement,
    };
    use std::{ffi::c_void, os::fd::AsRawFd};
    // Apple CSCommon.h：运行时加固与调试状态是系统定义的标志，不是业务优先级。
    const SIGNATURE_RUNTIME: u32 = 0x10000;
    const STATUS_DEBUGGED: u32 = 0x10000000;
    const SIGNING_INFORMATION: u32 = 1 << 1;
    const DYNAMIC_INFORMATION: u32 = 1 << 3;
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecCodeCopySigningInformation(code: CFTypeRef, flags: u32, info: *mut CFTypeRef) -> i32;
        static kSecCodeInfoFlags: CFStringRef;
        static kSecCodeInfoStatus: CFStringRef;
        static kSecCodeInfoEntitlementsDict: CFStringRef;
    }
    pub(crate) fn inspect(stream: &tokio::net::UnixStream) -> Result<Identity> {
        let peer = stream
            .peer_cred()
            .map_err(|_| Error::DebugIdentityRejected)?;
        let pid = peer
            .pid()
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v != 0)
            .ok_or(Error::DebugIdentityRejected)?;
        let mut token = [0u8; 32];
        let mut size = token.len() as libc::socklen_t;
        // SAFETY: 已连接 Socket 的 fd 在同步调用期间存活；固定数组和长度独占可写，返回长度严格检查。
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_LOCAL,
                libc::LOCAL_PEERTOKEN,
                token.as_mut_ptr().cast::<c_void>(),
                &mut size,
            )
        };
        if result != 0 || size as usize != token.len() {
            return Err(Error::DebugIdentityRejected);
        }
        Ok(Identity {
            uid: peer.uid(),
            pid,
            token,
        })
    }
    pub(crate) fn validate_requirement(requirement: &str) -> Result<()> {
        if requirement.is_empty() || requirement.len() > 1024 {
            return Err(Error::InvalidInput);
        }
        requirement
            .parse::<SecRequirement>()
            .map(|_| ())
            .map_err(|_| Error::InvalidInput)
    }
    pub(crate) fn verify(identity: &Identity, requirement: &str) -> Result<()> {
        if requirement.is_empty() || requirement.len() > 1024 {
            return Err(Error::DebugIdentityRejected);
        }
        let data = CFData::from_buffer(&identity.token);
        let mut guest = GuestAttributes::new();
        guest.set_audit_token(data.as_concrete_TypeRef());
        let code =
            SecCode::copy_guest_with_attribues(None, &guest, Flags::NONE).map_err(|_error| {
                #[cfg(test)]
                eprintln!("对端代码查询状态：{}", _error.code());
                Error::DebugIdentityRejected
            })?;
        let required: SecRequirement = requirement
            .parse()
            .map_err(|_| Error::DebugIdentityRejected)?;
        code.check_validity(Flags::STRICT_VALIDATE | Flags::NO_NETWORK_ACCESS, &required)
            .map_err(|_error| {
                #[cfg(test)]
                eprintln!("代码有效性状态：{}", _error.code());
                Error::DebugIdentityRejected
            })?;
        let mut raw = std::ptr::null();
        // SAFETY: 代码对象存活；公开 API 返回 Copy 所有权交给 RAII，失败不解引用。
        let copied = unsafe {
            SecCodeCopySigningInformation(
                code.as_CFTypeRef(),
                SIGNING_INFORMATION | DYNAMIC_INFORMATION,
                &mut raw,
            )
        };
        #[cfg(test)]
        eprintln!("签名资料查询状态：{}", copied);
        if copied != 0 || raw.is_null() {
            return Err(Error::DebugIdentityRejected);
        }
        // SAFETY: 成功 API 返回 CFDictionary 的拥有引用；仅转换实际字典类型并由 RAII 释放。
        let info: CFDictionary<CFString, core_foundation::base::CFType> =
            unsafe { CFDictionary::wrap_under_create_rule(raw.cast()) };
        // SAFETY: Security 框架公开常量在进程内永久有效，get rule 不夺取静态所有权。
        let flags_key = unsafe { CFString::wrap_under_get_rule(kSecCodeInfoFlags) };
        // SAFETY: Security 框架公开静态键永久有效，不转移其所有权。
        let status_key = unsafe { CFString::wrap_under_get_rule(kSecCodeInfoStatus) };
        let dynamic = info
            .find(&status_key)
            .and_then(|v| v.downcast::<core_foundation::number::CFNumber>())
            .and_then(|v| v.to_i64())
            .ok_or(Error::DebugIdentityRejected)? as u32;
        if dynamic & STATUS_DEBUGGED != 0 {
            return Err(Error::DebugIdentityRejected);
        }
        let flags = info
            .find(&flags_key)
            .and_then(|v| v.downcast::<core_foundation::number::CFNumber>())
            .and_then(|v| v.to_i64())
            .ok_or(Error::DebugIdentityRejected)? as u32;
        #[cfg(test)]
        eprintln!("运行时加固标志：{:x}，动态状态：{:x}", flags, dynamic);
        if flags & SIGNATURE_RUNTIME == 0 {
            return Err(Error::DebugIdentityRejected);
        }
        // SAFETY: 框架静态键永久有效；字典内值由 info 所有者保活。
        let entitlement_key =
            unsafe { CFString::wrap_under_get_rule(kSecCodeInfoEntitlementsDict) };
        if let Some(entitlements) = info
            .find(&entitlement_key)
            .and_then(|v| v.downcast::<CFDictionary>())
        {
            // SAFETY: downcast 已验证 CFDictionary 类型；拥有引用保活，转换仅指定实际键值 Rust 视图。
            let entitlements: CFDictionary<CFString, core_foundation::base::CFType> =
                unsafe { CFDictionary::wrap_under_get_rule(entitlements.as_concrete_TypeRef()) };
            for name in [
                "com.apple.security.get-task-allow",
                "com.apple.security.cs.disable-library-validation",
                "com.apple.security.cs.allow-dyld-environment-variables",
                "com.apple.security.cs.allow-unsigned-executable-memory",
                "com.apple.security.cs.disable-executable-page-protection",
            ] {
                if entitlements
                    .find(CFString::new(name))
                    .and_then(|v| v.downcast::<core_foundation::boolean::CFBoolean>())
                    .is_some_and(|v| v.into())
                {
                    return Err(Error::DebugIdentityRejected);
                }
            }
        }
        Ok(())
    }
}
#[cfg(target_os = "macos")]
pub(crate) use apple::{inspect, validate_requirement, verify};
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn inspect(_stream: &tokio::net::UnixStream) -> Result<Identity> {
    Err(Error::Unsupported)
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn validate_requirement(_requirement: &str) -> Result<()> {
    Err(Error::Unsupported)
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn verify(_identity: &Identity, _requirement: &str) -> Result<()> {
    Err(Error::Unsupported)
}

#[cfg(unix)]
fn trust_io<T>(result: std::io::Result<T>) -> Result<T> {
    result.map_err(|error| {
        let mapped = if error.kind() == std::io::ErrorKind::OutOfMemory {
            Error::ResourceLimit
        } else {
            Error::DebugIdentityRejected
        };
        crate::error::diagnostics::record_io(&error, mapped.code());
        mapped
    })
}
#[cfg(unix)]
fn protected_file(
    path: &std::path::Path,
    maximum: usize,
    root_owned: bool,
) -> Result<Zeroizing<Vec<u8>>> {
    use std::{
        io::Read,
        os::unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    };
    if !path.is_absolute() || path.as_os_str().as_bytes().len() > 4096 {
        return Err(Error::InvalidInput);
    }
    // SAFETY: geteuid 无参数，不持有或返回业务指针，仅查询当前内核进程身份。
    let uid = if root_owned {
        0
    } else {
        // SAFETY: geteuid 无参数和业务内存访问，仅查询有效用户身份。
        unsafe { libc::geteuid() }
    };
    let mut cursor = std::path::PathBuf::from("/");
    for component in path.parent().ok_or(Error::InvalidInput)?.components() {
        match component {
            std::path::Component::RootDir => (),
            std::path::Component::Normal(name) => cursor.push(name),
            _ => return Err(Error::InvalidInput),
        }
        let metadata = trust_io(std::fs::symlink_metadata(&cursor))?;
        // 根策略的每级目录必须不允许非 root 写入；凭证目录允许当前用户私有目录。
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || (root_owned && (metadata.uid() != 0 || metadata.mode() & 0o022 != 0))
        {
            return Err(Error::DebugIdentityRejected);
        }
    }
    let mut file = trust_io(
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path),
    )?;
    let metadata = trust_io(file.metadata())?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & (if root_owned { 0o022 } else { 0o077 }) != 0
    {
        return Err(Error::DebugIdentityRejected);
    }
    if metadata.len() > maximum as u64 {
        return Err(Error::ResourceLimit);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(maximum.min(metadata.len() as usize)));
    trust_io(
        file.by_ref()
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes),
    )?;
    if bytes.len() > maximum {
        return Err(Error::ResourceLimit);
    }
    Ok(bytes)
}
#[cfg(unix)]
use zeroize::Zeroizing;
#[cfg(unix)]
pub(crate) fn read_policy(path: &std::path::Path, maximum: usize) -> Result<Zeroizing<Vec<u8>>> {
    protected_file(path, maximum, true)
}
#[cfg(unix)]
pub(crate) fn read_credential(path: &std::path::Path) -> Result<Zeroizing<Vec<u8>>> {
    let bytes = protected_file(path, 32, false)?;
    if bytes.len() != 32 {
        return Err(Error::DebugAuthenticationFailed);
    }
    Ok(bytes)
}
#[cfg(unix)]
pub(crate) fn private_directory(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if !path.is_absolute() {
        return Err(Error::InvalidInput);
    }
    let metadata = trust_io(std::fs::symlink_metadata(path))?;
    // SAFETY: 系统身份查询不读写任何调用方内存。
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::DebugIdentityRejected);
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod file_tests {
    use super::*;
    #[test]
    fn private_seed_rejects_wrong_permissions_symlinks_and_wrong_size() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "fluxa-debug-file-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let file = directory.join("seed");
        std::fs::write(&file, [3; 32]).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(read_credential(&file).unwrap().len(), 32);
        let alias = directory.join("alias");
        symlink(&file, &alias).unwrap();
        assert!(read_credential(&alias).is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            read_credential(&file),
            Err(Error::DebugIdentityRejected)
        ));
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&file, [3; 31]).unwrap();
        assert!(matches!(
            read_credential(&file),
            Err(Error::DebugAuthenticationFailed)
        ));
        std::fs::write(&file, [3; 33]).unwrap();
        assert!(matches!(read_credential(&file), Err(Error::ResourceLimit)));
        // 临时目录可被普通用户写入，不能充当 root 管理策略的信任链。
        assert!(matches!(
            read_policy(&file, 32768),
            Err(Error::DebugIdentityRejected)
        ));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
