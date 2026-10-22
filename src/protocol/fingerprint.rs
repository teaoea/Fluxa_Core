//! 固定浏览器 ClientHello 线路模板；随机字段每次生成，模板不会绕过 REALITY 认证。
//! 模板名称指 TLS 握手形状，不包含 HTTP/2、JavaScript 或浏览器行为模拟。
use crate::error::{Error, Result};
use ring::{
    agreement,
    rand::{SecureRandom, SystemRandom},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fingerprint {
    Nodus,
    Chrome,
    Chrome131,
    Chrome133,
    Firefox,
    Safari,
    Ios,
    Android,
    Android133,
    Edge,
}
impl Fingerprint {
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "nodus" => Self::Nodus,
            "chrome120" => Self::Chrome,
            "chrome131" => Self::Chrome131,
            "chrome" | "chrome133" => Self::Chrome133,
            "firefox" | "firefox120" => Self::Firefox,
            "safari" | "safari16" => Self::Safari,
            "ios" | "ios14" => Self::Ios,
            "android-chrome120" => Self::Android,
            "android" | "android-chrome133" => Self::Android133,
            "edge" | "edge106" => Self::Edge,
            "random" => {
                let mut n = [0];
                SystemRandom::new()
                    .fill(&mut n)
                    .map_err(|_| Error::Internal)?;
                [
                    Self::Chrome133,
                    Self::Firefox,
                    Self::Safari,
                    Self::Ios,
                    Self::Android133,
                    Self::Edge,
                ][usize::from(n[0]) % 6]
            }
            _ => return Err(Error::Unsupported),
        })
    }
    pub fn version(self) -> &'static str {
        match self {
            Self::Nodus => "Nodus TLS 1.3",
            Self::Chrome => "Chrome 120",
            Self::Chrome131 => "Chrome 131",
            Self::Chrome133 => "Chrome 133",
            Self::Firefox => "Firefox 120",
            Self::Safari => "Safari 16.0",
            Self::Ios => "iOS 14",
            Self::Android => "Android Chrome 120",
            Self::Android133 => "Android Chrome 133",
            Self::Edge => "Edge 106",
        }
    }
}
pub struct Material {
    pub hybrid: Option<super::hybrid::Hybrid>,
    pub ciphers: Vec<u8>,
    pub extensions: Vec<u8>,
    pub p256: Option<agreement::EphemeralPrivateKey>,
}
fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}
fn vector(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() > 65535 {
        return Err(Error::ResourceLimit);
    }
    let mut out = (bytes.len() as u16).to_be_bytes().to_vec();
    out.extend(bytes);
    Ok(out)
}
fn ext(tag: u16, body: Vec<u8>) -> (u16, Vec<u8>) {
    (tag, body)
}
fn ech(firefox: bool, rng: &SystemRandom) -> Result<Vec<u8>> {
    let mut noise = [0; 256];
    rng.fill(&mut noise).map_err(|_| Error::Internal)?;
    let length = if firefox {
        239
    } else {
        [144, 176, 208, 240][usize::from(noise[0]) % 4]
    };
    let mut out = vec![
        0,
        0,
        1,
        0,
        if firefox && noise[1] & 1 != 0 { 3 } else { 1 },
        noise[2],
    ];
    let ephemeral = agreement::EphemeralPrivateKey::generate(&agreement::X25519, rng)
        .map_err(|_| Error::Internal)?;
    let encapsulated = ephemeral
        .compute_public_key()
        .map_err(|_| Error::Internal)?;
    out.extend(vector(encapsulated.as_ref())?);
    out.extend(vector(&noise[..length])?);
    Ok(out)
}
/// 密码能力保持在协议实现中；此处只构造公开的有界握手字段。
pub fn build(kind: Fingerprint, sni: &str, share: &[u8; 32]) -> Result<Material> {
    if sni.is_empty() || sni.len() > 253 {
        return Err(Error::InvalidInput);
    }
    let rng = SystemRandom::new();
    let mut random = [0; 64];
    rng.fill(&mut random).map_err(|_| Error::Internal)?;
    let grease = |n: u8| {
        let b = (n % 16) * 16 + 10;
        u16::from_be_bytes([b, b])
    };
    let g = [
        grease(random[0]),
        grease(random[1]),
        grease(random[2]),
        grease(random[3]),
    ];
    let grease_end = if g[1] == grease(random[4]) {
        grease(random[4].wrapping_add(1))
    } else {
        grease(random[4])
    };
    let firefox = kind == Fingerprint::Firefox;
    let apple = matches!(kind, Fingerprint::Safari | Fingerprint::Ios);
    let native = kind == Fingerprint::Nodus;
    let chrome = matches!(
        kind,
        Fingerprint::Chrome
            | Fingerprint::Chrome131
            | Fingerprint::Chrome133
            | Fingerprint::Android
            | Fingerprint::Android133
    );
    let modern = matches!(
        kind,
        Fingerprint::Chrome131 | Fingerprint::Chrome133 | Fingerprint::Android133
    );
    let mut suites = if native {
        vec![0x1301]
    } else if firefox {
        vec![
            0x1301, 0x1303, 0x1302, 0xc02b, 0xc02f, 0xcca9, 0xcca8, 0xc02c, 0xc030, 0xc00a, 0xc009,
            0xc013, 0xc014, 0x009c, 0x009d, 0x002f, 0x0035,
        ]
    } else if apple {
        let mut c = vec![
            g[0], 0x1301, 0x1302, 0x1303, 0xc02c, 0xc02b, 0xcca9, 0xc030, 0xc02f, 0xcca8,
        ];
        if kind == Fingerprint::Ios {
            c.extend([0xc024, 0xc023]);
        }
        c.extend([0xc00a, 0xc009]);
        if kind == Fingerprint::Ios {
            c.extend([0xc028, 0xc027]);
        }
        c.extend([0xc014, 0xc013, 0x009d, 0x009c]);
        if kind == Fingerprint::Ios {
            c.extend([0x003d, 0x003c]);
        }
        c.extend([0x0035, 0x002f, 0xc008, 0xc012, 0x000a]);
        c
    } else {
        vec![
            g[0], 0x1301, 0x1302, 0x1303, 0xc02b, 0xc02f, 0xc02c, 0xc030, 0xcca9, 0xcca8, 0xc013,
            0xc014, 0x009c, 0x009d, 0x002f, 0x0035,
        ]
    };
    if suites.len() > 32 {
        return Err(Error::Internal);
    }
    let ciphers = words(&suites);
    suites.clear();
    let mut name = vec![0];
    name.extend(vector(sni.as_bytes())?);
    let name = vector(&name)?;
    let mut groups = if firefox {
        vec![29, 23, 24, 25, 256, 257]
    } else if apple {
        vec![g[2], 29, 23, 24, 25]
    } else if native {
        vec![29]
    } else {
        vec![g[2], 29, 23, 24]
    };
    if modern {
        groups.insert(1, 4588);
    }
    let group_body = vector(&words(&groups))?;
    groups.clear();
    let sigs = if native {
        vec![0x0807, 0x0804, 0x0403]
    } else if firefox {
        vec![
            0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806, 0x0401, 0x0501, 0x0601, 0x0203, 0x0201,
        ]
    } else if apple {
        vec![
            0x0403, 0x0804, 0x0401, 0x0503, 0x0203, 0x0805, 0x0805, 0x0501, 0x0806, 0x0601, 0x0201,
        ]
    } else {
        vec![
            0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501, 0x0806, 0x0601,
        ]
    };
    let mut shares = Vec::new();
    if !native && !firefox {
        shares.extend(g[2].to_be_bytes());
        shares.extend([0, 1, 0]);
    }
    let hybrid = if modern {
        let (key, wire) = super::hybrid::Hybrid::generate()?;
        shares.extend(4588u16.to_be_bytes());
        shares.extend(vector(&wire)?);
        Some(key)
    } else {
        None
    };
    shares.extend(29u16.to_be_bytes());
    shares.extend(vector(share)?);
    let p256 = if firefox {
        let key = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng)
            .map_err(|_| Error::Internal)?;
        let public = key.compute_public_key().map_err(|_| Error::Internal)?;
        shares.extend(23u16.to_be_bytes());
        shares.extend(vector(public.as_ref())?);
        Some(key)
    } else {
        None
    };
    let versions = if native {
        vec![3, 4]
    } else if firefox {
        words(&[0x0304, 0x0303])
    } else if apple {
        words(&[g[3], 0x0304, 0x0303, 0x0302, 0x0301])
    } else {
        words(&[g[3], 0x0304, 0x0303])
    };
    let mut versions_body = vec![versions.len() as u8];
    versions_body.extend(versions);
    let alpn = vector(b"\x02h2\x08http/1.1")?;
    let mut middle = vec![ext(0, name)];
    if !native {
        middle.extend([ext(23, vec![]), ext(65281, vec![0])]);
    }
    middle.push(ext(10, group_body));
    if !native {
        middle.push(ext(11, vec![1, 0]));
    }
    if !native && !apple {
        middle.push(ext(35, vec![]));
    }
    middle.push(ext(16, alpn));
    if !native {
        middle.push(ext(5, vec![1, 0, 0, 0, 0]));
    }
    if firefox {
        middle.push(ext(34, vector(&words(&[0x0403, 0x0503, 0x0603, 0x0203]))?));
    }
    if !firefox {
        middle.push(ext(13, vector(&words(&sigs))?));
        if !native {
            middle.push(ext(18, vec![]));
        }
    }
    middle.push(ext(51, vector(&shares)?));
    if !firefox && !native {
        middle.push(ext(45, vec![1, 1]));
    }
    middle.push(ext(43, versions_body));
    if firefox {
        middle.extend([
            ext(13, vector(&words(&sigs))?),
            ext(45, vec![1, 1]),
            ext(28, vec![0x40, 1]),
            ext(65037, ech(true, &rng)?),
        ]);
    }
    if chrome || kind == Fingerprint::Edge {
        middle.extend([
            ext(27, vec![2, 0, 2]),
            ext(
                if matches!(kind, Fingerprint::Chrome133 | Fingerprint::Android133) {
                    17613
                } else {
                    17513
                },
                vector(b"\x02h2")?,
            ),
        ]);
    }
    if chrome {
        middle.push(ext(65037, ech(false, &rng)?));
    }
    if kind == Fingerprint::Safari {
        middle.push(ext(27, vec![2, 0, 1]));
    }
    if chrome {
        for i in (1..middle.len()).rev() {
            let mut pick = [0; 4];
            rng.fill(&mut pick).map_err(|_| Error::Internal)?;
            let j = (u32::from_be_bytes(pick) as usize) % (i + 1);
            middle.swap(i, j);
        }
    }
    let mut extensions = Vec::new();
    if !native && !firefox {
        extensions.extend(g[1].to_be_bytes());
        extensions.extend([0, 0]);
    }
    for (tag, body) in middle {
        extensions.extend(tag.to_be_bytes());
        extensions.extend(vector(&body)?);
    }
    if !native && !firefox {
        extensions.extend(grease_end.to_be_bytes());
        extensions.extend([0, 1, 0]);
    }
    // BoringSSL 风格填充：256..511 字节的 ClientHello 补到 512，填充扩展始终最后。
    let hello_len = 4 + 2 + 32 + 1 + 32 + 2 + ciphers.len() + 2 + 2 + extensions.len();
    if !native && !firefox && !modern && (256..512).contains(&hello_len) {
        let n = (512 - hello_len).saturating_sub(4);
        extensions.extend(21u16.to_be_bytes());
        extensions.extend(vector(&vec![0; n])?);
    }
    if extensions.len() > 4096 {
        return Err(Error::ResourceLimit);
    }
    Ok(Material {
        hybrid,
        ciphers,
        extensions,
        p256,
    })
}

#[cfg(test)]
#[path = "../../tests/test_protocol_fingerprint.rs"]
mod test_protocol_fingerprint;
