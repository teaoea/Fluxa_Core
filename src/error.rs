//! 内核公开错误协议的唯一权威定义；与自然语言、调用方身份和枚举排列无关。
use serde::{Deserialize, Serialize};

/// 已发布旧号永久保留，不得赋予新含义；它们不是当前可返回的 ErrorCode。
pub const RETIRED_ERROR_CODES: &[u16] = &[
    // DNS 查询超时的历史编号，已退役，禁止复用。
    1002, // DNS 响应失败的历史编号，已退役，禁止复用。
    1003, // DNS 验证失败的历史编号，已退役，禁止复用。
    1004, // 未生成合成地址的历史编号，已退役，禁止复用。
    1005, // 资源上限的历史编号，已退役，禁止复用。
    1006, // 连接或读写失败的历史编号，已退役，禁止复用。
    1007, // 网络操作超时的历史编号，已退役，禁止复用。
    1008, // 操作已取消的历史编号，已退役，禁止复用。
    1009, // TLS 握手或校验失败的历史编号，已退役，禁止复用。
    1010, // 证书或密钥无效的历史编号，已退役，禁止复用。
    1011, // 状态不允许操作的历史编号，已退役，禁止复用。
    1012, // 能力不支持的历史编号，已退役，禁止复用。
    1013, // 内部状态错误的历史编号，已退役，禁止复用。
    1014, // 脚本执行或输出失败的历史编号，已退役，禁止复用。
    1015, // 脚本执行超时的历史编号，已退役，禁止复用。
    1016, // 策略主动拒绝的历史编号，已退役，禁止复用。
    1017, // 无效 ABI 句柄的历史编号，已退役，禁止复用。
    1018, // 响应序列化失败的历史编号，已退役，禁止复用。
    1019, // 代理协议约束违反的历史编号，已退役，禁止复用。
    1020,
];
const fn is_retired_code(value: u16) -> bool {
    let mut index = 0;
    while index < RETIRED_ERROR_CODES.len() {
        if RETIRED_ERROR_CODES[index] == value {
            return true;
        }
        index += 1;
    }
    false
}

/// 每项定义同时生成强类型代码、固定整数值和可审查契约；新增项必须更新文档快照。
macro_rules! define_codes {
    ($($(#[$doc:meta])* $variant:ident => ($id:literal, $name:literal, $category:literal,
        $detail:literal, $trigger:literal, $effect:literal, $source:literal)),+ $(,)?) => {
        #[repr(u16)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        /// 四位代码按类别分段；固定值显式登记，不通过枚举位置推导。
        pub enum ErrorCode {
            $($(#[$doc])* $variant = $id),+
        }
        const _: () = {
            $(assert!($id >= 1000 && $id <= 9999 && $id % 100 != 0);
              assert!(!is_retired_code($id));
              assert!(stringify!($id).len() == 4 && stringify!($id).as_bytes()[0] != b'0');)+
        };
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
            /// 无参数错误的固定 JSON；必填上下文代码返回 None，不制造无效错误对象。
            pub const fn json_bytes(self) -> Option<&'static [u8]> {
                if matches!(self, Self::ConfigUnknownField | Self::ConfigDuplicateField | Self::ConfigUnsupportedVersion | Self::DebugInvalidLifetime | Self::ResourceBudgetExceeded) { return None; }
                match self { $(Self::$variant => Some(concat!("{\"code\":", stringify!($id), ",\"params\":{}}").as_bytes())),+ }
            }
            pub const fn contract(self) -> ErrorContract {
                match self { $(Self::$variant => ErrorContract {
                    code: $id, name: $name, category: $category, modules: Self::$variant.modules(), detail: $detail,
                    trigger: $trigger, effect: $effect, source: $source,
                    params: Self::$variant.params_schema(), parameters:Self::$variant.parameters(), status: "有效",
                }),+ }
            }
        }
    };
}
/// 结构化日志级别与错误/状态代码分离；状态不应改变 Result 的成功语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Warn,
    Info,
    Error,
}
impl ErrorCode {
    pub const fn level(self) -> LogLevel {
        if matches!(self, Self::GeoipNotConfigured) {
            LogLevel::Warn
        } else if matches!(self, Self::GeoipReady | Self::GeoipNoMatch) {
            LogLevel::Info
        } else {
            LogLevel::Error
        }
    }
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
    pub parameters: &'static [ParameterSchema],
    pub source: &'static str,
    pub status: &'static str,
}
/// 机器可读参数契约；范围、可空性和安全要求均必须同步验证。
#[derive(Debug, Serialize)]
pub struct ParameterSchema {
    pub name: &'static str,
    pub kind: &'static str,
    pub required: bool,
    pub nullable: bool,
    pub minimum: u64,
    pub maximum: u64,
    pub meaning: &'static str,
    pub safety: &'static str,
}
define_codes! {
    /// 请求、配置或规则未满足类型、格式、范围及完整性约束；现有通用类别不能声称具体字段错误。
    InvalidInput => (1001, "无效输入", "INPUT", "请求、配置或规则未满足类型、格式、范围及完整性约束；现有通用类别不能声称具体字段错误。", "解析、字段校验、规则或协议帧校验拒绝输入。", "当前操作返回失败；原子配置提交在校验成功前不替换有效快照。", "src/routing/yaml.rs decode；src/routing/runtime.rs；src/protocol/config.rs"),
    /// 显式配置上游的单次或总期限耗尽，未取得符合要求的 DNS 结果。
    DnsTimeout => (5001, "DNS 查询超时", "DNS", "显式配置上游的单次或总期限耗尽，未取得符合要求的 DNS 结果。", "Resolver::resolve 的超时或 NetError::Timeout 分支。", "返回 DNS 失败，出站不能把此结果转换成直连成功。", "src/dns/mod.rs Resolver::resolve"),
    /// 上游响应、解析结果或地址结果不满足要求；合法 NXDOMAIN 可作为负响应返回，不必属于此错误。
    DnsResponse => (5002, "DNS 响应失败", "DNS", "上游响应、解析结果或地址结果不满足要求；合法 NXDOMAIN 可作为负响应返回，不必属于此错误。", "DNS 上游非成功状态、非允许负响应、无可用目标地址。", "拒绝当前解析或依赖该结果的连接；不冒充规则拒绝。", "src/dns/mod.rs Resolver::resolve；src/outbound/resolution.rs"),
    /// 启用 DNSSEC 的查询未通过验证或其验证查询失败，不保证底层只发生签名错误。
    DnsValidation => (5003, "DNS 验证失败", "DNS", "启用 DNSSEC 的查询未通过验证或其验证查询失败，不保证底层只发生签名错误。", "Resolver 的 DNSSEC 失败分支。", "返回失败，不降级为未验证 DNS 成功。", "src/dns/mod.rs Resolver::resolve"),
    /// 当前域名被排除或未生成 Fake-IP，不能作为已分配的合成地址使用。
    FakeIpUnavailable => (5004, "未生成合成地址", "DNS", "当前域名被排除或未生成 Fake-IP，不能作为已分配的合成地址使用。", "allocate_fake_ip 的 synthetic_answer 返回 None。", "明确拒绝本次合成请求；不制造地址或映射。", "src/dns/mod.rs Resolver::allocate_fake_ip"),
    /// 资源预算、并发槽位、数量、缓冲区容量或安全计数器耗尽；不是普通规则未命中。
    ResourceLimit => (8001, "资源上限", "RESOURCE", "资源预算、并发槽位、数量、缓冲区容量或安全计数器耗尽；不是普通规则未命中。", "ByteBudget::reserve、try_reserve、长度数量检查或 checked 运算失败。", "拒绝超限操作，释放临时所有权；不会为继续执行绕过上限。", "src/budget.rs ByteBudget::reserve；src/lifecycle.rs Registry::create；src/ffi.rs OutputReservation"),
    /// 现有类别覆盖网络连接、读写以及持久化文件 I/O；它不专指远端拒绝，不应被解读为规则 REJECT。
    Network => (6001, "连接或读写失败", "IO", "现有类别覆盖网络连接、读写以及持久化文件 I/O；它不专指远端拒绝，不应被解读为规则 REJECT。", "io::Error 转换或网络及文件操作的 map_err 分支。", "当前 I/O 操作失败；事务文件未成功提交时不发布新快照。", "src/error.rs From<io::Error>；src/routing/store.rs；src/outbound/stream.rs"),
    /// TCP、TLS 外层连接或数据读写的既定操作期限耗尽；DNS 专用超时使用另一代码。
    NetworkTimeout => (6002, "网络操作超时", "IO", "TCP、TLS 外层连接或数据读写的既定操作期限耗尽；DNS 专用超时使用另一代码。", "网络 timeout 分支。", "终止或拒绝本次操作，保留既定取消与清理流程。", "src/outbound/execution.rs；src/outbound/stream.rs；src/https.rs"),
    /// 取消令牌、关闭或提交前取消阻止操作继续；不表示成功完成。
    Cancelled => (9002, "操作已取消", "LIFECYCLE", "取消令牌、关闭或提交前取消阻止操作继续；不表示成功完成。", "出站取消分支、规则任务取消检查或 TUN 关闭取消。", "返回取消；已提交结果的回滚规则仍按原事务约定。", "src/outbound/stream.rs；src/routing/store.rs；src/tun/udp.rs"),
    /// TLS 握手、TLS 记录或上游证书校验未满足协议要求。
    Tls => (7001, "TLS 握手或校验失败", "TLS", "TLS 握手、TLS 记录或上游证书校验未满足协议要求。", "TLS 连接、协议或记录处理拒绝底层结果。", "终止 TLS 操作，不降级明文或跳过证书验证。", "src/https.rs；src/protocol/inner_tls.rs；src/protocol/stream.rs"),
    /// 证书、私钥、CA 材料或 REALITY 密钥协商材料无效；与常规上游在线 TLS 验证失败区分。
    Certificate => (7002, "证书或密钥无效", "TLS", "证书、私钥、CA 材料或 REALITY 密钥协商材料无效；与常规上游在线 TLS 验证失败区分。", "Authority 导入、签发、私钥解析失败，或 REALITY 密钥协商的共享密钥不具贡献性。", "拒绝该证书或密钥协商操作，不产生伪造成功的 Authority 或可用安全连接。", "src/mitm/authority.rs Authority；src/protocol/reality.rs hello"),
    /// 会话关闭、缺少所需资源、代次不符、版本竞争或独占文件锁忙；现有通用类别不承诺具体状态子原因。
    InvalidState => (9001, "状态不允许操作", "LIFECYCLE", "会话关闭、缺少所需资源、代次不符、版本竞争或独占文件锁忙；现有通用类别不承诺具体状态子原因。", "状态、所有者、版本或锁占用检查失败。", "拒绝当前操作；不会继续使用未取得或已失效的对象。", "src/routing/store.rs；src/routing/debug.rs Session；src/dns/mod.rs restore_fake_ip"),
    /// 协议、配置版本或请求能力不在当前实现范围；不表示能够自动替换成直连。
    Unsupported => (1021, "能力不支持", "INPUT", "协议、配置版本或请求能力不在当前实现范围；不表示能够自动替换成直连。", "支持范围校验、未实现功能入口或版本检查。", "明确拒绝，不伪造成功或静默能力降级。", "src/ffi.rs Command::TunStart；src/routing/cache.rs load；src/protocol/config.rs"),
    /// 锁中毒、运行时创建、任务 panic 或不能成立的内部状态；不吸收已知输入、资源及网络错误。
    Internal => (9901, "内部状态错误", "INTERNAL", "锁中毒、运行时创建、任务 panic 或不能成立的内部状态；不吸收已知输入、资源及网络错误。", "锁错误、运行时或内部一致性失败以及 FFI panic 隔离。", "返回失败或边界状态，不继续处理失效的内部状态。", "src/ffi.rs runtime；src/routing/background.rs；各模块锁转换"),
    /// 脚本执行失败，或结果不符合要求的 JSON/输出约束。
    Script => (1301, "脚本执行或输出失败", "SCRIPT", "脚本执行失败，或结果不符合要求的 JSON/输出约束。", "脚本引擎或脚本结果解析验证失败。", "终止该脚本处理；按原请求管线返回错误，不擅自 Continue。", "src/script.rs；src/traffic_script.rs；src/rewrite.rs"),
    /// 受限脚本执行超过既定时间预算。
    ScriptTimeout => (1302, "脚本执行超时", "SCRIPT", "受限脚本执行超过既定时间预算。", "脚本中断或超时判定。", "返回超时，不忽略超时后继续当作成功结果。", "src/script.rs"),
    /// 规则策略主动拒绝执行；这是策略结果，不是网络连接拒绝或执行故障。
    Rejected => (1101, "策略主动拒绝", "POLICY", "规则策略主动拒绝执行；这是策略结果，不是网络连接拒绝或执行故障。", "执行路由、传输拦截或规则策略为 Reject。", "阻止该请求或数据发送；纯规则匹配仍可成功返回 Reject 决策。", "src/outbound/execution.rs validate_decision；src/routing.rs check_transport；src/rewrite.rs"),
    /// C ABI 值句柄不存在、类型不符或已释放；不包含对象地址。
    InvalidHandle => (1201, "无效 ABI 句柄", "ABI", "C ABI 值句柄不存在、类型不符或已释放；不包含对象地址。", "ABI 在注册表查找所需类型失败。", "拒绝访问该对象，不解析为另一个对象。", "src/ffi.rs engine、udp_session、destroy"),
    /// 已执行操作的响应不能按既定协议编码；不保证当前业务操作未发生，不能盲目重试。
    SerializationFailed => (1202, "响应序列化失败", "ABI", "已执行操作的响应不能按既定协议编码；不保证当前业务操作未发生，不能盲目重试。", "serde_json::to_writer 的非预算序列化错误。", "不返回成功响应；释放未发布响应句柄，并记录边界失败。", "src/ffi.rs fluxa_engine_command"),
    /// XUDP 报文的会话、标志或来源不符合已绑定目标约束；不表示命中规则 REJECT。
    ProtocolViolation => (1401, "代理协议约束违反", "PROTOCOL", "XUDP 返回报文的会话、标志或来源违反单目标绑定约束，不能作为合法报文交付。", "decode 检测非零会话、未知标志或返回来源不等于绑定目标。", "拒绝报文并终止该失败连接；不放宽验证，也不声称规则 REJECT。", "src/protocol/datagram.rs decode"),
    /// YAML 事件解析器拒绝语法，未构造有效配置。
    ConfigYamlSyntax => (2001, "YAML 语法错误", "CONFIG", "YAML 事件解析器拒绝语法，未构造有效配置。", "Parser::next_token 返回解析错误。", "拒绝本次配置；不发布部分产物。", "src/routing/yaml.rs Decoder::next"),
    /// 严格配置反序列化遇到未声明字段；字段名只接受有界 ASCII 标识。
    ConfigUnknownField => (2002, "未知配置字段", "CONFIG", "严格配置反序列化遇到未声明字段；字段名只接受有界 ASCII 标识。", "serde unknown_field 类型回调，不解析错误字符串。", "拒绝整个输入，保留原有效配置。", "src/routing/yaml.rs de::Error::unknown_field"),
    /// 同一 YAML 映射出现重复字段，不允许后项覆盖前项。
    ConfigDuplicateField => (2003, "重复配置字段", "CONFIG", "同一 YAML 映射出现重复字段，不允许后项覆盖前项。", "Mapping::next_key_seed 检测字段重复。", "拒绝整个输入，不部分接受。", "src/routing/yaml.rs Mapping::next_key_seed"),
    /// GenerationInput 声明的格式版本不是当前支持的版本 1。
    ConfigUnsupportedVersion => (2004, "规则来源配置版本不支持", "CONFIG", "GenerationInput 声明的格式版本不是当前支持的版本 1。", "GenerationInput::from_yaml 检查 format_version。", "拒绝该来源配置，不放宽解析规则。", "src/routing/runtime.rs GenerationInput::from_yaml"),
    /// 调试规则生命周期缺失、为零或超过 3600 秒。
    DebugInvalidLifetime => (4001, "非法调试生命周期", "DEBUG", "调试规则生命周期缺失、为零或超过 3600 秒。", "Session::replace_at 逐条校验生命周期。", "拒绝整次替换，原调试方案保持有效。", "src/routing/debug.rs Session::replace_at"),
    /// 已经关闭的调试会话不允许替换、清空或读取状态。
    DebugSessionClosed => (4002, "调试会话已关闭", "DEBUG", "已经关闭的调试会话不允许替换、清空或读取状态。", "Session 的 closed 状态检查。", "拒绝操作，不重新开启会话。", "src/routing/debug.rs Session"),
    /// 申请量超过静态预算，或当前占用加申请量溢出或超过限额。
    ResourceBudgetExceeded => (8002, "预算准入失败", "RESOURCE", "申请量超过静态预算，或当前占用加申请量溢出或超过限额。", "ByteBudget::reserve 原子准入失败。", "不增加预算占用，不构造租约。", "src/budget.rs ByteBudget::reserve"),
    /// 操作系统明确返回 ConnectionRefused；与规则主动拒绝区分。
    NetworkConnectRefused => (6003, "连接被远端拒绝", "IO", "操作系统明确返回 ConnectionRefused；与规则主动拒绝区分。", "Error::classify_io 的类型化 I/O kind 检查。", "当前连接失败，不转换为策略 REJECT 或成功。", "src/error.rs Error::classify_io"),

    /// 显式 geoip_url 为空、超长或不是合法下载地址。
    GeoipInvalidUrl => (2401, "GeoIP URL 格式非法", "GEOIP", "显式 geoip_url 为空、超长或不是合法下载地址。", "Config::validate 拒绝 URL 格式、主机、凭据、片段或控制符。", "ERROR，拒绝整次配置并保留旧来源和网络配置。", "src/geoip.rs Config::validate"),
    /// GeoIP 数据源使用不允许的协议，当前仅接受 HTTPS。
    GeoipUrlScheme => (2402, "GeoIP URL 协议不允许", "GEOIP", "GeoIP 数据源使用不允许的协议，当前仅接受 HTTPS。", "URL 已解析但 scheme 不是 https。", "ERROR，拒绝整次配置，不降级 HTTP。", "src/geoip.rs Config::validate"),
    /// 来源 DNS、连接、TLS、HTTP 状态或其他访问失败，未取得完整下载结果。
    GeoipDownloadFailed => (2403, "GeoIP 数据库下载失败", "GEOIP", "来源 DNS、连接、TLS、HTTP 状态或其他访问失败，未取得完整下载结果。", "HTTPS 数据源访问失败或 GET 最终状态非 200。", "ERROR，下载返回 Err，底层类型保留在固定诊断环；不默认下载或静默启用旧库。", "src/geoip.rs access_error；src/https.rs execute_inner"),
    /// 数据库下载超出共用 HTTPS 的 60 秒总期限。
    GeoipDownloadTimeout => (2404, "GeoIP 数据库下载超时", "GEOIP", "数据库下载超出共用 HTTPS 的 60 秒总期限。", "下载流程返回 NetworkTimeout。", "ERROR，取消下载，返回 Err；临时预算释放。", "src/geoip.rs access_error；src/https.rs download_database"),
    /// 下载数据不是可识别的原始 MMDB 格式或使用不支持的内容编码。
    GeoipInvalidDatabase => (2405, "GeoIP 数据库格式非法", "GEOIP", "下载数据不是可识别的原始 MMDB 格式或使用不支持的内容编码。", "正文为空、非 identity 编码、缺失 MMDB 元数据标记或非 MMDB 内容。", "ERROR，拒绝加载，不发布无效数据库。", "src/geoip.rs Database::from_bytes"),
    /// 响应截断、声明长度不一致或 MMDB 结构初始化验证失败。
    GeoipIntegrityFailed => (2406, "GeoIP 数据库完整性验证失败", "GEOIP", "响应截断、声明长度不一致或 MMDB 结构初始化验证失败。", "下载分帧读到提前 EOF；或已初始化 MMDB 的可达记录结构验证失败。", "ERROR，拒绝加载；此校验不等于来源签名或预期 SHA-256 验证。", "src/http.rs read_body_bounded；src/geoip.rs Database::from_bytes"),
    /// 数据库长度、下载分配或全局数据库配额超过限额。
    GeoipResourceLimit => (2407, "GeoIP 数据库超过资源预算", "GEOIP", "数据库长度、下载分配或全局数据库配额超过限额。", "256 MiB 单库上限或 512 MiB 总预算准入失败、受控分配失败。", "ERROR，拒绝下载或加载，归还临时租约；不绕过原预算。", "src/geoip.rs resource_error、Service::refresh；src/http.rs read_database_body"),
    /// 用户配置了 GeoIP，但当前来源没有可用数据库或数据库状态锁故障。
    GeoipInitializationFailed => (2408, "GeoIP 数据库初始化失败", "GEOIP", "用户配置了 GeoIP，但当前来源没有可用数据库或数据库状态锁故障。", "查询前数据库尚未加载、已关闭，或数据库状态锁不可用。", "ERROR，查询返回 Err，不切换旧来源、不以 Disabled 代替明确配置失败。", "src/geoip.rs Service::database"),
    /// 数据库已启用，但查询或记录字段解码执行失败。
    GeoipQueryFailed => (2409, "GeoIP 查询执行失败", "GEOIP", "数据库已启用，但查询或记录字段解码执行失败。", "Reader::lookup、decode_path 或国家代码格式校验失败。", "ERROR，返回 Err，不冒充合法无匹配记录。", "src/geoip.rs Database::lookup"),
    /// 没有提供 geoip_url，表示用户未启用可选 GeoIP 功能。
    GeoipNotConfigured => (2410, "GeoIP 未配置并关闭", "GEOIP", "没有提供 geoip_url，表示用户未启用可选 GeoIP 功能。", "配置缺少该字段或显式为 null。", "WARN + Disabled，不初始化 MMDB、不下载、不查询目标 DNS；返回状态代码而非执行 Err。", "src/geoip.rs Status::disabled；src/ffi.rs GeoipLookup"),
    /// 当前来源的 MMDB 已加载，可进行查询。
    GeoipReady => (2411, "GeoIP 已启用", "GEOIP", "当前来源的 MMDB 已加载，可进行查询。", "完整下载初始化成功，或查询取得有效记录。", "INFO + Ready，返回成功状态代码，不生成 error_info。", "src/geoip.rs Status::ready；src/ffi.rs GeoipRefresh"),
    /// 有效数据库完成查询，但合法目标 IP 没有归属地记录。
    GeoipNoMatch => (2412, "GeoIP 合法未匹配", "GEOIP", "有效数据库完成查询，但合法目标 IP 没有归属地记录。", "MMDB lookup 返回无记录。", "非错误，返回 no-match 和状态代码，不生成 error_info 或默认归属地。", "src/geoip.rs Status::no_match；src/ffi.rs GeoipLookup"),
}
impl ErrorCode {
    pub const fn parameters(self) -> &'static [ParameterSchema] {
        match self {
            Self::ConfigUnknownField | Self::ConfigDuplicateField => &[ParameterSchema {
                name: "field",
                kind: "ascii_identifier",
                required: true,
                nullable: true,
                minimum: 1,
                maximum: 32,
                meaning: "失败的字段名，不含字段值；null 表示安全省略",
                safety: "只接受 ASCII 字母、数字及下划线",
            }],
            Self::ConfigUnsupportedVersion => &[
                ParameterSchema {
                    name: "actual",
                    kind: "u32",
                    required: true,
                    nullable: false,
                    minimum: 0,
                    maximum: u32::MAX as u64,
                    meaning: "实际来源配置版本，必须不等于 1",
                    safety: "固定宽度整数",
                },
                ParameterSchema {
                    name: "expected",
                    kind: "u32",
                    required: true,
                    nullable: false,
                    minimum: 1,
                    maximum: 1,
                    meaning: "支持的格式版本",
                    safety: "固定为 1",
                },
            ],
            Self::DebugInvalidLifetime => &[
                ParameterSchema {
                    name: "actual",
                    kind: "u64",
                    required: true,
                    nullable: true,
                    minimum: 0,
                    maximum: u64::MAX,
                    meaning: "实际生命周期；null 表示缺失，只接受 0 或大于 3600",
                    safety: "不包含规则正文",
                },
                ParameterSchema {
                    name: "limit",
                    kind: "u64",
                    required: true,
                    nullable: false,
                    minimum: 3600,
                    maximum: 3600,
                    meaning: "生命周期上限，单位秒",
                    safety: "固定为 3600",
                },
            ],
            Self::ResourceBudgetExceeded => &[
                ParameterSchema {
                    name: "actual",
                    kind: "u64",
                    required: true,
                    nullable: false,
                    minimum: 0,
                    maximum: u64::MAX,
                    meaning: "本次申请量；单位与预算一致，不声称进程 RSS",
                    safety: "不包含地址、配置或对象标识",
                },
                ParameterSchema {
                    name: "limit",
                    kind: "u64",
                    required: true,
                    nullable: false,
                    minimum: 0,
                    maximum: u64::MAX,
                    meaning: "该预算的静态限额",
                    safety: "固定宽度整数",
                },
            ],
            Self::ConfigYamlSyntax
            | Self::InvalidInput
            | Self::Unsupported
            | Self::DnsTimeout
            | Self::DnsResponse
            | Self::DnsValidation
            | Self::FakeIpUnavailable
            | Self::ResourceLimit
            | Self::Network
            | Self::NetworkTimeout
            | Self::Cancelled
            | Self::Tls
            | Self::Certificate
            | Self::InvalidState
            | Self::Internal
            | Self::Script
            | Self::ScriptTimeout
            | Self::Rejected
            | Self::InvalidHandle
            | Self::SerializationFailed
            | Self::ProtocolViolation
            | Self::GeoipNotConfigured
            | Self::GeoipInitializationFailed
            | Self::GeoipNoMatch
            | Self::GeoipDownloadFailed
            | Self::GeoipInvalidDatabase
            | Self::GeoipReady
            | Self::GeoipInvalidUrl
            | Self::GeoipUrlScheme
            | Self::GeoipDownloadTimeout
            | Self::GeoipIntegrityFailed
            | Self::GeoipResourceLimit
            | Self::GeoipQueryFailed
            | Self::DebugSessionClosed
            | Self::NetworkConnectRefused => &[],
        }
    }
    pub const fn params_schema(self) -> &'static str {
        match self {
            Self::ConfigUnknownField | Self::ConfigDuplicateField => {
                "field：必填，ASCII 标识字符串 1–32 字节或 null（安全省略）；不接受路径、配置正文或任意对象"
            }
            Self::ConfigUnsupportedVersion => "actual：必填 u32；expected：必填且为 1",
            Self::DebugInvalidLifetime => {
                "actual：必填 u64 或 null（缺失）；limit：必填且为 3600；只允许缺失、0 或超限的失败值"
            }
            Self::ResourceBudgetExceeded => {
                "actual：必填 u64，申请量；limit：必填 u64，预算上限；准入因并发占用也可能失败"
            }
            Self::ConfigYamlSyntax
            | Self::InvalidInput
            | Self::Unsupported
            | Self::DnsTimeout
            | Self::DnsResponse
            | Self::DnsValidation
            | Self::FakeIpUnavailable
            | Self::ResourceLimit
            | Self::Network
            | Self::NetworkTimeout
            | Self::Cancelled
            | Self::Tls
            | Self::Certificate
            | Self::InvalidState
            | Self::Internal
            | Self::Script
            | Self::ScriptTimeout
            | Self::Rejected
            | Self::InvalidHandle
            | Self::SerializationFailed
            | Self::ProtocolViolation
            | Self::GeoipNotConfigured
            | Self::GeoipInitializationFailed
            | Self::GeoipNoMatch
            | Self::GeoipDownloadFailed
            | Self::GeoipInvalidDatabase
            | Self::GeoipReady
            | Self::GeoipInvalidUrl
            | Self::GeoipUrlScheme
            | Self::GeoipDownloadTimeout
            | Self::GeoipIntegrityFailed
            | Self::GeoipResourceLimit
            | Self::GeoipQueryFailed
            | Self::DebugSessionClosed
            | Self::NetworkConnectRefused => "{}（不接受任何参数键）",
        }
    }
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
            Self::GeoipNotConfigured
            | Self::GeoipInitializationFailed
            | Self::GeoipNoMatch
            | Self::GeoipDownloadFailed
            | Self::GeoipInvalidDatabase
            | Self::GeoipReady
            | Self::GeoipInvalidUrl
            | Self::GeoipUrlScheme
            | Self::GeoipDownloadTimeout
            | Self::GeoipIntegrityFailed
            | Self::GeoipResourceLimit
            | Self::GeoipQueryFailed => "geoip、ffi",
            Self::ConfigYamlSyntax | Self::ConfigUnknownField | Self::ConfigDuplicateField => {
                "routing::yaml"
            }
            Self::ConfigUnsupportedVersion => "routing::runtime",
            Self::DebugInvalidLifetime | Self::DebugSessionClosed => "routing::debug",
            Self::ResourceBudgetExceeded => "budget",
            Self::NetworkConnectRefused => "error、outbound",
        }
    }
}
/// 字段名仅保留 ASCII 标识，不公开任意原文；固定 32 字节，无堆分配。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldName {
    bytes: [u8; 32],
    len: u8,
}
impl FieldName {
    pub fn new(value: &str) -> Option<Self> {
        if value.is_empty()
            || value.len() > 32
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return None;
        }
        let mut bytes = [0; 32];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Some(Self {
            bytes,
            len: value.len() as u8,
        })
    }
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).expect("构造器保证 ASCII")
    }
}
impl Serialize for FieldName {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for FieldName {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Field;
        impl serde::de::Visitor<'_> for Field {
            type Value = FieldName;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("1–32 字节 ASCII 字段标识")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<FieldName, E> {
                FieldName::new(v).ok_or_else(|| E::custom("字段标识不符合安全限制"))
            }
        }
        deserializer.deserialize_str(Field)
    }
}
/// 上下文按错误代码校验；没有自由 JSON 对象、敏感原文或无界字符串。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<FieldName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<u64>,
}
/// 固定上下文值，不分配堆内存；code 与 params 作为一个整体校验。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorInfo {
    pub code: ErrorCode,
    pub params: ErrorParams,
}
impl Serialize for ErrorInfo {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        if !self.valid() {
            return Err(serde::ser::Error::custom("错误代码与参数不一致"));
        }
        let mut object = serializer.serialize_struct("ErrorInfo", 2)?;
        object.serialize_field("code", &self.code.value())?;
        if matches!(
            self.code,
            ErrorCode::ConfigUnknownField | ErrorCode::ConfigDuplicateField
        ) {
            #[derive(Serialize)]
            struct Field {
                field: Option<FieldName>,
            }
            object.serialize_field(
                "params",
                &Field {
                    field: self.params.field,
                },
            )?;
        } else if self.code == ErrorCode::DebugInvalidLifetime {
            #[derive(Serialize)]
            struct Lifetime {
                actual: Option<u64>,
                limit: u64,
            }
            object.serialize_field(
                "params",
                &Lifetime {
                    actual: self.params.actual,
                    limit: 3600,
                },
            )?;
        } else {
            object.serialize_field("params", &self.params)?;
        }
        object.end()
    }
}
/// 只接受对象，避免 derive 结构体反序列化同时接受位置数组。
fn object_only<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<T, D::Error> {
    struct Object<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Object<T> {
        type Value = T;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("结构化对象")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            map: A,
        ) -> std::result::Result<T, A::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }
    d.deserialize_map(Object(std::marker::PhantomData))
}
impl<'de> Deserialize<'de> for ErrorInfo {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        // 区分缺失与显式 null；防止可选字段把缺失的必填参数悄悄接受。
        fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
            d: D,
        ) -> std::result::Result<Option<Option<T>>, D::Error> {
            Ok(Some(Option::<T>::deserialize(d)?))
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Params {
            #[serde(default, deserialize_with = "present")]
            field: Option<Option<FieldName>>,
            #[serde(default, deserialize_with = "present")]
            actual: Option<Option<u64>>,
            #[serde(default, deserialize_with = "present")]
            limit: Option<Option<u64>>,
            #[serde(default, deserialize_with = "present")]
            expected: Option<Option<u64>>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            code: ErrorCode,
            #[serde(deserialize_with = "object_only")]
            params: Params,
        }
        let wire: Wire = object_only(deserializer)?;
        let p = wire.params;
        let shape = (
            p.field.is_some(),
            p.actual.is_some(),
            p.limit.is_some(),
            p.expected.is_some(),
        );
        let expected = match wire.code {
            ErrorCode::ConfigUnknownField | ErrorCode::ConfigDuplicateField => {
                (true, false, false, false)
            }
            ErrorCode::ConfigUnsupportedVersion => (false, true, false, true),
            ErrorCode::DebugInvalidLifetime | ErrorCode::ResourceBudgetExceeded => {
                (false, true, true, false)
            }
            ErrorCode::ConfigYamlSyntax
            | ErrorCode::InvalidInput
            | ErrorCode::Unsupported
            | ErrorCode::DnsTimeout
            | ErrorCode::DnsResponse
            | ErrorCode::DnsValidation
            | ErrorCode::FakeIpUnavailable
            | ErrorCode::ResourceLimit
            | ErrorCode::Network
            | ErrorCode::NetworkTimeout
            | ErrorCode::Cancelled
            | ErrorCode::Tls
            | ErrorCode::Certificate
            | ErrorCode::InvalidState
            | ErrorCode::Internal
            | ErrorCode::Script
            | ErrorCode::ScriptTimeout
            | ErrorCode::Rejected
            | ErrorCode::InvalidHandle
            | ErrorCode::SerializationFailed
            | ErrorCode::ProtocolViolation
            | ErrorCode::DebugSessionClosed
            | ErrorCode::GeoipNotConfigured
            | ErrorCode::GeoipInitializationFailed
            | ErrorCode::GeoipNoMatch
            | ErrorCode::GeoipDownloadFailed
            | ErrorCode::GeoipInvalidDatabase
            | ErrorCode::GeoipReady
            | ErrorCode::GeoipInvalidUrl
            | ErrorCode::GeoipUrlScheme
            | ErrorCode::GeoipDownloadTimeout
            | ErrorCode::GeoipIntegrityFailed
            | ErrorCode::GeoipResourceLimit
            | ErrorCode::GeoipQueryFailed
            | ErrorCode::NetworkConnectRefused => (false, false, false, false),
        };
        let info = Self {
            code: wire.code,
            params: ErrorParams {
                field: p.field.flatten(),
                actual: p.actual.flatten(),
                limit: p.limit.flatten(),
                expected: p.expected.flatten(),
            },
        };
        if shape != expected || !info.valid() {
            return Err(serde::de::Error::custom("错误代码与参数不一致"));
        }
        Ok(info)
    }
}
impl ErrorInfo {
    pub const fn new(code: ErrorCode) -> Self {
        Self {
            code,
            params: ErrorParams {
                field: None,
                actual: None,
                limit: None,
                expected: None,
            },
        }
    }
    pub fn valid(&self) -> bool {
        let p = self.params;
        match self.code {
            ErrorCode::ConfigUnknownField | ErrorCode::ConfigDuplicateField => {
                p.actual.is_none() && p.limit.is_none() && p.expected.is_none()
            }
            ErrorCode::ConfigUnsupportedVersion => {
                p.field.is_none()
                    && p.limit.is_none()
                    && p.expected == Some(1)
                    && p.actual.is_some_and(|v| v <= u64::from(u32::MAX) && v != 1)
            }
            ErrorCode::DebugInvalidLifetime => {
                p.field.is_none()
                    && p.expected.is_none()
                    && p.limit == Some(3600)
                    && p.actual.is_none_or(|v| v == 0 || v > 3600)
            }
            ErrorCode::ResourceBudgetExceeded => {
                p.field.is_none() && p.expected.is_none() && p.actual.is_some() && p.limit.is_some()
            }
            ErrorCode::ConfigYamlSyntax
            | ErrorCode::InvalidInput
            | ErrorCode::Unsupported
            | ErrorCode::DnsTimeout
            | ErrorCode::DnsResponse
            | ErrorCode::DnsValidation
            | ErrorCode::FakeIpUnavailable
            | ErrorCode::ResourceLimit
            | ErrorCode::Network
            | ErrorCode::NetworkTimeout
            | ErrorCode::Cancelled
            | ErrorCode::Tls
            | ErrorCode::Certificate
            | ErrorCode::InvalidState
            | ErrorCode::Internal
            | ErrorCode::Script
            | ErrorCode::ScriptTimeout
            | ErrorCode::Rejected
            | ErrorCode::InvalidHandle
            | ErrorCode::SerializationFailed
            | ErrorCode::ProtocolViolation
            | ErrorCode::DebugSessionClosed
            | ErrorCode::GeoipNotConfigured
            | ErrorCode::GeoipInitializationFailed
            | ErrorCode::GeoipNoMatch
            | ErrorCode::GeoipDownloadFailed
            | ErrorCode::GeoipInvalidDatabase
            | ErrorCode::GeoipReady
            | ErrorCode::GeoipInvalidUrl
            | ErrorCode::GeoipUrlScheme
            | ErrorCode::GeoipDownloadTimeout
            | ErrorCode::GeoipIntegrityFailed
            | ErrorCode::GeoipResourceLimit
            | ErrorCode::GeoipQueryFailed
            | ErrorCode::NetworkConnectRefused => p == ErrorParams::default(),
        }
    }
    pub fn from_json(bytes: &[u8]) -> crate::error::Result<Self> {
        if bytes.len() > MAX_ERROR_BYTES {
            return Err(Error::ResourceLimit);
        }
        serde_json::from_slice(bytes).map_err(|_| Error::InvalidInput)
    }
}
/// 错误对象仅包含两个固定字段，512 字节上限同时用于 ABI 固定缓冲区。
pub const MAX_ERROR_BYTES: usize = 512;

use std::{fmt, io};

#[path = "error/diagnostics.rs"]
pub mod diagnostics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// YAML 事件语法不合法。
    ConfigYamlSyntax,
    /// 未声明的配置字段，安全字段名有界。
    ConfigUnknownField(Option<FieldName>),
    /// 同一配置映射重复字段。
    ConfigDuplicateField(Option<FieldName>),
    /// 不支持的规则来源配置版本。
    ConfigUnsupportedVersion(u32),
    /// 缺失或越界的调试生命周期，单位秒。
    DebugInvalidLifetime(Option<u64>),
    /// 已关闭的调试会话。
    DebugSessionClosed,
    /// 原子预算申请失败；只保存申请量及限额。
    ResourceBudgetExceeded {
        actual: u64,
        limit: u64,
    },
    /// 操作系统明确拒绝连接，不是规则 REJECT。
    NetworkConnectRefused,

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
    /// 显式 geoip_url 为空、超长或不是合法下载地址。
    GeoipInvalidUrl,
    /// GeoIP 数据源使用不允许的协议，当前仅接受 HTTPS。
    GeoipUrlScheme,
    /// 来源 DNS、连接、TLS、HTTP 状态或其他访问失败，未取得完整下载结果。
    GeoipDownloadFailed,
    /// 数据库下载超出共用 HTTPS 的 60 秒总期限。
    GeoipDownloadTimeout,
    /// 下载数据不是可识别的原始 MMDB 格式或使用不支持的内容编码。
    GeoipInvalidDatabase,
    /// 响应截断、声明长度不一致或 MMDB 结构初始化验证失败。
    GeoipIntegrityFailed,
    /// 数据库长度、下载分配或全局数据库配额超过限额。
    GeoipResourceLimit,
    /// 用户配置了 GeoIP，但当前来源没有可用数据库或数据库状态锁故障。
    GeoipInitializationFailed,
    /// 数据库已启用，但查询或记录字段解码执行失败。
    GeoipQueryFailed,
}
impl Error {
    /// 每个现有内部类别穷尽映射；不解析 Display 文本，不以枚举索引推导代码。
    pub const fn code(self) -> ErrorCode {
        match self {
            Self::ConfigYamlSyntax => ErrorCode::ConfigYamlSyntax,
            Self::ConfigUnknownField(_) => ErrorCode::ConfigUnknownField,
            Self::ConfigDuplicateField(_) => ErrorCode::ConfigDuplicateField,
            Self::ConfigUnsupportedVersion(_) => ErrorCode::ConfigUnsupportedVersion,
            Self::DebugInvalidLifetime(_) => ErrorCode::DebugInvalidLifetime,
            Self::DebugSessionClosed => ErrorCode::DebugSessionClosed,
            Self::ResourceBudgetExceeded { .. } => ErrorCode::ResourceBudgetExceeded,
            Self::NetworkConnectRefused => ErrorCode::NetworkConnectRefused,
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
            Self::GeoipInvalidUrl => ErrorCode::GeoipInvalidUrl,
            Self::GeoipUrlScheme => ErrorCode::GeoipUrlScheme,
            Self::GeoipDownloadFailed => ErrorCode::GeoipDownloadFailed,
            Self::GeoipDownloadTimeout => ErrorCode::GeoipDownloadTimeout,
            Self::GeoipInvalidDatabase => ErrorCode::GeoipInvalidDatabase,
            Self::GeoipIntegrityFailed => ErrorCode::GeoipIntegrityFailed,
            Self::GeoipResourceLimit => ErrorCode::GeoipResourceLimit,
            Self::GeoipInitializationFailed => ErrorCode::GeoipInitializationFailed,
            Self::GeoipQueryFailed => ErrorCode::GeoipQueryFailed,
        }
    }
    /// Rust 调用方取得结构化错误，不改变原 Result 的失败语义。
    pub const fn public_info(self) -> ErrorInfo {
        let mut info = ErrorInfo::new(self.code());
        match self {
            Self::ConfigUnknownField(field) | Self::ConfigDuplicateField(field) => {
                info.params.field = field
            }
            Self::ConfigUnsupportedVersion(actual) => {
                info.params.actual = Some(actual as u64);
                info.params.expected = Some(1);
            }
            Self::DebugInvalidLifetime(actual) => {
                info.params.actual = actual;
                info.params.limit = Some(3600);
            }
            Self::ResourceBudgetExceeded { actual, limit } => {
                info.params.actual = Some(actual);
                info.params.limit = Some(limit);
            }
            Self::ConfigYamlSyntax
            | Self::DebugSessionClosed
            | Self::NetworkConnectRefused
            | Self::InvalidInput
            | Self::DnsTimeout
            | Self::DnsResponse
            | Self::DnsValidation
            | Self::FakeIpUnavailable
            | Self::ResourceLimit
            | Self::Network
            | Self::NetworkTimeout
            | Self::Cancelled
            | Self::Tls
            | Self::Certificate
            | Self::InvalidState
            | Self::Unsupported
            | Self::Internal
            | Self::Script
            | Self::ScriptTimeout
            | Self::Rejected
            | Self::ProtocolViolation
            | Self::GeoipInvalidUrl
            | Self::GeoipUrlScheme
            | Self::GeoipDownloadFailed
            | Self::GeoipDownloadTimeout
            | Self::GeoipInvalidDatabase
            | Self::GeoipIntegrityFailed
            | Self::GeoipResourceLimit
            | Self::GeoipInitializationFailed
            | Self::GeoipQueryFailed => (),
        }
        info
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ConfigYamlSyntax => "YAML 语法不合法",
            Self::ConfigUnknownField(_) => "配置包含未知字段",
            Self::ConfigDuplicateField(_) => "配置包含重复字段",
            Self::ConfigUnsupportedVersion(_) => "规则来源配置版本不支持",
            Self::DebugInvalidLifetime(_) => "调试生命周期必须为 1–3600 秒",
            Self::DebugSessionClosed => "调试会话已关闭",
            Self::ResourceBudgetExceeded { .. } => "预算准入失败",
            Self::NetworkConnectRefused => "连接被远端拒绝",

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
            Self::GeoipInvalidUrl => "GeoIP URL 格式非法",
            Self::GeoipUrlScheme => "GeoIP URL 协议不允许",
            Self::GeoipDownloadFailed => "GeoIP 数据库下载失败",
            Self::GeoipDownloadTimeout => "GeoIP 数据库下载超时",
            Self::GeoipInvalidDatabase => "GeoIP 数据库格式非法",
            Self::GeoipIntegrityFailed => "GeoIP 数据库完整性验证失败",
            Self::GeoipResourceLimit => "GeoIP 数据库超过资源预算",
            Self::GeoipInitializationFailed => "GeoIP 数据库初始化失败",
            Self::GeoipQueryFailed => "GeoIP 查询执行失败",
            Self::ScriptTimeout => "脚本执行超时",
            Self::Internal => "内部资源错误",
        })
    }
}
impl std::error::Error for Error {}
// 底层错误链只检查类型，最多 16 层，避免恶意循环 source 或无界遍历。
const MAX_ERROR_SOURCE_DEPTH: usize = 16;
impl Error {
    /// 原始 I/O 分类先保留已包裹的内核原因；不同协议阶段复用，不按错误文本判断。
    fn classify_io(error: &io::Error) -> Self {
        let mut mapped = match error.kind() {
            io::ErrorKind::TimedOut => Self::NetworkTimeout,
            io::ErrorKind::ConnectionRefused => Self::NetworkConnectRefused,
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
        mapped
    }
    /// TLS 阶段只归类通用 I/O；预算、取消、内部状态及协议等明确原因不能被 TLS 覆盖。
    pub(crate) fn from_tls(error: io::Error) -> Self {
        let mapped = Self::classify_io(&error);
        let mapped = if mapped == Self::Network {
            Self::Tls
        } else {
            mapped
        };
        diagnostics::record_io(&error, mapped.code());
        mapped
    }
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        let mapped = Self::classify_io(&error);
        diagnostics::record_io(&error, mapped.code());
        mapped
    }
}
#[cfg(test)]
mod tls_mapping_tests {
    use super::*;
    #[test]
    fn tls_keeps_budget_state_protocol_and_cancellation_causes() {
        // 预期来自业务契约：改变传输层不能把资源或权限/状态原因改报为 TLS 校验。
        for error in [
            Error::ResourceLimit,
            Error::Internal,
            Error::InvalidState,
            Error::Cancelled,
            Error::Rejected,
            Error::Unsupported,
            Error::ProtocolViolation,
        ] {
            assert_eq!(Error::from_tls(io::Error::other(error)), error);
        }
        assert_eq!(
            Error::from_tls(io::Error::from(io::ErrorKind::OutOfMemory)),
            Error::ResourceLimit
        );
        assert_eq!(
            Error::from_tls(io::Error::from(io::ErrorKind::TimedOut)),
            Error::NetworkTimeout
        );
        assert_eq!(
            Error::from_tls(io::Error::from(io::ErrorKind::InvalidData)),
            Error::Tls
        );
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// 成功和队列空是非错误状态；全部失败返回 ErrorCode 的四位整数。
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiStatus {
    /// 操作成功，不属于错误代码。
    Success = 0,
    /// 接收队列暂时为空，不属于错误代码。
    Empty = 11,
}
impl AbiStatus {
    pub const fn value(self) -> i32 {
        self as i32
    }
}
/// 零只表示同线程目前没有失败，不登记为一个错误代码。
pub const NO_ERROR_CODE: u16 = 0;
