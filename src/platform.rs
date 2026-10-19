//! 操作系统身份与代码签名适配，不接受客户端自行声明的可信身份。
pub(crate) mod debug_identity;

/// 有界操作系统进程归属查询；外部进程名声明不是可信身份。
pub(crate) mod process;

/// 提供者日历与同级目录原子交换。
pub(crate) mod providers;

/// Apple 共享容器与只读扩展宿主身份。
pub(crate) mod storage;
