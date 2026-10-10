use super::{
    Family, Manager, Opened, duration,
    resolution::{candidates, resolve},
};
use crate::{
    dns::Resolver,
    error::{Error, Result},
    routing::{Decision, Router, Target},
};
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutedTarget {
    pub target: Target,
    pub timeout_ms: u64,
    #[serde(default)]
    pub family: Family,
    #[serde(default)]
    pub mapping_generation: Option<u64>,
}
#[derive(Debug)]
pub struct RoutedOpened {
    pub connection: Opened,
    pub decision: Decision,
}
impl Manager {
    /// 全部地址候选可执行后才拨号，不通过切换地址族绕过拒绝策略。
    pub async fn connect_routed(
        &self,
        router: &Router,
        resolver: Option<&Resolver>,
        request: RoutedTarget,
    ) -> Result<RoutedOpened> {
        self.connect_with_snapshot(router, resolver, request, self.network_snapshot()?)
            .await
    }
    /// 同一次读取 DNS、路由和资源参数，避免配置替换时混用。
    pub async fn connect_configured(&self, request: RoutedTarget) -> Result<RoutedOpened> {
        let snapshot = self.network_snapshot()?;
        let router = snapshot.router.as_ref().ok_or(Error::InvalidState)?;
        self.connect_with_snapshot(
            router,
            snapshot.resolver.as_deref(),
            request,
            std::sync::Arc::clone(&snapshot),
        )
        .await
    }
    async fn connect_with_snapshot(
        &self,
        router: &Router,
        resolver: Option<&Resolver>,
        request: RoutedTarget,
        snapshot: std::sync::Arc<super::NetworkSnapshot>,
    ) -> Result<RoutedOpened> {
        let deadline = duration(request.timeout_ms)?;
        let mut metadata = request.target;
        if metadata.port == 0 || metadata.domain.is_some() == metadata.address.is_some() {
            return Err(Error::InvalidInput);
        }
        let mut numeric = metadata
            .address
            .as_deref()
            .map(|value| {
                if value.len() > 64 {
                    return Err(Error::InvalidInput);
                }
                value.parse::<IpAddr>().map_err(|_| Error::InvalidInput)
            })
            .transpose()?;
        if let (Some(ip), Some(resolver)) = (numeric, resolver) {
            if resolver.is_fake_ip(ip) {
                let generation = request.mapping_generation.ok_or(Error::InvalidState)?;
                metadata.domain = Some(resolver.restore_fake_ip(&ip.to_string(), generation)?);
                metadata.address = None;
                numeric = None;
            } else if request.mapping_generation.is_some() {
                return Err(Error::InvalidInput);
            }
        } else if request.mapping_generation.is_some() {
            return Err(Error::InvalidInput);
        }
        if let Some(name) = &metadata.domain {
            if name.len() > 254 {
                return Err(Error::InvalidInput);
            }
            let name = crate::network::normalize_host(name)?;
            if name.parse::<IpAddr>().is_ok() {
                return Err(Error::InvalidInput);
            }
            metadata.domain = Some(name);
        }
        if numeric.is_some_and(|ip| {
            ip.is_unspecified()
                || matches!(
                    (request.family, ip),
                    (Family::Ipv4, IpAddr::V6(_)) | (Family::Ipv6, IpAddr::V4(_))
                )
        }) {
            return Err(Error::InvalidInput);
        }
        let mut decision = None;
        let opened = self
            .establish_with(snapshot, deadline, async {
                if let Some(early) = router.evaluate_before_dns(metadata.clone())? {
                    early.check_support()?;
                }
                let addresses = if let Some(ip) = numeric {
                    vec![ip]
                } else {
                    let resolver = resolver.ok_or(Error::InvalidState)?;
                    resolve(
                        resolver,
                        metadata.domain.as_deref().ok_or(Error::InvalidInput)?,
                        request.family,
                        true,
                    )
                    .await?
                };
                let mut plans = Vec::with_capacity(addresses.len());
                for ip in addresses {
                    let mut target = metadata.clone();
                    target.address = Some(ip.to_string());
                    let route = router.evaluate(target)?;
                    route.check_support()?;
                    plans.push((SocketAddr::new(ip, metadata.port), route));
                }
                let dialed = candidates(
                    plans.iter().map(|(address, _)| *address).collect(),
                    metadata.transport,
                )
                .await?;
                decision = Some(
                    plans
                        .into_iter()
                        .find(|(address, _)| *address == dialed.2)
                        .ok_or(Error::Internal)?
                        .1,
                );
                Ok(dialed)
            })
            .await?;
        let Some(decision) = decision else {
            self.close(opened.id)?;
            return Err(Error::Internal);
        };
        Ok(RoutedOpened {
            connection: opened,
            decision,
        })
    }
}
