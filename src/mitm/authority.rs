//! CA 和站点证书均由 Rust 生成，Apple 层只负责钥匙串保存和公开证书分享。
use crate::error::{Error, Result};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose,
};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivatePkcs8KeyDer, pem::PemObject},
};
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use zeroize::Zeroizing;

pub struct Authority {
    issuer: Issuer<'static, KeyPair>,
    certificate: CertificateDer<'static>,
    pem: String,
    expires_at: OffsetDateTime,
}
impl Authority {
    pub fn generate() -> Result<Self> {
        let key = KeyPair::generate().map_err(|_| Error::Certificate)?;
        let mut params =
            CertificateParams::new(Vec::<String>::new()).map_err(|_| Error::Certificate)?;
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "Fluxa Local Debug CA");
        params.not_before = OffsetDateTime::now_utc() - Duration::minutes(5);
        params.not_after = OffsetDateTime::now_utc() + Duration::days(1095);
        let certificate = params.self_signed(&key).map_err(|_| Error::Certificate)?;
        Ok(Self {
            pem: certificate.pem(),
            certificate: certificate.der().clone(),
            expires_at: params.not_after,
            issuer: Issuer::new(params, key),
        })
    }

    pub fn restore(certificate_pem: &str, private_key_pem: &str) -> Result<Self> {
        let key = KeyPair::from_pem(private_key_pem).map_err(|_| Error::Certificate)?;
        let certificate = CertificateDer::from_pem_slice(certificate_pem.as_bytes())
            .map_err(|_| Error::Certificate)?;
        let (_, parsed) = x509_parser::parse_x509_certificate(certificate.as_ref())
            .map_err(|_| Error::Certificate)?;
        if !parsed.is_ca()
            || !parsed.validity().is_valid()
            || parsed.subject() != parsed.issuer()
            || parsed.verify_signature(None).is_err()
        {
            return Err(Error::Certificate);
        }
        let expires_at = parsed.validity().not_after.to_datetime();
        // 校验保存的证书与私钥匹配；错误时拒绝生成新的 CA 以免已信任证书悄然失效。
        ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|_| Error::Certificate)?
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.clone()],
                PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
            )
            .map_err(|_| Error::Certificate)?;
        let issuer =
            Issuer::from_ca_cert_pem(certificate_pem, key).map_err(|_| Error::Certificate)?;
        if !issuer.key_usages().contains(&KeyUsagePurpose::KeyCertSign) {
            return Err(Error::Certificate);
        }
        Ok(Self {
            issuer,
            certificate,
            pem: certificate_pem.to_owned(),
            expires_at,
        })
    }
    pub fn certificate_der(&self) -> &[u8] {
        self.certificate.as_ref()
    }
    pub fn certificate_pem(&self) -> &str {
        &self.pem
    }
    /// 仅给平台安全存储使用，禁止加入日志或公开证书导出。
    pub fn private_key_pem(&self) -> Zeroizing<String> {
        Zeroizing::new(self.issuer.key().serialize_pem())
    }

    pub fn server_config(&self, host: &str) -> Result<Arc<ServerConfig>> {
        let host = crate::https::normalize_host(host)?;
        let key = KeyPair::generate().map_err(|_| Error::Certificate)?;
        let mut params =
            CertificateParams::new(vec![host.clone()]).map_err(|_| Error::Certificate)?;
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, host);
        params.not_before = OffsetDateTime::now_utc() - Duration::minutes(5);
        params.not_after = (OffsetDateTime::now_utc() + Duration::days(7)).min(self.expires_at);
        if params.not_after <= OffsetDateTime::now_utc() {
            return Err(Error::Certificate);
        }
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let leaf = params
            .signed_by(&key, &self.issuer)
            .map_err(|_| Error::Certificate)?;
        let mut config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .map_err(|_| Error::Certificate)?
                .with_no_client_auth()
                .with_single_cert(
                    vec![leaf.der().clone()],
                    PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
                )
                .map_err(|_| Error::Certificate)?;
        // 仅协商已实现的 HTTP/1.1，不误接收 HTTP/2 二进制帧。
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Arc::new(config))
    }
}
