//! 固定容量 TCP 收发环；乱序位图不按输入增加节点，首个到达的字节保留。
use super::wire::{before, distance};
use crate::error::{Error, Result};
use zeroize::Zeroizing;
pub const CAPACITY: usize = 32768;
pub struct Receive {
    bytes: Zeroizing<Vec<u8>>,
    present: Vec<u64>,
    head: usize,
    base: u32,
    next: u32,
    finish: Option<u32>,
    pub finished: bool,
}
impl Receive {
    pub fn new(sequence: u32) -> Self {
        Self {
            bytes: Zeroizing::new(vec![0; CAPACITY]),
            present: vec![0; CAPACITY / 64],
            head: 0,
            base: sequence,
            next: sequence,
            finish: None,
            finished: false,
        }
    }
    pub fn next(&self) -> u32 {
        self.next.wrapping_add(u32::from(self.finished))
    }
    pub fn window(&self) -> u16 {
        (CAPACITY - distance(self.next, self.base)) as u16
    }
    pub fn pending(&self) -> usize {
        distance(self.next, self.base)
    }
    fn occupied(&self, index: usize) -> bool {
        self.present[index / 64] & (1 << (index % 64)) != 0
    }
    pub fn accept(&mut self, sequence: u32, bytes: &[u8], finish: bool) {
        if self.finished {
            return;
        }
        for (offset, byte) in bytes.iter().enumerate() {
            let seq = sequence.wrapping_add(offset as u32);
            if before(seq, self.base) || self.finish.is_some_and(|end| !before(seq, end)) {
                continue;
            }
            let delta = distance(seq, self.base);
            if delta >= CAPACITY {
                continue;
            }
            let index = (self.head + delta) % CAPACITY;
            if !self.occupied(index) {
                self.bytes[index] = *byte;
                self.present[index / 64] |= 1 << (index % 64);
            }
        }
        let end = sequence.wrapping_add(bytes.len() as u32);
        if finish
            && !before(end, self.next)
            && distance(end, self.base) < CAPACITY
            && self.finish.is_none()
        {
            self.finish = Some(end);
        }
        while distance(self.next, self.base) < CAPACITY
            && self.finish != Some(self.next)
            && self.occupied((self.head + distance(self.next, self.base)) % CAPACITY)
        {
            self.next = self.next.wrapping_add(1);
        }
        self.finished = self.finish == Some(self.next);
    }
    pub fn copy_pending(&self, maximum: usize) -> Zeroizing<Vec<u8>> {
        let count = maximum.min(self.pending());
        Zeroizing::new(
            (0..count)
                .map(|i| self.bytes[(self.head + i) % CAPACITY])
                .collect(),
        )
    }
    pub fn consume(&mut self, count: usize) -> Result<()> {
        if count > self.pending() {
            return Err(Error::InvalidState);
        }
        for offset in 0..count {
            let index = (self.head + offset) % CAPACITY;
            self.bytes[index] = 0;
            self.present[index / 64] &= !(1 << (index % 64));
        }
        self.head = (self.head + count) % CAPACITY;
        self.base = self.base.wrapping_add(count as u32);
        Ok(())
    }
}
pub struct Transmit {
    bytes: Zeroizing<Vec<u8>>,
    head: usize,
    pub base: u32,
    pub sent: usize,
    length: usize,
}
impl Transmit {
    pub fn new(sequence: u32) -> Self {
        Self {
            bytes: Zeroizing::new(vec![0; CAPACITY]),
            head: 0,
            base: sequence,
            sent: 0,
            length: 0,
        }
    }
    pub fn available(&self) -> usize {
        CAPACITY - self.length
    }
    pub fn unsent(&self) -> usize {
        self.length - self.sent
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > self.available() {
            return Err(Error::ResourceLimit);
        }
        for (offset, byte) in bytes.iter().enumerate() {
            self.bytes[(self.head + self.length + offset) % CAPACITY] = *byte;
        }
        self.length += bytes.len();
        Ok(())
    }
    pub fn copy(&self, sequence: u32, count: usize) -> Result<Zeroizing<Vec<u8>>> {
        let offset = distance(sequence, self.base);
        if offset > self.length || count > self.length - offset {
            return Err(Error::InvalidState);
        }
        Ok(Zeroizing::new(
            (0..count)
                .map(|i| self.bytes[(self.head + offset + i) % CAPACITY])
                .collect(),
        ))
    }
    pub fn acknowledge(&mut self, count: usize) -> Result<()> {
        if count > self.sent {
            return Err(Error::InvalidInput);
        }
        for offset in 0..count {
            self.bytes[(self.head + offset) % CAPACITY] = 0;
        }
        self.head = (self.head + count) % CAPACITY;
        self.base = self.base.wrapping_add(count as u32);
        self.length -= count;
        self.sent -= count;
        Ok(())
    }
}
#[cfg(test)]
#[path = "../../../tests/test_tun_tcp_buffer.rs"]
mod test_tun_tcp_buffer;
