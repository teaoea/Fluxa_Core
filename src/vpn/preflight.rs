//! 真实网络前置检查；只在隔离管理器执行，不修改系统状态或授予接管资格。
use super::transaction::{Cancellation, Controller, Step, Steps, trusted};
use crate::{
    budget::{ByteBudget, ByteLease},
    configuration::Document,
    error::{Error, ErrorInfo, Result},
    https::HttpsClient,
    outbound::Manager,
    storage::Resources,
};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};
const MAX_PROBES: usize = 32;
static INPUT_MEMORY: ByteBudget = ByteBudget::new(8 * 1024 * 1024);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Check {
    Configuration,
    RulesAndAssets,
    Geoip,
    Dns,
    RequiredOutbounds,
    ResourceBudget,
    Permission,
    SystemCapability,
    LoopProtection,
    LocalAuthorization,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    NotRun,
    Passed,
    Disabled,
    Partial,
    Unavailable,
    AuthorizationRequired,
    Failed,
}
#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub check: Check,
    pub outcome: Outcome,
    pub error_info: Option<ErrorInfo>,
}
/// 无目标网址、节点名称或凭据。网络检查成功不能当成完整 Preflight 通过。
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub transaction: u64,
    pub findings: [Finding; 10],
    pub dns_upstreams_checked: usize,
    pub required_outbounds: usize,
    pub verified_outbounds: usize,
    pub probes_completed: usize,
    pub network_checks_passed: bool,
    pub activation_permitted: bool,
    pub geoip_status: Option<crate::geoip::Status>,
    pub platform_check: Option<super::platform_check::Assessment>,
    pub startup_budget: Option<super::startup_budget::Assessment>,
}
impl Default for Report {
    fn default() -> Self {
        Self {
            transaction: 0,
            findings: [
                Check::Configuration,
                Check::RulesAndAssets,
                Check::Geoip,
                Check::Dns,
                Check::RequiredOutbounds,
                Check::ResourceBudget,
                Check::Permission,
                Check::SystemCapability,
                Check::LoopProtection,
                Check::LocalAuthorization,
            ]
            .map(|check| Finding {
                check,
                outcome: Outcome::NotRun,
                error_info: None,
            }),
            dns_upstreams_checked: 0,
            required_outbounds: 0,
            verified_outbounds: 0,
            probes_completed: 0,
            network_checks_passed: false,
            activation_permitted: false,
            geoip_status: None,
            platform_check: None,
            startup_budget: None,
        }
    }
}
struct Input {
    yaml: zeroize::Zeroizing<String>,
    resources: Resources,
    probes: Vec<String>,
    _memory: ByteLease,
}
pub(super) struct Inspection {
    pub(super) report: Mutex<Report>,
    // 终态摘要仍占实际内存，随全局记录及最后在途引用销毁归还。
    _memory: ByteLease,
}
struct Driver {
    dns_logs: Arc<crate::logging::Journal>,
    input: Mutex<Option<Input>>,
    manager: Arc<Manager>,
    report: Arc<Inspection>,
    // 仅内部回环测试提供根证书；公开入口固定系统根，不接收成功标志或跳过验证开关。
    tls: Option<HttpsClient>,
}
impl trusted::Sealed for Driver {}
fn validate(yaml: &str, resources: &Resources, probes: &[String]) -> Result<()> {
    if yaml.len() > crate::configuration::MAX_CONFIG_BYTES || probes.len() > MAX_PROBES {
        return Err(Error::ResourceLimit);
    }
    if probes.is_empty() {
        return Err(Error::InvalidInput);
    }
    resources.validate()?;
    let mut seen = BTreeSet::new();
    for value in probes {
        if value.len() > 2048 {
            return Err(Error::ResourceLimit);
        }
        let url = url::Url::parse(value).map_err(|_| Error::InvalidInput)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.port() == Some(0)
            || !seen.insert(url.to_string())
        {
            return Err(Error::InvalidInput);
        }
    }
    Ok(())
}
// 资源引用转移给内核后仍占内存：按真实容量收费，而不是仅按条目数或路径长度估算。
fn input_charge(yaml: &str, resources: &Resources, probes: &[String]) -> Result<usize> {
    if yaml.len() > crate::configuration::MAX_CONFIG_BYTES
        || probes.len() > MAX_PROBES
        || resources.plugins.len() > 128
        || resources.rules.len() > 128
    {
        return Err(Error::ResourceLimit);
    }
    let mut bytes = yaml
        .len()
        .checked_add(probes.len() * 4096)
        .ok_or(Error::ResourceLimit)?;
    let mut add = |amount: usize| -> Result<()> {
        bytes = bytes.checked_add(amount).ok_or(Error::ResourceLimit)?;
        Ok(())
    };
    add(resources
        .plugins
        .capacity()
        .checked_mul(std::mem::size_of::<crate::storage::PluginFiles>())
        .ok_or(Error::ResourceLimit)?)?;
    add(resources
        .rules
        .capacity()
        .checked_mul(std::mem::size_of::<crate::storage::NamedFile>())
        .ok_or(Error::ResourceLimit)?)?;
    let path_charge =
        |path: &std::path::PathBuf| path.capacity().checked_mul(2).ok_or(Error::ResourceLimit);
    for plugin in &resources.plugins {
        add(plugin.name.capacity())?;
        add(path_charge(&plugin.path)?)?;
        if let Some(script) = &plugin.script {
            add(script.name.capacity())?;
            add(path_charge(&script.path)?)?;
        }
    }
    for file in &resources.rules {
        add(file.name.capacity())?;
        add(path_charge(&file.path)?)?;
    }
    if let Some(file) = &resources.script {
        add(file.name.capacity())?;
        add(path_charge(&file.path)?)?;
    }
    if let Some(path) = &resources.geoip {
        add(path_charge(path)?)?;
    }
    Ok(bytes)
}
pub(super) fn begin(
    owner: &Controller,
    yaml: &str,
    resources: Resources,
    probes: &[String],
    deadline: Duration,
    tls: Option<HttpsClient>,
) -> Result<u64> {
    let memory = INPUT_MEMORY.reserve(input_charge(yaml, &resources, probes)?)?;
    validate(yaml, &resources, probes)?;
    let mut text = String::new();
    text.try_reserve_exact(yaml.len())
        .map_err(|_| Error::ResourceLimit)?;
    text.push_str(yaml);
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(probes.len())
        .map_err(|_| Error::ResourceLimit)?;
    for probe in probes {
        let mut copy = String::new();
        copy.try_reserve_exact(probe.len())
            .map_err(|_| Error::ResourceLimit)?;
        copy.push_str(probe);
        targets.push(copy);
    }
    let report = Arc::new(Inspection {
        report: Mutex::new(Report::default()),
        _memory: INPUT_MEMORY.reserve(65536)?,
    });
    let driver = Arc::new(Driver {
        dns_logs: owner.logs.clone(),
        input: Mutex::new(Some(Input {
            yaml: zeroize::Zeroizing::new(text),
            resources,
            probes: targets,
            _memory: memory,
        })),
        manager: Arc::new(Manager::default()),
        report: report.clone(),
        tls,
    });
    owner.begin_with_report(driver, deadline, Some(report))
}
impl Driver {
    fn update(&self, action: impl FnOnce(&mut Report)) -> Result<()> {
        let mut report = self.report.report.lock().map_err(|_| Error::Internal)?;
        action(&mut report);
        Ok(())
    }
    fn finding(&self, check: Check, outcome: Outcome, error: Option<Error>) -> Result<()> {
        self.update(|report| {
            if let Some(item) = report.findings.iter_mut().find(|f| f.check == check) {
                item.outcome = outcome;
                item.error_info = error.map(|e| e.public_info());
            }
        })
    }
    async fn inspect(
        &self,
        current: &mut Check,
        transaction: u64,
        cancel: &Cancellation,
    ) -> Result<()> {
        let input = self
            .input
            .lock()
            .map_err(|_| Error::Internal)?
            .take()
            .ok_or(Error::InvalidState)?;
        *current = Check::Configuration;
        let document = Arc::new(Document::from_yaml(&input.yaml)?);
        document.preparation_preflight(crate::configuration::PreparationPurpose::VpnCandidate)?;
        let nodes = document.summary().node_count;
        let dns = document.dns_settings().enable;
        let strategy = document.ip_strategy();
        self.finding(*current, Outcome::Passed, None)?;
        self.finding(
            Check::LocalAuthorization,
            if nodes == 0 {
                Outcome::AuthorizationRequired
            } else {
                Outcome::NotRun
            },
            None,
        )?;
        if document.geoip_url().is_some() && input.resources.geoip.is_none() {
            *current = Check::Geoip;
            return Err(Error::GeoipInitializationFailed);
        }
        *current = Check::RulesAndAssets;
        let client = match &self.tls {
            Some(tls) => tls.clone(),
            None => HttpsClient::system()?,
        };
        self.manager
            .prepare_vpn_candidate(
                document,
                client.clone(),
                input.resources,
                Some(self.dns_logs.clone()),
            )
            .await?;
        self.finding(*current, Outcome::Passed, None)?;
        let snapshot = self.manager.network_snapshot()?;
        *current = Check::Geoip;
        let geoip = self.manager.configuration_status()?.geoip_status;
        self.update(|r| r.geoip_status = Some(geoip))?;
        self.finding(
            *current,
            if snapshot.geoip.is_some() {
                Outcome::Passed
            } else {
                Outcome::Disabled
            },
            None,
        )?;
        *current = Check::RequiredOutbounds;
        let mut required = snapshot
            .router
            .as_ref()
            .ok_or(Error::InvalidState)?
            .preflight_outbounds()?;
        required.extend(
            snapshot
                .configuration
                .as_ref()
                .ok_or(Error::InvalidState)?
                .preflight_rewrite_outbounds(&snapshot.plugins),
        );
        if required.len() > MAX_PROBES {
            return Err(Error::ResourceLimit);
        }
        self.update(|r| r.required_outbounds = required.len())?;
        *current = Check::Dns;
        if dns {
            let name = input
                .probes
                .iter()
                .find_map(|target| {
                    let url = url::Url::parse(target).ok()?;
                    let host = url.host_str()?.trim_matches(['[', ']']);
                    host.parse::<std::net::IpAddr>()
                        .is_err()
                        .then(|| host.to_owned())
                })
                .ok_or(Error::InvalidInput)?;
            let count = snapshot
                .resolver
                .as_ref()
                .ok_or(Error::InvalidState)?
                .preflight_upstreams(
                    &name,
                    strategy,
                    Some(crate::dns::logging::Scope::new(
                        self.dns_logs.clone(),
                        None,
                        None,
                    )),
                )
                .await?;
            self.update(|r| r.dns_upstreams_checked = count)?;
            self.finding(*current, Outcome::Passed, None)?;
        } else {
            self.finding(*current, Outcome::Disabled, None)?;
        }
        *current = Check::RequiredOutbounds;
        let probe = client.with_network(self.manager.clone());
        let mut verified = BTreeSet::new();
        for target in &input.probes {
            let decision = probe.probe(target).await?;
            if decision.policy == crate::routing::Policy::Proxy {
                verified.insert(decision.outbound.ok_or(Error::InvalidState)?);
            }
            self.update(|r| {
                r.probes_completed += 1;
                r.verified_outbounds = verified.len();
            })?;
        }
        if !required.is_subset(&verified) {
            return Err(Error::InvalidInput);
        }
        self.finding(*current, Outcome::Passed, None)?;
        self.update(|r| r.network_checks_passed = true)?;
        // 完整生产运行计划和可信OS余量尚未接入，未知量不能用零或组件配额代替。
        let mut budget = super::startup_budget::Request::new(
            transaction,
            super::startup_budget::Plan::incomplete(),
            std::time::Instant::now(),
        )?;
        let assessment = budget.assess(None, std::time::Instant::now(), cancel.requested())?;
        self.update(|r| r.startup_budget = Some(assessment))?;
        self.finding(Check::ResourceBudget, Outcome::Partial, None)?;
        if assessment.reason == Some(super::startup_budget::Reason::Cancelled) {
            return Err(Error::Cancelled);
        }
        // 真实平台接线尚未提供：不从配置、URL或网络成功伪造平台证据。
        // 必要物理出站地址尚未由平台准备流程固定，None明确表示未解析而非空集合。
        let mut request = super::platform_check::Request::new(
            transaction,
            strategy,
            dns,
            None,
            std::time::Instant::now(),
        )?;
        let assessment = request.unavailable(std::time::Instant::now(), cancel.requested());
        self.update(|r| r.platform_check = Some(assessment))?;
        for (check, status) in [
            (Check::Permission, assessment.permission),
            (Check::SystemCapability, assessment.system_capability),
            (Check::LoopProtection, assessment.loop_protection),
        ] {
            let outcome = match status {
                super::platform_check::CheckState::NotRun => Outcome::NotRun,
                super::platform_check::CheckState::Passed => Outcome::Passed,
                super::platform_check::CheckState::Unavailable => Outcome::Unavailable,
                super::platform_check::CheckState::Rejected => Outcome::Failed,
            };
            let error = matches!(outcome, Outcome::Unavailable | Outcome::Failed).then_some(
                if assessment.reason == Some(super::platform_check::Reason::Cancelled) {
                    Error::Cancelled
                } else {
                    Error::Unsupported
                },
            );
            self.finding(check, outcome, error)?;
        }
        if assessment.reason == Some(super::platform_check::Reason::Cancelled) {
            return Err(Error::Cancelled);
        }
        // 此轮不实现可信本地授权或平台接线；绝不让网络成功推进到 Prepare。
        Err(Error::Unsupported)
    }
}
impl Steps for Driver {
    fn preflight(&self, transaction: u64, mut cancel: Cancellation) -> Step<'_> {
        Box::pin(async move {
            self.update(|r| r.transaction = transaction)?;
            let mut current = Check::Configuration;
            if cancel.requested() {
                return Err(Error::Cancelled);
            }
            let activity = cancel.clone();
            let result = tokio::select! {
                biased;
                _ = cancel.notified() => Err(Error::Cancelled),
                result = self.inspect(&mut current, transaction, &activity) => result,
            };
            if let Err(error) = result {
                let network_passed = self
                    .report
                    .report
                    .lock()
                    .map_err(|_| Error::Internal)?
                    .network_checks_passed;
                if !network_passed {
                    self.finding(current, Outcome::Failed, Some(error))?;
                    if error.public_info().code.contract().category == "GEOIP" {
                        self.finding(Check::Geoip, Outcome::Failed, Some(error))?;
                    }
                }
            }
            result
        })
    }
    fn prepare(&self, _: u64, _: Cancellation) -> Step<'_> {
        Box::pin(async { Err(Error::Unsupported) })
    }
    fn commit(&self, _: u64, _: Cancellation) -> Step<'_> {
        Box::pin(async { Err(Error::Unsupported) })
    }
    fn verify(&self, _: u64, _: Cancellation) -> Step<'_> {
        Box::pin(async { Err(Error::Unsupported) })
    }
    fn rollback(&self, _: u64) -> Step<'_> {
        Box::pin(self.manager.stop())
    }
}
#[cfg(test)]
#[path = "../../tests/test_vpn_preflight.rs"]
mod test_vpn_preflight;
