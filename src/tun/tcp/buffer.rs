//! 固定容量 TCP 收发环；乱序位图不按输入增加节点，首个到达的字节保留。
use super::wire::{Sacks, before, distance};
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
    recent: [Option<u32>; 4],
    pub finished: bool,
}
impl Receive {
    pub fn new(sequence: u32, scaled: bool) -> Self {
        let capacity = if scaled { 131072 } else { CAPACITY };
        Self {
            bytes: Zeroizing::new(vec![0; capacity]),
            present: vec![0; capacity / 64],
            head: 0,
            base: sequence,
            next: sequence,
            finish: None,
            recent: [None; 4],
            finished: false,
        }
    }
    pub fn next(&self) -> u32 {
        self.next.wrapping_add(u32::from(self.finished))
    }
    pub fn window(&self) -> usize {
        self.bytes.len() - distance(self.next, self.base)
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
        let mut trigger = None;
        for (offset, byte) in bytes.iter().enumerate() {
            let seq = sequence.wrapping_add(offset as u32);
            if before(seq, self.base) || self.finish.is_some_and(|end| !before(seq, end)) {
                continue;
            }
            let delta = distance(seq, self.base);
            if delta >= self.bytes.len() {
                continue;
            }
            trigger.get_or_insert(seq);
            let index = (self.head + delta) % self.bytes.len();
            if !self.occupied(index) {
                self.bytes[index] = *byte;
                self.present[index / 64] |= 1 << (index % 64);
            }
        }
        let end = sequence.wrapping_add(bytes.len() as u32);
        if finish
            && !before(end, self.next)
            && distance(end, self.base) < self.bytes.len()
            && self.finish.is_none()
        {
            self.finish = Some(end);
        }
        while distance(self.next, self.base) < self.bytes.len()
            && self.finish != Some(self.next)
            && self.occupied((self.head + distance(self.next, self.base)) % self.bytes.len())
        {
            self.next = self.next.wrapping_add(1);
        }
        self.finished = self.finish == Some(self.next);
        if let Some(sequence) = trigger
            && !before(sequence, self.next)
        {
            self.recent.rotate_right(1);
            self.recent[0] = Some(sequence);
        }
    }
    /// 从逻辑偏移读取连续 64 个存在位；环回绕只涉及两个已分配位图字。
    fn bits(&self, offset: usize) -> u64 {
        let index = (self.head + offset) % self.bytes.len();
        let bit = index % 64;
        let word = index / 64;
        let low = self.present[word] >> bit;
        if bit == 0 {
            low
        } else {
            low | (self.present[(word + 1) % self.present.len()] << (64 - bit))
        }
    }
    fn block(&self, sequence: u32) -> Option<(u32, u32)> {
        let lower = distance(self.next, self.base);
        let mut start = distance(sequence, self.base);
        let limit = self
            .finish
            .map_or(self.bytes.len(), |end| distance(end, self.base));
        if start < lower || start >= limit || !self.occupied((self.head + start) % self.bytes.len())
        {
            return None;
        }
        let mut end = start;
        while start > lower {
            let n = (start - lower).min(64);
            let count = (self.bits(start - n) << (64 - n)).leading_ones() as usize;
            start -= count;
            if count < n {
                break;
            }
        }
        while end < limit {
            let n = (limit - end).min(64);
            let count = (self.bits(end).trailing_ones() as usize).min(n);
            end += count;
            if count < n {
                break;
            }
        }
        Some((
            self.base.wrapping_add(start as u32),
            self.base.wrapping_add(end as u32),
        ))
    }
    /// 最近触发块优先，随后重复最近块；不足四块时按位图补齐，不按报文分配节点。
    pub fn sacks(&self) -> Sacks {
        let mut out = Sacks::default();
        if self.finished {
            return out;
        }
        for sequence in self.recent.iter().flatten() {
            if let Some((left, right)) = self.block(*sequence)
                && !out.blocks().contains(&(left, right))
            {
                // block 仅由有限接收窗口构造，必为合法的模序号区间。
                if out.push(left, right).is_err() {
                    return out;
                }
            }
        }
        let mut offset = distance(self.next, self.base);
        let limit = self
            .finish
            .map_or(self.bytes.len(), |end| distance(end, self.base));
        while out.blocks().len() < 4 && offset < limit {
            let zero = (self.bits(offset).trailing_zeros() as usize).min((limit - offset).min(64));
            offset += zero;
            if offset == limit {
                break;
            }
            if zero == 64 {
                continue;
            }
            if let Some((left, right)) = self.block(self.base.wrapping_add(offset as u32)) {
                if !out.blocks().contains(&(left, right)) && out.push(left, right).is_err() {
                    break;
                }
                offset = distance(right, self.base);
            } else {
                break;
            }
        }
        out
    }
    pub fn copy_pending(&self, maximum: usize) -> Zeroizing<Vec<u8>> {
        let count = maximum.min(self.pending());
        Zeroizing::new(
            (0..count)
                .map(|i| self.bytes[(self.head + i) % self.bytes.len()])
                .collect(),
        )
    }
    pub fn consume(&mut self, count: usize) -> Result<()> {
        if count > self.pending() {
            return Err(Error::InvalidState);
        }
        for offset in 0..count {
            let index = (self.head + offset) % self.bytes.len();
            self.bytes[index] = 0;
            self.present[index / 64] &= !(1 << (index % 64));
        }
        self.head = (self.head + count) % self.bytes.len();
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
