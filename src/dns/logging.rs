//! DNS每次调用的日志作用域；不绑定解析器、不保存查询历史或控制解析结果。
use super::{Answer, Query};
use crate::{
    error::{Error, Result},
    logging::{Context, DnsDetails, DnsText, Event, Journal},
};
use std::{sync::Arc, time::Instant};
#[derive(Clone)]
pub(crate) struct Scope {
    pub journal: Arc<Journal>,
    pub connection_id: Option<u64>,
    pub revision: Option<u64>,
}
impl Scope {
    pub(crate) fn new(
        journal: Arc<Journal>,
        connection_id: Option<u64>,
        revision: Option<u64>,
    ) -> Self {
        Self {
            journal,
            connection_id,
            revision,
        }
    }
}
pub(crate) struct Trace {
    scope: Scope,
    details: DnsDetails,
    started: Instant,
    finished: bool,
}
impl Trace {
    pub(crate) fn begin(scope: Option<Scope>, query: &Query) -> Option<Self> {
        let scope = scope?;
        // 日志ID耗尽只停发该查询日志，不能改变DNS准入/业务返回值。
        let query_id = crate::lifecycle::identifier()?;
        let trace = Self {
            scope,
            details: DnsDetails {
                query_id,
                name: DnsText::new(&query.name),
                query_type: DnsText::new(&query.query_type),
                upstream_index: None,
                upstream: None,
                source: None,
                rcode: None,
                records: None,
                addresses: None,
                ttl: None,
                from_cache: None,
                validation: None,
            },
            started: Instant::now(),
            finished: false,
        };
        trace.emit(Event::DnsQueryStarted, None, trace.details);
        Some(trace)
    }
    fn emit(&self, event: Event, error: Option<Error>, details: DnsDetails) {
        self.scope.journal.event(
            event,
            None,
            self.scope.revision,
            None,
            error,
            Context {
                connection_id: self.scope.connection_id,
                dns: Some(details),
                duration_ms: Some(self.started.elapsed().as_millis().min(u64::MAX as u128) as u64),
                ..Default::default()
            },
        );
    }
    pub(super) fn hit(&self, event: Event, source: &'static str, answer: &Answer) {
        let mut details = self.answer(answer);
        details.source = Some(source);
        self.emit(event, None, details);
    }
    pub(crate) fn upstream(&self, index: usize, label: &str, error: Option<Error>) {
        let mut details = self.details;
        details.upstream_index = Some(index);
        details.upstream = DnsText::new(label);
        details.source = Some("upstream");
        self.emit(
            if error.is_some() {
                Event::DnsUpstreamFailed
            } else {
                Event::DnsUpstreamSelected
            },
            error,
            details,
        );
    }
    fn answer(&self, answer: &Answer) -> DnsDetails {
        let mut details = self.details;
        details.rcode = Some(answer.rcode);
        details.records = Some(answer.records.len());
        details.addresses = Some(answer.addresses.len());
        details.ttl = Some(answer.ttl);
        details.from_cache = Some(answer.from_cache);
        details.validation = match answer.validation.as_str() {
            "hosts" => Some("hosts"),
            "synthetic" => Some("synthetic"),
            "secure" => Some("secure"),
            "insecure" => Some("insecure"),
            "disabled" => Some("disabled"),
            _ => None,
        };
        details
    }
    pub(crate) fn finish_system(&mut self, result: &Result<Vec<std::net::IpAddr>>) {
        self.finished = true;
        let mut details = self.details;
        details.source = Some("system");
        match result {
            Ok(values) => {
                details.addresses = Some(values.len());
                self.emit(Event::DnsQueryCompleted, None, details);
            }
            Err(error) => self.emit(Event::DnsQueryFailed, Some(*error), details),
        }
    }
    pub(super) fn scope(&self) -> Scope {
        self.scope.clone()
    }
    pub(super) fn finish_probe(&mut self, result: &Result<()>) {
        self.finished = true;
        let mut details = self.details;
        details.source = Some("preflight");
        match result {
            Ok(()) => self.emit(Event::DnsQueryCompleted, None, details),
            Err(error) => self.emit(Event::DnsQueryFailed, Some(*error), details),
        }
    }
    pub(super) fn finish(&mut self, result: &Result<Answer>) {
        self.finished = true;
        match result {
            Ok(answer) => self.emit(Event::DnsQueryCompleted, None, self.answer(answer)),
            Err(error) => self.emit(Event::DnsQueryFailed, Some(*error), self.details),
        }
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        if !self.finished {
            // 展开异常不是用户取消；日志保持失败类型，不把panic伪称正常取消。
            let (event, error) = if std::thread::panicking() {
                (Event::DnsQueryFailed, Error::Internal)
            } else {
                (Event::DnsQueryCancelled, Error::Cancelled)
            };
            self.emit(event, Some(error), self.details);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/test_dns_logging.rs"]
mod test_dns_logging;
