//! 私有签名根区域测试服务：每次生成 Ed25519 锚，真实 UDP 报文验证，不使用上游 AD 声明。
use hickory_resolver::proto::{
    dnssec::{
        DnssecSigner, SigningKey, TrustAnchors,
        crypto::Ed25519SigningKey,
        rdata::{DNSKEY, DNSSECRData, NSEC, RRSIG},
    },
    op::{Message, MessageType},
    rr::{
        DNSClass, Name, RData, Record, RecordSet, RecordType,
        rdata::{A, SOA},
    },
};
use std::{sync::Arc, time::Duration};
use tokio::{net::UdpSocket, task::JoinHandle};
pub struct Fixture {
    pub port: u16,
    pub anchors: TrustAnchors,
    task: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub async fn fixture() -> Fixture {
    let key = Ed25519SigningKey::from_pkcs8(&Ed25519SigningKey::generate_pkcs8().unwrap()).unwrap();
    let public = key.to_public_key().unwrap();
    let mut anchors = TrustAnchors::empty();
    anchors.insert(&public);
    let dnskey = DNSKEY::new(true, true, false, public);
    let signer = Arc::new(DnssecSigner::new(
        dnskey.clone(),
        Box::new(key),
        Name::root(),
        Duration::from_secs(3600),
    ));
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let mut bytes = [0; 4096];
        loop {
            let Ok((size, peer)) = socket.recv_from(&mut bytes).await else {
                break;
            };
            let Ok(request) = Message::from_vec(&bytes[..size]) else {
                continue;
            };
            let Some(question) = request.queries.first().cloned() else {
                continue;
            };
            let mut reply = Message::query();
            reply.metadata.id = request.metadata.id;
            reply.metadata.message_type = MessageType::Response;
            reply.metadata.authoritative = true;
            reply.metadata.recursion_available = true;
            reply.add_query(question.clone());
            if question.name.to_ascii().starts_with("nxdomain.") {
                reply.metadata.response_code = hickory_resolver::proto::op::ResponseCode::NXDomain;
                for data in [
                    RData::SOA(SOA::new(Name::root(), Name::root(), 1, 60, 60, 300, 5)),
                    RData::DNSSEC(DNSSECRData::NSEC(NSEC::new(
                        Name::root(),
                        vec![
                            RecordType::DNSKEY,
                            RecordType::SOA,
                            RecordType::NSEC,
                            RecordType::RRSIG,
                        ],
                    ))),
                ] {
                    let record = Record::from_rdata(Name::root(), 30, data);
                    let mut set = RecordSet::new(Name::root(), record.record_type(), 1);
                    set.set_ttl(30);
                    set.insert(record.clone(), 1);
                    let sig = RRSIG::from_rrset(
                        &set,
                        DNSClass::IN,
                        time::OffsetDateTime::now_utc() - time::Duration::seconds(60),
                        &signer,
                    )
                    .unwrap();
                    reply.add_authority(record);
                    reply.add_authority(Record::from_rdata(
                        Name::root(),
                        30,
                        RData::DNSSEC(DNSSECRData::RRSIG(sig)),
                    ));
                }
                let _ = socket.send_to(&reply.to_vec().unwrap(), peer).await;
                continue;
            }
            let data = match question.query_type {
                RecordType::DNSKEY => RData::DNSSEC(DNSSECRData::DNSKEY(dnskey.clone())),
                RecordType::A => RData::A(A::new(192, 0, 2, 55)),
                _ => {
                    let _ = socket.send_to(&reply.to_vec().unwrap(), peer).await;
                    continue;
                }
            };
            let record = Record::from_rdata(question.name.clone(), 60, data);
            let mut set = RecordSet::new(question.name.clone(), question.query_type, 1);
            set.set_ttl(60);
            set.insert(record.clone(), 1);
            let expired = question.name.to_ascii().starts_with("expired.");
            let inception = time::OffsetDateTime::now_utc()
                - time::Duration::seconds(if expired { 7200 } else { 60 });
            let signature = RRSIG::from_rrset(&set, DNSClass::IN, inception, &signer).unwrap();
            let mut signature_record = Record::from_rdata(
                question.name,
                60,
                RData::DNSSEC(DNSSECRData::RRSIG(signature)),
            );
            if record.name.to_ascii().starts_with("bogus.") {
                signature_record.ttl = 60;
                reply.add_answer(Record::from_rdata(
                    record.name,
                    60,
                    RData::A(A::new(192, 0, 2, 99)),
                ));
            } else {
                reply.add_answer(record);
            }
            reply.add_answer(signature_record);
            let _ = socket.send_to(&reply.to_vec().unwrap(), peer).await;
        }
    });
    Fixture {
        port,
        anchors,
        task,
    }
}
