//! 提供者的本地日历与原子目录交换边界；不接受客户端声明的日期或占用权。
use crate::error::{Error, Result};
use std::path::Path;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Day {
    pub date: i32,
    pub previous: i32,
    pub seconds: u32,
}
/// 系统本地日历：一天不是固定 86400 秒；前一天用本地正午回退，跨夏令时仍按日期补一次。
pub(crate) fn local_day(now: u64) -> Result<Day> {
    #[cfg(unix)]
    {
        let seconds: libc::time_t = now.try_into().map_err(|_| Error::InvalidInput)?;
        // SAFETY: tm 是 C 整数/指针 POD，零值有效；指针仅在系统调用期存在，随后检查返回指针。
        let mut date: libc::tm = unsafe { std::mem::zeroed() };
        // SAFETY: 输入 time_t 与独占可写 tm 均在栈上真实有效；不保留 libc 全局日历指针。
        if unsafe { libc::localtime_r(&seconds, &mut date) }.is_null() {
            return Err(Error::Internal);
        }
        let key = |tm: &libc::tm| -> Result<i32> {
            let year = tm.tm_year.checked_add(1900).ok_or(Error::ResourceLimit)?;
            if !(1970..=9999).contains(&year) {
                return Err(Error::InvalidInput);
            }
            Ok(year * 10000 + (tm.tm_mon + 1) * 100 + tm.tm_mday)
        };
        let current = key(&date)?;
        let within = (date.tm_hour * 3600 + date.tm_min * 60 + date.tm_sec.min(59)) as u32;
        date.tm_hour = 12;
        date.tm_min = 0;
        date.tm_sec = 0;
        date.tm_mday -= 1;
        date.tm_isdst = -1;
        // SAFETY: 独占完整 tm 交由 libc 按设备本地时区归一化；不以秒差推算民用日期。
        let yesterday = unsafe { libc::mktime(&mut date) };
        let previous = if yesterday < 0 { 0 } else { key(&date)? };
        Ok(Day {
            date: current,
            previous,
            seconds: within,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = now;
        Err(Error::Unsupported)
    }
}
/// 两个同级目录原子交换，文件、来源和更新时间记录始终属于同一缓存代次。
/// 不使用先删旧目录再 rename 的窗口；不支持的系统/文件系统明确报错。
pub(crate) fn exchange(left: &Path, right: &Path) -> Result<()> {
    #[cfg(any(target_vendor = "apple", target_os = "linux"))]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        if left.parent() != right.parent() {
            return Err(Error::InvalidInput);
        }
        let left = CString::new(left.as_os_str().as_bytes()).map_err(|_| Error::InvalidInput)?;
        let right = CString::new(right.as_os_str().as_bytes()).map_err(|_| Error::InvalidInput)?;
        #[cfg(target_vendor = "apple")]
        // SAFETY: 两个 NUL 结尾路径在同步调用期间有效；标志只允许原子交换，不传递业务裸指针。
        let code = unsafe {
            libc::renameatx_np(
                libc::AT_FDCWD,
                left.as_ptr(),
                libc::AT_FDCWD,
                right.as_ptr(),
                libc::RENAME_SWAP,
            )
        };
        #[cfg(target_os = "linux")]
        // SAFETY: 同上；Linux 使用标准 RENAME_EXCHANGE，不能降级为非原子多次重命名。
        let code = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                left.as_ptr(),
                libc::AT_FDCWD,
                right.as_ptr(),
                libc::RENAME_EXCHANGE,
            )
        };
        if code != 0 {
            let error = std::io::Error::last_os_error();
            return Err(match error.raw_os_error() {
                Some(libc::ENOSYS | libc::ENOTSUP | libc::EINVAL) => Error::Unsupported,
                _ => error.into(),
            });
        }
        Ok(())
    }
    #[cfg(not(any(target_vendor = "apple", target_os = "linux")))]
    {
        let _ = (left, right);
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
#[path = "../../tests/test_platform_providers.rs"]
mod test_platform_providers;
