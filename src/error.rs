//! 稳定错误类别不包含正文、凭据或私钥；协议模块共用一套错误。
use std::{fmt, io};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidInput,
    DnsTimeout,
    DnsResponse,
    DnsValidation,
    ResourceLimit,
    Network,
    Tls,
    Certificate,
    InvalidState,
    Unsupported,
    Internal,
    Script,
    ScriptTimeout,
    Rejected,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DnsTimeout => "DNS 查询超时",
            Self::DnsValidation => "DNSSEC 验证或验证查询失败",
            Self::DnsResponse => "DNS 上游返回错误状态",
            Self::InvalidInput => "请求或配置不合法",
            Self::ResourceLimit => "超过资源上限",
            Self::Network => "网络连接或读写失败",
            Self::Tls => "TLS 握手或上游证书校验失败",
            Self::Certificate => "证书或密钥不合法",
            Self::InvalidState => "资源状态不允许此操作",
            Self::Unsupported => "请求的能力尚未支持",
            Self::Script => "脚本执行失败或结果不是有效 JSON 数据",
            Self::Rejected => "请求被 Fluxa 插件规则拒绝",
            Self::ScriptTimeout => "脚本执行超时",
            Self::Internal => "内部资源错误",
        })
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(_: io::Error) -> Self {
        Self::Network
    }
}
pub type Result<T> = std::result::Result<T, Error>;
