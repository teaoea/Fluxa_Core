//! 解密日志按条数与总字节限制；读取后确认，避免 FFI 分配失败导致丢失记录。
use crate::budget::{ByteBudget, ByteLease};
use crate::{
    error::{Error, Result},
    http::Header,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Serialize, Serializer};
use std::{collections::VecDeque, sync::Mutex};
use zeroize::Zeroize;

#[derive(Clone, Serialize)]
pub struct Capture {
    pub id: u64,
    pub method: String,
    pub url: String,
    pub request_headers: Vec<Header>,
    pub response_headers: Vec<Header>,
    #[serde(serialize_with = "body_base64")]
    pub request_body: Vec<u8>,
    #[serde(serialize_with = "body_base64")]
    pub response_body: Vec<u8>,
    pub request_body_length: usize,
    pub response_body_length: usize,
    pub status: Option<u16>,
    pub intercepted: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
}
fn body_base64<S: Serializer>(body: &[u8], serializer: S) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&STANDARD.encode(body))
}
impl Capture {
    pub fn new() -> Self {
        Self {
            id: 0,
            method: "CONNECT".into(),
            url: String::new(),
            request_headers: Vec::new(),
            response_headers: Vec::new(),
            request_body: Vec::new(),
            response_body: Vec::new(),
            request_body_length: 0,
            response_body_length: 0,
            status: None,
            intercepted: false,
            duration_ms: 0,
            error: None,
        }
    }
    fn size(&self) -> usize {
        self.request_body.len()
            + self.response_body.len()
            + self.url.len()
            + self.method.len()
            + self
                .request_headers
                .iter()
                .chain(&self.response_headers)
                .map(|h| h.name.len() + h.value.len())
                .sum::<usize>()
            + 1024
    }
}
impl Default for Capture {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.request_body.zeroize();
        self.response_body.zeroize();
    }
}
struct StoredCapture {
    capture: Capture,
    _lease: ByteLease,
}
#[derive(Default)]
struct Queue {
    records: VecDeque<StoredCapture>,
    bytes: usize,
    next_id: u64,
    dropped: u64,
}
#[derive(Default)]
pub struct CaptureStore {
    queue: Mutex<Queue>,
}
impl CaptureStore {
    pub fn push(&self, mut capture: Capture) -> Result<()> {
        let mut queue = self.queue.lock().map_err(|_| Error::Internal)?;
        capture.id = queue.next_id.checked_add(1).ok_or(Error::ResourceLimit)?;
        queue.next_id = capture.id;
        let size = capture.size();
        while !queue.records.is_empty()
            && (queue.records.len() >= 64 || queue.bytes.saturating_add(size) > 32 * 1024 * 1024)
        {
            if let Some(old) = queue.records.pop_front() {
                queue.bytes -= old.capture.size();
                queue.dropped = queue.dropped.saturating_add(1);
            }
        }
        if size > 32 * 1024 * 1024 {
            return Err(Error::ResourceLimit);
        }
        static CAPTURE_MEMORY: ByteBudget = ByteBudget::new(64 * 1024 * 1024);
        let lease = match CAPTURE_MEMORY.reserve(size) {
            Ok(lease) => lease,
            Err(error) => {
                queue.dropped = queue.dropped.saturating_add(1);
                return Err(error);
            }
        };
        queue.bytes += size;
        queue.records.push_back(StoredCapture {
            capture,
            _lease: lease,
        });
        Ok(())
    }
    pub fn peek(&self) -> Result<Option<Capture>> {
        Ok(self
            .queue
            .lock()
            .map_err(|_| Error::Internal)?
            .records
            .front()
            .map(|record| record.capture.clone()))
    }
    pub fn acknowledge(&self, id: u64) -> Result<()> {
        let mut queue = self.queue.lock().map_err(|_| Error::Internal)?;
        if queue.records.front().map(|r| r.capture.id) != Some(id) {
            return Err(Error::InvalidState);
        }
        if let Some(record) = queue.records.pop_front() {
            queue.bytes -= record.capture.size();
        }
        Ok(())
    }
    pub fn dropped(&self) -> Result<u64> {
        Ok(self.queue.lock().map_err(|_| Error::Internal)?.dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oldest_records_are_evicted_and_acknowledgement_is_exact() {
        let store = CaptureStore::default();
        for _ in 0..65 {
            store.push(Capture::new()).unwrap();
        }
        assert_eq!(store.dropped().unwrap(), 1);
        let first = store.peek().unwrap().unwrap();
        assert_eq!(first.id, 2);
        assert!(store.acknowledge(1).is_err());
        assert_eq!(store.peek().unwrap().unwrap().id, 2);
        store.acknowledge(2).unwrap();
        assert_eq!(store.peek().unwrap().unwrap().id, 3);
    }
}
