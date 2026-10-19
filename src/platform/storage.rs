//! 系统共享容器与宿主身份适配；不从 HOME、环境变量或配置原文推断容器。
use crate::error::{Error, Result};
use std::path::PathBuf;

pub(crate) struct Context {
    pub root: PathBuf,
    pub read_only_configuration: bool,
    pub language: String,
}

pub(crate) fn context() -> Result<Context> {
    #[cfg(target_vendor = "apple")]
    {
        apple::context()
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        Err(Error::Unsupported)
    }
}

#[cfg(target_vendor = "apple")]
mod apple {
    use super::*;
    #[cfg(target_os = "macos")]
    use std::ptr;
    use std::{
        ffi::{CStr, c_char, c_void},
        path::Path,
    };
    type Object = *mut c_void;
    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {}
    #[cfg(target_os = "macos")]
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecTaskCreateFromSelf(allocator: Object) -> Object;
        fn SecTaskCopyValueForEntitlement(task: Object, key: Object, error: *mut Object) -> Object;
    }
    #[cfg(target_os = "macos")]
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFGetTypeID(value: Object) -> usize;
        fn CFArrayGetTypeID() -> usize;
        fn CFRelease(value: Object);
    }
    #[cfg(target_os = "macos")]
    struct Owned(Object);
    #[cfg(target_os = "macos")]
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: 仅持有由 Create/Copy 返回的独立 CF 引用；独占 guard 释放一次。
                unsafe { CFRelease(self.0) };
            }
        }
    }
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Object;
        fn sel_registerName(name: *const c_char) -> Object;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> Object;
        fn objc_autoreleasePoolPop(pool: Object);
    }
    struct Pool(Object);
    impl Drop for Pool {
        fn drop(&mut self) {
            // SAFETY: 令牌由本线程 push 创建，独占 guard 保证只在同线程 pop 一次。
            unsafe { objc_autoreleasePoolPop(self.0) };
        }
    }
    fn class(name: &CStr) -> Object {
        // SAFETY: 类名为静态 NUL 结尾字节；运行时返回的类对象不由调用者释放。
        unsafe { objc_getClass(name.as_ptr()) }
    }
    fn selector(name: &CStr) -> Object {
        // SAFETY: 选择子为静态 NUL 结尾字节，运行时拥有其生命周期。
        unsafe { sel_registerName(name.as_ptr()) }
    }
    fn send(receiver: Object, name: &CStr) -> Object {
        // SAFETY: 仅用于下列已核对的无参数、对象返回类型 Foundation 方法。
        unsafe {
            let call: unsafe extern "C" fn(Object, Object) -> Object =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            call(receiver, selector(name))
        }
    }
    fn send_object(receiver: Object, name: &CStr, argument: Object) -> Object {
        // SAFETY: 仅用于一个对象参数、对象返回类型的方法，参数由当前 autorelease pool 保活。
        unsafe {
            let call: unsafe extern "C" fn(Object, Object, Object) -> Object =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            call(receiver, selector(name), argument)
        }
    }
    fn string(value: &CStr) -> Object {
        // SAFETY: stringWithUTF8String: 的唯一参数是本次同步借用的有效 C 字符串。
        unsafe {
            let call: unsafe extern "C" fn(Object, Object, *const c_char) -> Object =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            call(
                class(c"NSString"),
                selector(c"stringWithUTF8String:"),
                value.as_ptr(),
            )
        }
    }
    fn text(value: Object, method: &CStr) -> Result<String> {
        if value.is_null() {
            return Err(Error::InvalidState);
        }
        // SAFETY: 仅向 NSString 发送 UTF8String 或向 NSURL 发送 fileSystemRepresentation；
        // 返回的 C 字节在 pool 存续期复制，不跨调用保存指针。
        unsafe {
            let call: unsafe extern "C" fn(Object, Object) -> *const c_char =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let bytes = call(value, selector(method));
            if bytes.is_null() {
                return Err(Error::InvalidState);
            }
            let bytes = CStr::from_ptr(bytes).to_bytes();
            if bytes.len() > 4096 {
                return Err(Error::ResourceLimit);
            }
            String::from_utf8(bytes.to_vec()).map_err(|_| Error::InvalidInput)
        }
    }
    #[cfg(target_os = "macos")]
    fn verify_entitlement() -> Result<()> {
        // SAFETY: 只读取当前进程真实 Security Task；空 allocator 采用系统默认分配器。
        let task = Owned(unsafe { SecTaskCreateFromSelf(ptr::null_mut()) });
        if task.0.is_null() {
            return Err(Error::InvalidState);
        }
        let mut error = ptr::null_mut();
        // SAFETY: task 存活，NSString 与 CFString toll-free 对应；error 独占可写。
        let value = Owned(unsafe {
            SecTaskCopyValueForEntitlement(
                task.0,
                string(c"com.apple.security.application-groups"),
                &mut error,
            )
        });
        let _error = Owned(error);
        if value.0.is_null() {
            return Err(Error::InvalidState);
        }
        // SAFETY: 非空 CF 返回对象的类型先检查，只有数组才调用 NSArray 方法。
        if unsafe { CFGetTypeID(value.0) != CFArrayGetTypeID() } {
            return Err(Error::InvalidState);
        }
        // SAFETY: 真实 CFArray toll-free 对应 NSArray，containsObject: 返回 BOOL，不接收外部对象指针。
        let contains = unsafe {
            let call: unsafe extern "C" fn(Object, Object, Object) -> u8 =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            call(
                value.0,
                selector(c"containsObject:"),
                string(c"group.app.fluxa.ios"),
            )
        };
        if contains == 0 {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    pub(super) fn context() -> Result<Context> {
        // SAFETY: pool 在当前同步调用内创建和释放；没有对象指针逃出作用域。
        let _pool = Pool(unsafe { objc_autoreleasePoolPush() });
        #[cfg(target_os = "macos")]
        verify_entitlement()?;
        let bundle = send(class(c"NSBundle"), c"mainBundle");
        if bundle.is_null() {
            return Err(Error::InvalidState);
        }
        let extension = send_object(
            bundle,
            c"objectForInfoDictionaryKey:",
            string(c"NSExtension"),
        );
        let bundle_url = send(bundle, c"bundleURL");
        let bundle_path = text(bundle_url, c"fileSystemRepresentation")?;
        let read_only_configuration = !extension.is_null()
            || Path::new(&bundle_path)
                .extension()
                .is_some_and(|v| v == "appex");
        let manager = send(class(c"NSFileManager"), c"defaultManager");
        let url = send_object(
            manager,
            c"containerURLForSecurityApplicationGroupIdentifier:",
            string(c"group.app.fluxa.ios"),
        );
        if url.is_null() {
            return Err(Error::InvalidState);
        }
        let root = PathBuf::from(text(url, c"fileSystemRepresentation")?);
        let languages = send(class(c"NSLocale"), c"preferredLanguages");
        let language = text(send(languages, c"firstObject"), c"UTF8String")?;
        Ok(Context {
            root,
            read_only_configuration,
            language,
        })
    }
}

/// 文件操作绑定已打开目录描述符，拒绝跟随目录和最终文件符号链接。
#[cfg(unix)]
pub(crate) struct Directory(std::fs::File);
#[cfg(unix)]
impl Directory {
    pub fn open(root: &std::path::Path, child: &str, create: bool) -> Result<Self> {
        use std::os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::OpenOptionsExt,
        };
        let parent = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)?;
        let child = name(child)?;
        if create {
            // SAFETY: 父目录 FD 有效，名称为单一 NUL 结尾组件，不保留指针。
            let result = unsafe { libc::mkdirat(parent.as_raw_fd(), child.as_ptr(), 0o700) };
            if result != 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        // SAFETY: 只打开父目录下单级目录；NOFOLLOW 拒绝符号链接，成功 FD 立即交给 File 独占释放。
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                child.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: fd 是上面成功创建、尚未归属任何 File 的有效描述符。
        Ok(Self(unsafe { std::fs::File::from_raw_fd(fd) }))
    }
    fn file(&self, value: &str, flags: i32) -> Result<std::fs::File> {
        self.open_file(&name(value)?, flags).map_err(Into::into)
    }
    fn open_file(&self, value: &std::ffi::CStr, flags: i32) -> std::io::Result<std::fs::File> {
        use std::os::fd::{AsRawFd, FromRawFd};
        // SAFETY: 当前目录 FD 保活，单级 C 字符串只在同步 openat 期间借用。
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                value.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: 成功 FD 首次且唯一交给 File 管理。
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        Ok(file)
    }
    pub fn validate_target(&self, value: &str) -> Result<()> {
        use std::os::fd::AsRawFd;
        let value = name(value)?;
        // SAFETY: stat 为 C POD 零初始化，指针只在同步 fstatat 内借用；NOFOLLOW 只检查目录项本身。
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: 有效父目录 FD、单级名称以及独占可写 stat。
        if unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                value.as_ptr(),
                &mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            let error = std::io::Error::last_os_error();
            return if error.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(error.into())
            };
        }
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    pub fn lock(&self) -> Result<std::fs::File> {
        let value = name(".configuration.lock")?;
        // 先打开已有锁；新建用 EXCL，竞争者只能重新打开，绝不截断/替换锁文件。
        let file = match self.open_file(&value, libc::O_RDWR) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match self.open_file(&value, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL) {
                    Ok(file) => file,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        self.open_file(&value, libc::O_RDWR)?
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Err(error) => return Err(error.into()),
        };
        file.try_lock().map_err(|e| match e {
            std::fs::TryLockError::WouldBlock => Error::InvalidState,
            std::fs::TryLockError::Error(e) => e.into(),
        })?;
        Ok(file)
    }
    pub fn read(&self, value: &str) -> Result<zeroize::Zeroizing<String>> {
        use std::io::Read;
        let file = self.file(value, libc::O_RDONLY)?;
        let maximum = crate::configuration::MAX_CONFIG_BYTES;
        if file.metadata()?.len() > maximum as u64 {
            return Err(Error::ResourceLimit);
        }
        let mut data = zeroize::Zeroizing::new(Vec::new());
        data.try_reserve_exact(maximum + 1)
            .map_err(|_| Error::ResourceLimit)?;
        file.take((maximum + 1) as u64).read_to_end(&mut data)?;
        if data.len() > maximum {
            return Err(Error::ResourceLimit);
        }
        let text = std::str::from_utf8(&data).map_err(|_| Error::InvalidInput)?;
        let mut output = String::new();
        output
            .try_reserve_exact(text.len())
            .map_err(|_| Error::ResourceLimit)?;
        output.push_str(text);
        Ok(zeroize::Zeroizing::new(output))
    }
    pub fn has_yaml(&self) -> Result<bool> {
        use std::{ffi::CStr, os::fd::AsRawFd};
        // SAFETY: dup 复制有效 FD，fdopendir 成功后接管该副本，原 FD 仍由 self 持有。
        let fd = unsafe { libc::dup(self.0.as_raw_fd()) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: fd 是可读目录的独立描述符。
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            // SAFETY: fdopendir 失败未接管 fd；关闭一次。
            unsafe { libc::close(fd) };
            return Err(std::io::Error::last_os_error().into());
        }
        struct Stream(*mut libc::DIR);
        impl Drop for Stream {
            fn drop(&mut self) {
                // SAFETY: 当前 guard 独占此有效 DIR，关闭后不再使用。
                unsafe { libc::closedir(self.0) };
            }
        }
        let stream = Stream(stream);
        // SAFETY: 新 DIR 有效；重置 dup 共享的目录偏移，重复检查不能漏掉已有配置。
        unsafe { libc::rewinddir(stream.0) };
        for _ in 0..258 {
            set_errno(0);
            // SAFETY: stream 有效；目录项在下次 readdir 前立即复制/比较，不保留指针。
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                let error = std::io::Error::last_os_error();
                return if error.raw_os_error() == Some(0) {
                    Ok(false)
                } else {
                    Err(error.into())
                };
            }
            // SAFETY: POSIX 保证 d_name 是该有效目录项内的 NUL 结尾文件名。
            let bytes = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidInput)?;
            if text.rsplit_once('.').is_some_and(|(_, extension)| {
                extension.eq_ignore_ascii_case("yaml") || extension.eq_ignore_ascii_case("yml")
            }) {
                // 存在符号链接、坏 YAML 或目录也不能以新配置覆盖它；读文件时再严格拒绝。
                return Ok(true);
            }
        }
        Err(Error::ResourceLimit)
    }
    pub fn write(&self, target: &str, bytes: &[u8], replace: bool) -> Result<bool> {
        use std::{io::Write, os::fd::AsRawFd};
        let target = name(target)?;
        // 临时文件名不能依赖会在新进程重置的计数器；崩溃遗留文件不阻塞后续保存。
        use ring::rand::SecureRandom;
        let mut nonce = [0u8; 16];
        ring::rand::SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| Error::Internal)?;
        let temporary = name(&format!(".config-{:032x}.tmp", u128::from_ne_bytes(nonce)))?;
        // SAFETY: 有效目录 FD、独立临时单级文件名；EXCL/NOFOLLOW 防止覆盖或跟随既存文件。
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        use std::os::fd::FromRawFd;
        // SAFETY: fd 成功创建且尚未被接管。
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            // SAFETY: 两个名称均位于 self 的相同存活目录 FD；不经过外部路径。
            let code = unsafe {
                if replace {
                    libc::renameat(
                        self.0.as_raw_fd(),
                        temporary.as_ptr(),
                        self.0.as_raw_fd(),
                        target.as_ptr(),
                    )
                } else {
                    libc::linkat(
                        self.0.as_raw_fd(),
                        temporary.as_ptr(),
                        self.0.as_raw_fd(),
                        target.as_ptr(),
                        0,
                    )
                }
            };
            if code != 0 {
                let error = std::io::Error::last_os_error();
                if !replace && error.kind() == std::io::ErrorKind::AlreadyExists {
                    return Ok(false);
                }
                return Err(error.into());
            }
            self.0.sync_all()?;
            Ok(true)
        })();
        // SAFETY: 临时名称为此调用拥有；rename 成功后 ENOENT 正常，其余失败需报告。
        let removed = unsafe { libc::unlinkat(self.0.as_raw_fd(), temporary.as_ptr(), 0) };
        if removed != 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::NotFound {
            return Err(std::io::Error::last_os_error().into());
        }
        result
    }
}
#[cfg(unix)]
fn name(value: &str) -> Result<std::ffi::CString> {
    if value.is_empty()
        || value.len() > 255
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(Error::InvalidInput);
    }
    std::ffi::CString::new(value).map_err(|_| Error::InvalidInput)
}

#[cfg(unix)]
fn set_errno(value: i32) {
    #[cfg(target_vendor = "apple")]
    // SAFETY: libc 返回当前线程专属 errno 指针，只在同步调用内写入。
    unsafe {
        *libc::__error() = value;
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    // SAFETY: 同上，使用当前目标的 libc errno 入口。
    unsafe {
        *libc::__errno_location() = value;
    }
    #[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
    let _ = value;
}
