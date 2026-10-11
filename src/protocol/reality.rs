//! REALITY 原生 TLS 1.3 客户端；密码原语由 ring/X25519 实现，业务层不使用 unsafe。
//! 不修改 rustls 私有状态，不接受真实站点证书作为代理认证成功。
use super::{
    config::Node,
    crypto::{Cipher, Size, Suite},
};
use crate::error::{Error, Result};
use ring::{
    aead, hkdf, hmac,
    rand::{SecureRandom, SystemRandom},
    signature,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;
const MAX_RECORD: usize = 16640;
const MAX_HANDSHAKE: usize = 131072;
fn u16v(out: &mut Vec<u8>, v: &[u8]) -> Result<()> {
    if v.len() > 65535 {
        return Err(Error::ResourceLimit);
    }
    out.extend((v.len() as u16).to_be_bytes());
    out.extend(v);
    Ok(())
}
fn message(kind: u8, body: &[u8]) -> Result<Vec<u8>> {
    if body.len() > MAX_HANDSHAKE {
        return Err(Error::ResourceLimit);
    }
    let mut out = vec![kind];
    out.extend(&(body.len() as u32).to_be_bytes()[1..]);
    out.extend(body);
    Ok(out)
}
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(Error::ResourceLimit)?;
        let value = self.data.get(self.pos..end).ok_or(Error::Tls)?;
        self.pos = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn word(&mut self) -> Result<u16> {
        let v = self.take(2)?;
        Ok(u16::from_be_bytes([v[0], v[1]]))
    }
    fn vec8(&mut self) -> Result<&'a [u8]> {
        let n = self.byte()?;
        self.take(usize::from(n))
    }
    fn vec16(&mut self) -> Result<&'a [u8]> {
        let n = self.word()?;
        self.take(usize::from(n))
    }
    fn vec24(&mut self) -> Result<&'a [u8]> {
        let b = self.take(3)?;
        let n = (usize::from(b[0]) << 16) | (usize::from(b[1]) << 8) | usize::from(b[2]);
        self.take(n)
    }
    fn end(&self) -> Result<()> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(Error::Tls)
        }
    }
}
pub async fn record<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut header = [0; 5];
    let n = reader.read(&mut header[..1]).await?;
    if n == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..]).await?;
    if !matches!(header[0], 20..=23) || header[1] != 3 || header[2] > 3 {
        return Err(Error::Tls);
    }
    let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
    if length == 0 || length > MAX_RECORD {
        return Err(Error::ResourceLimit);
    }
    let mut out = header.to_vec();
    out.resize(5 + length, 0);
    reader.read_exact(&mut out[5..]).await?;
    Ok(Some(out))
}
struct Hello {
    wire: Vec<u8>,
    session: [u8; 32],
    auth: Zeroizing<Vec<u8>>,
    secret: StaticSecret,
    p256: Option<ring::agreement::EphemeralPrivateKey>,
    hybrid: Option<super::hybrid::Hybrid>,
    ciphers: Vec<u8>,
}
fn hello(node: &Node) -> Result<Hello> {
    let rng = SystemRandom::new();
    let mut random = [0; 32];
    let mut private = Zeroizing::new([0; 32]);
    rng.fill(&mut random).map_err(|_| Error::Internal)?;
    rng.fill(private.as_mut()).map_err(|_| Error::Internal)?;
    let secret = StaticSecret::from(*private);
    let public = PublicKey::from(&secret);
    let shared = secret.diffie_hellman(&PublicKey::from(node.public_key));
    if !shared.was_contributory() {
        return Err(Error::Certificate);
    }
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &random[..20]).extract(shared.as_bytes());
    let mut auth = Zeroizing::new(vec![0; 32]);
    prk.expand(&[b"REALITY"], Size(32))
        .map_err(|_| Error::Internal)?
        .fill(&mut auth)
        .map_err(|_| Error::Internal)?;
    let mut body = vec![3, 3];
    body.extend(random);
    body.push(32);
    body.extend([0; 32]);
    let material = super::fingerprint::build(node.fingerprint, &node.sni, public.as_bytes())?;
    u16v(&mut body, &material.ciphers)?;
    body.extend([1, 0]);
    u16v(&mut body, &material.extensions)?;
    let mut wire = message(1, &body)?;
    // REALITY 握手版本标识使用固定互通基线，与 crate 发布版本分别管理。
    let mut plain = vec![26, 3, 27, 0];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::InvalidState)?
        .as_secs();
    let now = u32::try_from(now).map_err(|_| Error::InvalidState)?;
    plain.extend(now.to_be_bytes());
    plain.extend(node.short_id);
    let key = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_256_GCM, &auth).map_err(|_| Error::Tls)?,
    );
    let nonce = aead::Nonce::try_assume_unique_for_key(&random[20..]).map_err(|_| Error::Tls)?;
    key.seal_in_place_append_tag(nonce, aead::Aad::from(wire.as_slice()), &mut plain)
        .map_err(|_| Error::Tls)?;
    let session: [u8; 32] = plain.as_slice().try_into().map_err(|_| Error::Internal)?;
    wire[39..71].copy_from_slice(&session);
    Ok(Hello {
        wire,
        session,
        auth,
        secret,
        p256: material.p256,
        hybrid: material.hybrid,
        ciphers: material.ciphers,
    })
}
fn server_hello(data: &[u8], session: &[u8; 32]) -> Result<(Suite, u16, Vec<u8>)> {
    let mut c = Cursor::new(data);
    if c.take(2)? != [3, 3] {
        return Err(Error::Tls);
    }
    let random = c.take(32)?;
    if random
        == [
            0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65,
            0xb8, 0x91, 0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2,
            0xc8, 0xa8, 0x33, 0x9c,
        ]
    {
        return Err(Error::Unsupported);
    }
    if c.vec8()? != session {
        return Err(Error::Tls);
    }
    let suite = Suite::parse(c.word()?)?;
    if c.byte()? != 0 {
        return Err(Error::Tls);
    }
    let mut e = Cursor::new(c.vec16()?);
    c.end()?;
    let (mut share, mut version) = (None, false);
    let mut group = 0;
    let mut seen = std::collections::BTreeSet::new();
    while e.pos < e.data.len() {
        let kind = e.word()?;
        if !seen.insert(kind) {
            return Err(Error::Tls);
        }
        let val = e.vec16()?;
        match kind {
            43 => {
                if val != [3, 4] {
                    return Err(Error::Tls);
                }
                version = true;
            }
            51 => {
                let mut s = Cursor::new(val);
                group = s.word()?;
                if !matches!(group, 29 | 23 | 4588) {
                    return Err(Error::Unsupported);
                }
                share = Some(s.vec16()?.to_vec());
                s.end()?;
            }
            _ => return Err(Error::Unsupported),
        }
    }
    if !version {
        return Err(Error::Tls);
    }
    Ok((suite, group, share.ok_or(Error::Tls)?))
}
fn certificate(body: &[u8], auth: &[u8]) -> Result<Vec<u8>> {
    let mut c = Cursor::new(body);
    if !c.vec8()?.is_empty() {
        return Err(Error::Tls);
    }
    let mut list = Cursor::new(c.vec24()?);
    c.end()?;
    let der = list.vec24()?;
    if der.len() > 8192 {
        return Err(Error::ResourceLimit);
    }
    let _ = list.vec16()?;
    let (rest, cert) = x509_parser::parse_x509_certificate(der).map_err(|_| Error::Certificate)?;
    if !rest.is_empty() || cert.public_key().algorithm.algorithm.to_id_string() != "1.3.101.112" {
        return Err(Error::Certificate);
    }
    let public = cert.public_key().subject_public_key.data.as_ref();
    if public.len() != 32 {
        return Err(Error::Certificate);
    }
    hmac::verify(
        &hmac::Key::new(hmac::HMAC_SHA512, auth),
        public,
        cert.signature_value.data.as_ref(),
    )
    .map_err(|_| Error::Certificate)?;
    let mut count = 1;
    while list.pos < list.data.len() {
        let _ = list.vec24()?;
        let _ = list.vec16()?;
        count += 1;
        if count > 4 {
            return Err(Error::ResourceLimit);
        }
    }
    Ok(public.to_vec())
}
pub async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    node: &Node,
) -> Result<(Cipher, Cipher)> {
    let mut hello = hello(node)?;
    let mut first = vec![22, 3, 1];
    u16v(&mut first, &hello.wire)?;
    stream.write_all(&first).await?;
    let server = record(stream).await?.ok_or(Error::Tls)?;
    if server[0] != 22 {
        return Err(Error::Tls);
    }
    let mut c = Cursor::new(&server[5..]);
    if c.byte()? != 2 {
        return Err(Error::Tls);
    }
    let body = c.vec24()?;
    c.end()?;
    let (suite, group, key) = server_hello(body, &hello.session)?;
    if !hello
        .ciphers
        .as_chunks::<2>()
        .0
        .iter()
        .any(|s| u16::from_be_bytes(*s) == suite.id())
    {
        return Err(Error::Tls);
    }
    let shared = if group == 29 {
        let peer: [u8; 32] = key.as_slice().try_into().map_err(|_| Error::Tls)?;
        let value = hello.secret.diffie_hellman(&PublicKey::from(peer));
        if !value.was_contributory() {
            return Err(Error::Tls);
        }
        Zeroizing::new(value.as_bytes().to_vec())
    } else if group == 4588 {
        hello
            .hybrid
            .as_ref()
            .ok_or(Error::Unsupported)?
            .agree(&key)?
    } else {
        let secret = hello.p256.take().ok_or(Error::Unsupported)?;
        ring::agreement::agree_ephemeral(
            secret,
            &ring::agreement::UnparsedPublicKey::new(&ring::agreement::ECDH_P256, key),
            |bytes| Zeroizing::new(bytes.to_vec()),
        )
        .map_err(|_| Error::Tls)?
    };
    let mut transcript = hello.wire.clone();
    transcript.extend(&server[5..]);
    let empty = suite.hash(&[]);
    let early = suite.extract(&vec![0; suite.hash_len()], &vec![0; suite.hash_len()])?;
    let derived = suite.expand(&early, "derived", &empty, suite.hash_len())?;
    let hs = suite.extract(&derived, &shared)?;
    let client_secret = suite.expand(
        &hs,
        "c hs traffic",
        &suite.hash(&transcript),
        suite.hash_len(),
    )?;
    let server_secret = suite.expand(
        &hs,
        "s hs traffic",
        &suite.hash(&transcript),
        suite.hash_len(),
    )?;
    let finished_key = suite.expand(&server_secret, "finished", &[], suite.hash_len())?;
    let mut rx = Cipher::new(server_secret, suite)?;
    let mut pending = Zeroizing::new(Vec::new());
    let mut stage = 0;
    let mut public = Vec::new();
    let mut controls = 0;
    loop {
        let frame = record(stream).await?.ok_or(Error::Tls)?;
        controls += 1;
        if controls > 128 {
            return Err(Error::ResourceLimit);
        }
        if frame[0] == 20 {
            if frame[5..] != [1] {
                return Err(Error::Tls);
            }
            continue;
        }
        let (kind, data) = rx.open(&frame)?;
        if kind != 22 {
            return Err(Error::Tls);
        }
        if pending.len() + data.len() > MAX_HANDSHAKE {
            return Err(Error::ResourceLimit);
        }
        pending.extend(data.iter());
        loop {
            if pending.len() < 4 {
                break;
            }
            let n = (usize::from(pending[1]) << 16)
                | (usize::from(pending[2]) << 8)
                | usize::from(pending[3]);
            if n > MAX_HANDSHAKE {
                return Err(Error::ResourceLimit);
            }
            if pending.len() < n + 4 {
                break;
            }
            let kind = pending[0];
            let body = &pending[4..n + 4];
            match (stage, kind) {
                (0, 8) => {
                    let mut c = Cursor::new(body);
                    let mut e = Cursor::new(c.vec16()?);
                    c.end()?;
                    let mut seen = std::collections::BTreeSet::new();
                    while e.pos < e.data.len() {
                        let tag = e.word()?;
                        if !seen.insert(tag) {
                            return Err(Error::Tls);
                        }
                        let v = e.vec16()?;
                        match tag {
                            0 if v.is_empty() => {}
                            // TLS 1.3 服务端可回传组偏好；校验格式，不改变当前已选密钥交换。
                            10 => {
                                let mut groups = Cursor::new(v);
                                let list = groups.vec16()?;
                                groups.end()?;
                                if list.is_empty()
                                    || list.len() > 128
                                    || !list.len().is_multiple_of(2)
                                {
                                    return Err(Error::Tls);
                                }
                            }
                            16 => {
                                if !matches!(
                                    v,
                                    [0, 3, 2, b'h', b'2']
                                        | [0, 9, 8, b'h', b't', b't', b'p', b'/', b'1', b'.', b'1']
                                ) {
                                    return Err(Error::Tls);
                                }
                            }
                            _ => return Err(Error::Unsupported),
                        }
                    }
                }
                (1, 11) => public = certificate(body, &hello.auth)?,
                (2, 15) => {
                    let mut c = Cursor::new(body);
                    if c.word()? != 0x0807 {
                        return Err(Error::Unsupported);
                    }
                    let sig = c.vec16()?;
                    c.end()?;
                    let mut signed = vec![32; 64];
                    signed.extend(b"TLS 1.3, server CertificateVerify");
                    signed.push(0);
                    signed.extend(suite.hash(&transcript));
                    signature::UnparsedPublicKey::new(&signature::ED25519, &public)
                        .verify(&signed, sig)
                        .map_err(|_| Error::Tls)?;
                }
                (3, 20) => hmac::verify(
                    &hmac::Key::new(suite.mac(), &finished_key),
                    &suite.hash(&transcript),
                    body,
                )
                .map_err(|_| Error::Tls)?,
                _ => return Err(Error::Tls),
            }
            if transcript.len() + n + 4 > MAX_HANDSHAKE {
                return Err(Error::ResourceLimit);
            }
            transcript.extend(&pending[..n + 4]);
            pending.drain(..n + 4);
            stage += 1;
            if stage == 4 {
                if !pending.is_empty() {
                    return Err(Error::Tls);
                }
                let derived = suite.expand(&hs, "derived", &empty, suite.hash_len())?;
                let master = suite.extract(&derived, &vec![0; suite.hash_len()])?;
                let context = suite.hash(&transcript);
                let tx = Cipher::new(
                    suite.expand(&master, "c ap traffic", &context, suite.hash_len())?,
                    suite,
                )?;
                let rx = Cipher::new(
                    suite.expand(&master, "s ap traffic", &context, suite.hash_len())?,
                    suite,
                )?;
                let client_finished =
                    suite.expand(&client_secret, "finished", &[], suite.hash_len())?;
                let verify = hmac::sign(&hmac::Key::new(suite.mac(), &client_finished), &context);
                let mut hs_tx = Cipher::new(client_secret, suite)?;
                let final_message = message(20, verify.as_ref())?;
                stream.write_all(&hs_tx.seal(22, &final_message)?).await?;
                return Ok((rx, tx));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_cipher_authentication_sequence_and_update() {
        let key = Zeroizing::new(vec![1; 32]);
        let mut tx = Cipher::new(key.clone(), Suite::Aes128).unwrap();
        let mut rx = Cipher::new(key, Suite::Aes128).unwrap();
        let frame = tx.seal(23, b"payload").unwrap();
        assert_eq!(rx.open(&frame).unwrap().1.as_slice(), b"payload");
        assert!(rx.open(&frame).is_err());
        tx.update().unwrap();
        rx.update().unwrap();
        let mut frame = tx.seal(23, b"new").unwrap();
        frame[7] ^= 1;
        assert!(rx.open(&frame).is_err());
    }
}
