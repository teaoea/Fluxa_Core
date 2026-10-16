//! 客户端与监听服务共用响应；保留标准报文，不从展示文本重新解析记录。
use super::{Query, Resolver};
use crate::error::{Error, Result};
use hickory_resolver::proto::{
    op::{Edns, Message, MessageType, OpCode, ResponseCode},
    rr::{
        DNSClass, Name, RData, Record, RecordType,
        rdata::{A, AAAA},
    },
};
pub(super) fn encode(message: &Message) -> Result<Vec<u8>> {
    if message.answers.len() + message.authorities.len() + message.additionals.len() > 256 {
        return Err(Error::ResourceLimit);
    }
    let bytes = message.to_vec().map_err(|_| Error::DnsResponse)?;
    if bytes.len() > 65535 {
        return Err(Error::ResourceLimit);
    }
    Ok(bytes)
}
pub(super) fn synthetic(name: &str, address: &str, kind: RecordType) -> Result<Vec<u8>> {
    let data = if kind == RecordType::A {
        RData::A(A(address.parse().map_err(|_| Error::Internal)?))
    } else {
        RData::AAAA(AAAA(address.parse().map_err(|_| Error::Internal)?))
    };
    let mut message = Message::query();
    message.add_answer(Record::from_rdata(
        Name::from_ascii(format!("{name}.")).map_err(|_| Error::InvalidInput)?,
        60,
        data,
    ));
    encode(&message)
}
fn response(request: &Message, code: ResponseCode) -> Message {
    let mut result = Message::response(request.metadata.id, request.metadata.op_code);
    result.metadata.response_code = code;
    result.metadata.recursion_available = true;
    result.metadata.recursion_desired = request.metadata.recursion_desired;
    result.metadata.checking_disabled = request.metadata.checking_disabled;
    if request.queries.len() == 1 {
        result.queries = request.queries.clone();
    }
    result
}
/// 畸形报文仅回应小型 FORMERR；编码失败必须返回 DNS 错误，不能伪造空报文成功。
pub(super) fn malformed(bytes: &[u8]) -> Result<Vec<u8>> {
    let id = bytes
        .get(..2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .unwrap_or(0);
    Message::error_msg(id, OpCode::Query, ResponseCode::FormErr)
        .to_vec()
        .map_err(|_| Error::DnsResponse)
}
impl Resolver {
    pub async fn respond(&self, bytes: &[u8], udp: bool) -> Result<Vec<u8>> {
        // 退休/关闭实例连畸形报文也不能继续提供服务；不能用 FORMERR 成功绕过准入。
        self.ensure_active()?;
        if !(12..=4096).contains(&bytes.len()) {
            return malformed(bytes);
        }
        let Ok(request) = Message::from_vec(bytes) else {
            return malformed(bytes);
        };
        if request.metadata.message_type != MessageType::Query {
            return Err(Error::InvalidInput);
        }
        let max_payload = request
            .edns
            .as_ref()
            .map_or(512, |e| e.max_payload().clamp(512, 1232)) as usize;
        let mut result = if request.metadata.op_code != OpCode::Query {
            response(&request, ResponseCode::NotImp)
        } else if request.queries.len() != 1
            || !request.answers.is_empty()
            || !request.authorities.is_empty()
        {
            response(&request, ResponseCode::FormErr)
        } else if request.signature.is_some() || !request.additionals.is_empty() {
            response(&request, ResponseCode::Refused)
        } else if request.edns.as_ref().is_some_and(|e| e.version() != 0) {
            response(&request, ResponseCode::BADVERS)
        } else if request.queries[0].query_class != DNSClass::IN
            || !request.metadata.recursion_desired
        {
            response(&request, ResponseCode::Refused)
        } else {
            let question = &request.queries[0];
            match self
                .resolve(Query {
                    name: question.name.to_ascii(),
                    query_type: question.query_type.to_string(),
                })
                .await
            {
                Ok(answer) => {
                    let mut result =
                        Message::from_vec(&answer.wire).map_err(|_| Error::Internal)?;
                    result.metadata =
                        response(&request, <ResponseCode as From<u16>>::from(answer.rcode))
                            .metadata;
                    result.queries = request.queries.clone();
                    result.metadata.authentic_data = answer.validation == "secure"
                        && (request.metadata.authentic_data
                            || request.edns.as_ref().is_some_and(|e| e.flags().dnssec_ok));
                    for record in result
                        .answers
                        .iter_mut()
                        .chain(&mut result.authorities)
                        .chain(&mut result.additionals)
                    {
                        record.ttl = record.ttl.min(answer.ttl);
                    }
                    if !request.edns.as_ref().is_some_and(|e| e.flags().dnssec_ok) {
                        for records in [
                            &mut result.answers,
                            &mut result.authorities,
                            &mut result.additionals,
                        ] {
                            records.retain(|r| {
                                r.record_type() == question.query_type
                                    || !matches!(
                                        r.record_type(),
                                        RecordType::RRSIG | RecordType::NSEC | RecordType::NSEC3
                                    )
                            });
                        }
                    }
                    result
                }
                Err(Error::InvalidInput) => response(&request, ResponseCode::FormErr),
                Err(Error::Unsupported) => response(&request, ResponseCode::Refused),
                Err(_) => response(&request, ResponseCode::ServFail),
            }
        };
        result.edns = request.edns.as_ref().map(|_| {
            let mut edns = Edns::new();
            edns.set_max_payload(1232);
            edns.set_dnssec_ok(request.edns.as_ref().is_some_and(|e| e.flags().dnssec_ok));
            edns
        });
        let mut encoded = encode(&result)?;
        if udp && encoded.len() > max_payload {
            result.answers.clear();
            result.authorities.clear();
            result.additionals.clear();
            result.metadata.truncation = true;
            result.metadata.authentic_data = false;
            encoded = encode(&result)?;
        }
        Ok(encoded)
    }
}
