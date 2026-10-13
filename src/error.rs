//! 内核公开错误协议的唯一权威定义；与自然语言、调用方身份和枚举排列无关。
use serde::{Deserialize, Serialize};

/// 每项定义同时生成强类型代码、固定整数值和可审查契约；新增项必须更新文档快照。
macro_rules! define_codes {
    ($($(#[$doc:meta])* $variant:ident => ($id:literal, $name:literal, $category:literal,
        $detail:literal, $trigger:literal, $effect:literal, $source:literal)),+ $(,)?) => {
        #[repr(u16)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum ErrorCode {
            $($(#[$doc])* $variant = $id),+
        }
        impl Serialize for ErrorCode {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                serializer.serialize_u16(self.value())
            }
        }
        impl<'de> Deserialize<'de> for ErrorCode {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
                let value = u16::deserialize(deserializer)?;
                match value { $($id => Ok(Self::$variant)),+,
                    _ => Err(serde::de::Error::custom("未登记的整数错误代码")),
                }
            }
        }
        impl ErrorCode {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn value(self) -> u16 {
                match self { $(Self::$variant => $id),+ }
            }
            /// 整数只作稳定协议标识，不从枚举排列或数字运算推断语义。
            pub const fn from_value(value: u16) -> Option<Self> {
                match value { $($id => Some(Self::$variant)),+, _ => None }
            }
            /// 固定错误 JSON，不进行堆分配或调用可能失败的序列化器。
            pub const fn json_bytes(self) -> &'static [u8] {
                match self { $(Self::$variant => concat!("{\"code\":", stringify!($id), ",\"params\":{}}").as_bytes()),+ }
            }
            pub const fn contract(self) -> ErrorContract {
                match self { $(Self::$variant => ErrorContract {
                    code: $id, name: $name, category: $category, modules: Self::$variant.modules(), detail: $detail,
                    trigger: $trigger, effect: $effect, source: $source,
                    params: "{}（不接受任何参数键）", status: "有效",
                }),+ }
            }
        }
    };
}
/// 契约字段来自实际映射，文档一致性不代替独立业务语义审查。
#[derive(Debug, Serialize)]
pub struct ErrorContract {
    pub code: u16,
    pub name: &'static str,
    pub category: &'static str,
    pub modules: &'static str,
    pub detail: &'static str,
    pub trigger: &'static str,
    pub effect: &'static str,
    pub params: &'static str,
    pub source: &'static str,
    pub status: &'static str,
}
define_codes! {
    /// 请求、配置或规则未满足类型、格式、范围及完整性约束；现有通用类别不能声称具体字段错误。
    InvalidInput => (1001, "无效输入", "INPUT", "请求、配置或规则未满足类型、格式、范围及完整性约束；现有通用类别不能声称具体字段错误。", "解析、字段校验、规则或协议帧校验拒绝输入。", "当前操作返回失败；原子配置提交在校验成功前不替换有效快照。", "src/routing/yaml.rs decode；src/routing/runtime.rs；src/protocol/config.rs"),
    /// 显式配置上游的单次或总期限耗尽，未取得符合要求的 DNS 结果。
    DnsTimeout => (1002, "DNS 查询超时", "DNS", "显式配置上游的单次或总期限耗尽，未取得符合要求的 DNS 结果。", "Resolver::resolve 的超时或 NetError::Timeout 分支。", "返回 DNS 失败，出站不能把此结果转换成直连成功。", "src/dns/mod.rs Resolver::resolve"),
    /// 上游响应、解析结果或地址结果不满足要求；合法 NXDOMAIN 可作为负响应返回，不必属于此错误。
    DnsResponse => (1003, "DNS 响应失败", "DNS", "上游响应、解析结果或地址结果不满足要求；合法 NXDOMAIN 可作为负响应返回，不必属于此错误。", "DNS 上游非成功状态、非允许负响应、无可用目标地址。", "拒绝当前解析或依赖该结果的连接；不冒充规则拒绝。", "src/dns/mod.rs Resolver::resolve；src/outbound/resolution.rs"),
    /// 启用 DNSSEC 的查询未通过验证或其验证查询失败，不保证底层只发生签名错误。
    DnsValidation => (1004, "DNS 验证失败", "DNS", "启用 DNSSEC 的查询未通过验证或其验证查询失败，不保证底层只发生签名错误。", "Resolver 的 DNSSEC 失败分支。", "返回失败，不降级为未验证 DNS 成功。", "src/dns/mod.rs Resolver::resolve"),
    /// 当前域名被排除或未生成 Fake-IP，不能作为已分配的合成地址使用。
    FakeIpUnavailable => (1005, "未生成合成地址", "DNS", "当前域名被排除或未生成 Fake-IP，不能作为已分配的合成地址使用。", "allocate_fake_ip 的 synthetic_answer 返回 None。", "明确拒绝本次合成请求；不制造地址或映射。", "src/dns/mod.rs Resolver::allocate_fake_ip"),
    /// 资源预算、并发槽位、数量、缓冲区容量或安全计数器耗尽；不是普通规则未命中。
    ResourceLimit => (1006, "资源上限", "RESOURCE", "资源预算、并发槽位、数量、缓冲区容量或安全计数器耗尽；不是普通规则未命中。", "ByteBudget::reserve、try_reserve、长度数量检查或 checked 运算失败。", "拒绝超限操作，释放临时所有权；不会为继续执行绕过上限。", "src/budget.rs ByteBudget::reserve；src/lifecycle.rs Registry::create；src/ffi.rs OutputReservation"),
    /// 现有类别覆盖网络连接、读写以及持久化文件 I/O；它不专指远端拒绝，不应被解读为规则 REJECT。
    Network => (1007, "连接或读写失败", "IO", "现有类别覆盖网络连接、读写以及持久化文件 I/O；它不专指远端拒绝，不应被解读为规则 REJECT。", "io::Error 转换或网络及文件操作的 map_err 分支。", "当前 I/O 操作失败；事务文件未成功提交时不发布新快照。", "src/error.rs From<io::Error>；src/routing/store.rs；src/outbound/stream.rs"),
    /// TCP、TLS 外层连接或数据读写的既定操作期限耗尽；DNS 专用超时使用另一代码。
    NetworkTimeout => (1008, "网络操作超时", "NETWORK", "TCP、TLS 外层连接或数据读写的既定操作期限耗尽；DNS 专用超时使用另一代码。", "网络 timeout 分支。", "终止或拒绝本次操作，保留既定取消与清理流程。", "src/outbound/execution.rs；src/outbound/stream.rs；src/https.rs"),
    /// 取消令牌、关闭或提交前取消阻止操作继续；不表示成功完成。
    Cancelled => (1009, "操作已取消", "LIFECYCLE", "取消令牌、关闭或提交前取消阻止操作继续；不表示成功完成。", "出站取消分支、规则任务取消检查或 TUN 关闭取消。", "返回取消；已提交结果的回滚规则仍按原事务约定。", "src/outbound/stream.rs；src/routing/store.rs；src/tun/udp.rs"),
    /// TLS 握手、TLS 记录或上游证书校验未满足协议要求。
    Tls => (1010, "TLS 握手或校验失败", "TLS", "TLS 握手、TLS 记录或上游证书校验未满足协议要求。", "TLS 连接、协议或记录处理拒绝底层结果。", "终止 TLS 操作，不降级明文或跳过证书验证。", "src/https.rs；src/protocol/inner_tls.rs；src/protocol/stream.rs"),
    /// 证书、私钥、CA 材料或 REALITY 密钥协商材料无效；与常规上游在线 TLS 验证失败区分。
    Certificate => (1011, "证书或密钥无效", "TLS", "证书、私钥、CA 材料或 REALITY 密钥协商材料无效；与常规上游在线 TLS 验证失败区分。", "Authority 导入、签发、私钥解析失败，或 REALITY 密钥协商的共享密钥不具贡献性。", "拒绝该证书或密钥协商操作，不产生伪造成功的 Authority 或可用安全连接。", "src/mitm/authority.rs Authority；src/protocol/reality.rs hello"),
    /// 会话关闭、缺少所需资源、代次不符、版本竞争或独占文件锁忙；现有通用类别不承诺具体状态子原因。
    InvalidState => (1012, "状态不允许操作", "STATE", "会话关闭、缺少所需资源、代次不符、版本竞争或独占文件锁忙；现有通用类别不承诺具体状态子原因。", "状态、所有者、版本或锁占用检查失败。", "拒绝当前操作；不会继续使用未取得或已失效的对象。", "src/routing/store.rs；src/routing/debug.rs Session；src/dns/mod.rs restore_fake_ip"),
    /// 协议、配置版本或请求能力不在当前实现范围；不表示能够自动替换成直连。
    Unsupported => (1013, "能力不支持", "CAPABILITY", "协议、配置版本或请求能力不在当前实现范围；不表示能够自动替换成直连。", "支持范围校验、未实现功能入口或版本检查。", "明确拒绝，不伪造成功或静默能力降级。", "src/ffi.rs Command::TunStart；src/routing/cache.rs load；src/protocol/config.rs"),
    /// 锁中毒、运行时创建、任务 panic 或不能成立的内部状态；不吸收已知输入、资源及网络错误。
    Internal => (1014, "内部状态错误", "INTERNAL", "锁中毒、运行时创建、任务 panic 或不能成立的内部状态；不吸收已知输入、资源及网络错误。", "锁错误、运行时或内部一致性失败以及 FFI panic 隔离。", "返回失败或边界状态，不继续处理失效的内部状态。", "src/ffi.rs runtime；src/routing/background.rs；各模块锁转换"),
    /// 脚本执行失败，或结果不符合要求的 JSON/输出约束。
    Script => (1015, "脚本执行或输出失败", "SCRIPT", "脚本执行失败，或结果不符合要求的 JSON/输出约束。", "脚本引擎或脚本结果解析验证失败。", "终止该脚本处理；按原请求管线返回错误，不擅自 Continue。", "src/script.rs；src/traffic_script.rs；src/rewrite.rs"),
    /// 受限脚本执行超过既定时间预算。
    ScriptTimeout => (1016, "脚本执行超时", "SCRIPT", "受限脚本执行超过既定时间预算。", "脚本中断或超时判定。", "返回超时，不忽略超时后继续当作成功结果。", "src/script.rs"),
    /// 规则策略主动拒绝执行；这是策略结果，不是网络连接拒绝或执行故障。
    Rejected => (1017, "策略主动拒绝", "POLICY", "规则策略主动拒绝执行；这是策略结果，不是网络连接拒绝或执行故障。", "执行路由、传输拦截或规则策略为 Reject。", "阻止该请求或数据发送；纯规则匹配仍可成功返回 Reject 决策。", "src/outbound/execution.rs validate_decision；src/routing.rs check_transport；src/rewrite.rs"),
    /// C ABI 值句柄不存在、类型不符或已释放；不包含对象地址。
    InvalidHandle => (1018, "无效 ABI 句柄", "ABI", "C ABI 值句柄不存在、类型不符或已释放；不包含对象地址。", "ABI 在注册表查找所需类型失败。", "拒绝访问该对象，不解析为另一个对象。", "src/ffi.rs engine、udp_session、destroy"),
    /// 已执行操作的响应不能按既定协议编码；不保证当前业务操作未发生，不能盲目重试。
    SerializationFailed => (1019, "响应序列化失败", "ABI", "已执行操作的响应不能按既定协议编码；不保证当前业务操作未发生，不能盲目重试。", "serde_json::to_writer 的非预算序列化错误。", "不返回成功响应；释放未发布响应句柄，并记录边界失败。", "src/ffi.rs fluxa_engine_command"),
    /// XUDP 报文的会话、标志或来源不符合已绑定目标约束；不表示命中规则 REJECT。
    ProtocolViolation => (1020, "代理协议约束违反", "PROTOCOL", "XUDP 返回报文的会话、标志或来源违反单目标绑定约束，不能作为合法报文交付。", "decode 检测非零会话、未知标志或返回来源不等于绑定目标。", "拒绝报文并终止该失败连接；不放宽验证，也不声称规则 REJECT。", "src/protocol/datagram.rs decode"),
}
impl ErrorCode {
    /// 实际产生或传播该类别的 Rust 模块；不是协议分类名称。
    pub const fn modules(self) -> &'static str {
        match self {
            Self::InvalidInput => {
                "ffi、routing、protocol、dns、http、https、rewrite、subscription、tun"
            }
            Self::DnsTimeout => "dns、outbound::resolution",
            Self::DnsResponse => "dns、outbound::resolution、ffi",
            Self::DnsValidation => "dns",
            Self::FakeIpUnavailable => "dns",
            Self::ResourceLimit => {
                "budget、lifecycle、ffi、routing、dns、http、https、protocol、tun、mitm"
            }
            Self::Network => "error、http、https、outbound、protocol、routing::store、dns::service",
            Self::NetworkTimeout => "https、outbound、mitm、tun::udp",
            Self::Cancelled => "outbound、routing::store、routing::background、tun::udp",
            Self::Tls => "https、protocol、mitm",
            Self::Certificate => "mitm::authority、protocol::reality",
            Self::InvalidState => "ffi、routing、outbound、dns、protocol、tun",
            Self::Unsupported => "ffi、routing、protocol、dns、http、rewrite、tun",
            Self::Internal => "ffi、routing、dns、outbound、protocol、script、geoip、subscription",
            Self::Script => "script、traffic_script、rewrite",
            Self::ScriptTimeout => "script",
            Self::Rejected => "routing、outbound、rewrite",
            Self::InvalidHandle => "ffi",
            Self::SerializationFailed => "ffi",
            Self::ProtocolViolation => "protocol::datagram",
        }
    }
}
/// 当前没有允许公开的动态参数；私钥、Token、路径、配置和底层文本不能通过此类型发送。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ErrorParams {}
/// 稳定结构化失败；code 与 params 必填。未知字段、代码、参数拒绝解析。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ErrorInfo {
    pub code: ErrorCode,
    pub params: ErrorParams,
}
impl<'de> Deserialize<'de> for ErrorParams {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Empty;
        impl<'de> serde::de::Visitor<'de> for Empty {
            type Value = ErrorParams;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("空参数对象")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                if map.next_key::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::custom("错误参数不允许任何键"));
                }
                Ok(ErrorParams {})
            }
        }
        // 必须是对象；不能把空数组、null 或其他类型当作有效参数。
        deserializer.deserialize_map(Empty)
    }
}
impl<'de> Deserialize<'de> for ErrorInfo {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        enum Key {
            Code,
            Params,
        }
        impl<'de> Deserialize<'de> for Key {
            fn deserialize<D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> std::result::Result<Self, D::Error> {
                struct Keys;
                impl serde::de::Visitor<'_> for Keys {
                    type Value = Key;
                    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.write_str("已登记错误字段")
                    }
                    fn visit_str<E: serde::de::Error>(
                        self,
                        key: &str,
                    ) -> std::result::Result<Key, E> {
                        match key {
                            "code" => Ok(Key::Code),
                            "params" => Ok(Key::Params),
                            _ => Err(E::custom("未知错误字段")),
                        }
                    }
                }
                deserializer.deserialize_identifier(Keys)
            }
        }
        struct Fields;
        impl<'de> serde::de::Visitor<'de> for Fields {
            type Value = ErrorInfo;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("整数错误对象")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<ErrorInfo, A::Error> {
                let (mut code, mut params) = (None, None);
                while let Some(key) = map.next_key::<Key>()? {
                    match key {
                        Key::Code => {
                            if code.is_some() {
                                return Err(serde::de::Error::custom("重复 code 字段"));
                            }
                            code = Some(map.next_value()?);
                        }
                        Key::Params => {
                            if params.is_some() {
                                return Err(serde::de::Error::custom("重复 params 字段"));
                            }
                            params = Some(map.next_value()?);
                        }
                    }
                }
                Ok(ErrorInfo {
                    code: code.ok_or_else(|| serde::de::Error::custom("缺少 code 字段"))?,
                    params: params.ok_or_else(|| serde::de::Error::custom("缺少 params 字段"))?,
                })
            }
        }
        // 强制对象格式，保持机器协议的类型约束，不接受位置数组替代字段。
        deserializer.deserialize_map(Fields)
    }
}
impl ErrorInfo {
    pub const fn new(code: ErrorCode) -> Self {
        Self {
            code,
            params: ErrorParams {},
        }
    }
    /// 输入长度在解析前限制；不存在动态参数或递归错误链。
    pub fn from_json(bytes: &[u8]) -> crate::error::Result<Self> {
        if bytes.len() > MAX_ERROR_BYTES {
            return Err(crate::error::Error::ResourceLimit);
        }
        serde_json::from_slice(bytes).map_err(|_| crate::error::Error::InvalidInput)
    }
}
/// 错误对象仅包含两个固定字段，512 字节上限同时用于 ABI 固定缓冲区。
pub const MAX_ERROR_BYTES: usize = 512;

use std::{fmt, io};

#[path = "error/diagnostics.rs"]
pub mod diagnostics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidInput,
    DnsTimeout,
    DnsResponse,
    DnsValidation,
    FakeIpUnavailable,
    ResourceLimit,
    Network,
    NetworkTimeout,
    Cancelled,
    Tls,
    Certificate,
    InvalidState,
    Unsupported,
    Internal,
    Script,
    ScriptTimeout,
    Rejected,
    /// 代理报文违反会话或目标绑定约束，和正式规则 REJECT 分离。
    ProtocolViolation,
}
impl Error {
    /// 每个现有内部类别穷尽映射；不解析 Display 文本，不以枚举索引推导代码。
    pub const fn code(self) -> ErrorCode {
        match self {
            Self::InvalidInput => ErrorCode::InvalidInput,
            Self::DnsTimeout => ErrorCode::DnsTimeout,
            Self::DnsResponse => ErrorCode::DnsResponse,
            Self::DnsValidation => ErrorCode::DnsValidation,
            Self::FakeIpUnavailable => ErrorCode::FakeIpUnavailable,
            Self::ResourceLimit => ErrorCode::ResourceLimit,
            Self::Network => ErrorCode::Network,
            Self::NetworkTimeout => ErrorCode::NetworkTimeout,
            Self::Cancelled => ErrorCode::Cancelled,
            Self::Tls => ErrorCode::Tls,
            Self::Certificate => ErrorCode::Certificate,
            Self::InvalidState => ErrorCode::InvalidState,
            Self::Unsupported => ErrorCode::Unsupported,
            Self::Internal => ErrorCode::Internal,
            Self::Script => ErrorCode::Script,
            Self::ScriptTimeout => ErrorCode::ScriptTimeout,
            Self::Rejected => ErrorCode::Rejected,
            Self::ProtocolViolation => ErrorCode::ProtocolViolation,
        }
    }
    /// Rust 调用方取得结构化错误，不改变原 Result 的失败语义。
    pub const fn public_info(self) -> ErrorInfo {
        ErrorInfo::new(self.code())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FakeIpUnavailable => "当前域名未生成 Fake-IP 映射，请检查配置及排除项",
            Self::DnsTimeout => "DNS 查询超时",
            Self::DnsValidation => "DNSSEC 验证或验证查询失败",
            Self::DnsResponse => "DNS 上游返回错误状态",
            Self::InvalidInput => "请求或配置不合法",
            Self::ResourceLimit => "超过资源上限",
            Self::NetworkTimeout => "网络操作超时",
            Self::Cancelled => "网络操作已取消",
            Self::Network => "网络连接或读写失败",
            Self::Tls => "TLS 握手或上游证书校验失败",
            Self::Certificate => "证书或密钥不合法",
            Self::InvalidState => "资源状态不允许此操作",
            Self::Unsupported => "请求的能力尚未支持",
            Self::Script => "脚本执行失败或结果不是有效 JSON 数据",
            Self::Rejected => "请求被 Fluxa 规则拒绝",
            Self::ProtocolViolation => "代理报文违反会话或目标绑定约束",
            Self::ScriptTimeout => "脚本执行超时",
            Self::Internal => "内部资源错误",
        })
    }
}
impl std::error::Error for Error {}
// 底层错误链只检查类型，最多 16 层，避免恶意循环 source 或无界遍历。
const MAX_ERROR_SOURCE_DEPTH: usize = 16;
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        let mut mapped = match error.kind() {
            io::ErrorKind::TimedOut => Self::NetworkTimeout,
            // 系统明确报告内存不足时归入资源失败，不能误标成普通连接错误。
            io::ErrorKind::OutOfMemory => Self::ResourceLimit,
            _ => Self::Network,
        };
        let mut source = error
            .get_ref()
            .map(|cause| cause as &(dyn std::error::Error + 'static));
        for _ in 0..MAX_ERROR_SOURCE_DEPTH {
            let Some(cause) = source else {
                break;
            };
            if let Some(kernel) = cause.downcast_ref::<Self>() {
                mapped = *kernel;
                break;
            }
            source = cause.source();
        }
        diagnostics::record_io(&error, mapped.code());
        mapped
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// ABI v1 历史返回状态集中保存；它是兼容层，不是新整数错误协议的类别或优先级。
/// 成功与队列空不是错误；细分原因读取 ErrorCode，不用旧合并状态猜测。
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyAbiStatus {
    /// 操作成功；不表示后台尽力清理没有诊断。
    Success = 0,
    /// 旧 ABI 句柄不允许访问。
    InvalidHandle = 1,
    /// 旧 ABI 请求能力未支持。
    Unsupported = 2,
    /// 旧 ABI 内部状态失败。
    Internal = 3,
    /// 旧 TUN 输入不合法。
    InvalidInput = 4,
    /// 旧 TUN 资源或响应配额不足。
    ResourceLimit = 5,
    /// 旧 TUN 会话或资源状态不允许操作。
    InvalidState = 6,
    /// 旧 TUN 操作取消。
    Cancelled = 7,
    /// 旧 TUN 合并的网络、DNS、TLS 或处理失败；新协议不丢失细分类别。
    ProcessingFailed = 8,
    /// 旧 TUN 的拒绝；新协议区分策略拒绝与 XUDP 协议约束违反。
    Rejected = 9,
    /// 旧 TUN 合并的 DNS 与网络超时；新协议保留原错误类别。
    Timeout = 10,
    /// 接收队列暂时为空，不是执行错误。
    Empty = 11,
}
impl LegacyAbiStatus {
    pub const fn value(self) -> i32 {
        self as i32
    }
}
impl Error {
    /// 仅兼容旧数值协议；新的代码始终使用 code() 的唯一权威映射。
    pub const fn legacy_tun_status(self) -> LegacyAbiStatus {
        match self {
            Self::Unsupported => LegacyAbiStatus::Unsupported,
            Self::Internal => LegacyAbiStatus::Internal,
            Self::InvalidInput => LegacyAbiStatus::InvalidInput,
            Self::ResourceLimit => LegacyAbiStatus::ResourceLimit,
            Self::InvalidState => LegacyAbiStatus::InvalidState,
            Self::Cancelled => LegacyAbiStatus::Cancelled,
            Self::Rejected | Self::ProtocolViolation => LegacyAbiStatus::Rejected,
            Self::NetworkTimeout | Self::DnsTimeout => LegacyAbiStatus::Timeout,
            Self::Network
            | Self::DnsResponse
            | Self::DnsValidation
            | Self::FakeIpUnavailable
            | Self::Tls
            | Self::Certificate
            | Self::Script
            | Self::ScriptTimeout => LegacyAbiStatus::ProcessingFailed,
        }
    }
}
/// 零只表示同线程目前没有失败，不登记为一个错误代码。
pub const NO_ERROR_CODE: u16 = 0;
