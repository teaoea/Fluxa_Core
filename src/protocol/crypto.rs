//! 密码套件与记录层集中复用；密码原语来自 ring，协议业务不使用 unsafe。
use crate::error::{Error, Result};
use ring::{aead, digest, hkdf, hmac};
use zeroize::Zeroizing;
const MAX_RECORD: usize = 16640;
pub struct Size(pub usize);
impl hkdf::KeyType for Size {
    fn len(&self) -> usize {
        self.0
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Suite {
    Aes128,
    Aes256,
    ChaCha,
}
impl Suite {
    pub fn id(self) -> u16 {
        match self {
            Self::Aes128 => 0x1301,
            Self::Aes256 => 0x1302,
            Self::ChaCha => 0x1303,
        }
    }
    pub fn parse(id: u16) -> Result<Self> {
        match id {
            0x1301 => Ok(Self::Aes128),
            0x1302 => Ok(Self::Aes256),
            0x1303 => Ok(Self::ChaCha),
            _ => Err(Error::Unsupported),
        }
    }
    pub fn hash_len(self) -> usize {
        if matches!(self, Self::Aes256) { 48 } else { 32 }
    }
    fn key_len(self) -> usize {
        if matches!(self, Self::Aes128) { 16 } else { 32 }
    }
    fn aead(self) -> &'static aead::Algorithm {
        match self {
            Self::Aes128 => &aead::AES_128_GCM,
            Self::Aes256 => &aead::AES_256_GCM,
            Self::ChaCha => &aead::CHACHA20_POLY1305,
        }
    }
    pub fn mac(self) -> hmac::Algorithm {
        if matches!(self, Self::Aes256) {
            hmac::HMAC_SHA384
        } else {
            hmac::HMAC_SHA256
        }
    }
    pub fn hash(self, data: &[u8]) -> Vec<u8> {
        digest::digest(
            if matches!(self, Self::Aes256) {
                &digest::SHA384
            } else {
                &digest::SHA256
            },
            data,
        )
        .as_ref()
        .to_vec()
    }
    pub fn extract(self, salt: &[u8], ikm: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        Ok(Zeroizing::new(
            hmac::sign(&hmac::Key::new(self.mac(), salt), ikm)
                .as_ref()
                .to_vec(),
        ))
    }
    pub fn expand(
        self,
        secret: &[u8],
        label: &str,
        context: &[u8],
        size: usize,
    ) -> Result<Zeroizing<Vec<u8>>> {
        let label = format!("tls13 {label}");
        if label.len() > 255 || context.len() > 255 || size > 65535 {
            return Err(Error::InvalidInput);
        }
        let mut info = Vec::with_capacity(4 + label.len() + context.len());
        info.extend((size as u16).to_be_bytes());
        info.push(label.len() as u8);
        info.extend(label.as_bytes());
        info.push(context.len() as u8);
        info.extend(context);
        let alg = if matches!(self, Self::Aes256) {
            hkdf::HKDF_SHA384
        } else {
            hkdf::HKDF_SHA256
        };
        let prk = hkdf::Prk::new_less_safe(alg, secret);
        let input = [info.as_slice()];
        let mut result = Zeroizing::new(vec![0; size]);
        prk.expand(&input, Size(size))
            .map_err(|_| Error::Internal)?
            .fill(&mut result)
            .map_err(|_| Error::Internal)?;
        Ok(result)
    }
}
pub struct Cipher {
    suite: Suite,
    key: aead::LessSafeKey,
    iv: Zeroizing<Vec<u8>>,
    sequence: u64,
    secret: Zeroizing<Vec<u8>>,
}
impl Cipher {
    pub fn new(secret: Zeroizing<Vec<u8>>, suite: Suite) -> Result<Self> {
        let key = suite.expand(&secret, "key", &[], suite.key_len())?;
        let iv = suite.expand(&secret, "iv", &[], 12)?;
        Ok(Self {
            suite,
            key: aead::LessSafeKey::new(
                aead::UnboundKey::new(suite.aead(), &key).map_err(|_| Error::Tls)?,
            ),
            iv,
            sequence: 0,
            secret,
        })
    }
    fn nonce(&mut self) -> Result<aead::Nonce> {
        let mut nonce: [u8; 12] = self.iv.as_slice().try_into().map_err(|_| Error::Internal)?;
        for (v, n) in nonce[4..].iter_mut().zip(self.sequence.to_be_bytes()) {
            *v ^= n;
        }
        self.sequence = self.sequence.checked_add(1).ok_or(Error::ResourceLimit)?;
        Ok(aead::Nonce::assume_unique_for_key(nonce))
    }
    pub fn seal(&mut self, kind: u8, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() > 16384 {
            return Err(Error::ResourceLimit);
        }
        let n = data.len() + 17;
        let mut out = vec![23, 3, 3, (n >> 8) as u8, n as u8];
        let aad = out.clone();
        let mut body = data.to_vec();
        body.push(kind);
        let nonce = self.nonce()?;
        self.key
            .seal_in_place_append_tag(nonce, aead::Aad::from(aad), &mut body)
            .map_err(|_| Error::Tls)?;
        out.extend(body);
        Ok(out)
    }
    pub fn open(&mut self, record: &[u8]) -> Result<(u8, Zeroizing<Vec<u8>>)> {
        if record.len() < 22
            || record.len() > MAX_RECORD + 5
            || record[..3] != [23, 3, 3]
            || usize::from(u16::from_be_bytes([record[3], record[4]])) != record.len() - 5
        {
            return Err(Error::Tls);
        }
        let mut body = Zeroizing::new(record[5..].to_vec());
        let nonce = self.nonce()?;
        let plain = self
            .key
            .open_in_place(nonce, aead::Aad::from(&record[..5]), &mut body)
            .map_err(|_| Error::Tls)?;
        let last = plain.iter().rposition(|b| *b != 0).ok_or(Error::Tls)?;
        let kind = plain[last];
        body.truncate(last);
        Ok((kind, body))
    }
    pub fn update(&mut self) -> Result<()> {
        let secret = self
            .suite
            .expand(&self.secret, "traffic upd", &[], self.suite.hash_len())?;
        *self = Self::new(secret, self.suite)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/test_protocol_crypto.rs"]
mod test_protocol_crypto;
