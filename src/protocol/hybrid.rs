//! X25519MLKEM768 密钥交换；与普通 X25519 分离，秘密通过 RAII 清零。
use crate::error::{Error, Result};
use ml_kem::{Decapsulate, KeyExport};
use ring::rand::{SecureRandom, SystemRandom};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;
pub struct Hybrid {
    kem: Box<ml_kem::DecapsulationKey768>,
    secret: StaticSecret,
}
impl Hybrid {
    pub fn generate() -> Result<(Self, Vec<u8>)> {
        let rng = SystemRandom::new();
        let mut seed = Zeroizing::new([0; 64]);
        let mut private = Zeroizing::new([0; 32]);
        rng.fill(seed.as_mut()).map_err(|_| Error::Internal)?;
        rng.fill(private.as_mut()).map_err(|_| Error::Internal)?;
        let kem = Box::new(ml_kem::DecapsulationKey768::from_seed((*seed).into()));
        let secret = StaticSecret::from(*private);
        let mut share = kem.encapsulation_key().to_bytes().to_vec();
        share.extend(PublicKey::from(&secret).as_bytes());
        Ok((Self { kem, secret }, share))
    }
    pub fn agree(&self, wire: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        if wire.len() != 1088 + 32 {
            return Err(Error::Tls);
        }
        let ciphertext = ml_kem::Ciphertext::<ml_kem::MlKem768>::try_from(&wire[..1088])
            .map_err(|_| Error::Tls)?;
        let mut shared = self.kem.decapsulate(&ciphertext);
        let peer: [u8; 32] = wire[1088..].try_into().map_err(|_| Error::Tls)?;
        let classical = self.secret.diffie_hellman(&PublicKey::from(peer));
        if !classical.was_contributory() {
            return Err(Error::Tls);
        }
        let mut result = Zeroizing::new(shared.to_vec());
        use zeroize::Zeroize;
        shared.zeroize();
        result.extend(classical.as_bytes());
        Ok(result)
    }
}
