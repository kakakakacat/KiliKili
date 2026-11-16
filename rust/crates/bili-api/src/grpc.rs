//! ADR-012 的 unary 网关：普通 POST 承载帧，状态只从响应头读取。
use std::io::{Read, Write};

use base64::{Engine, engine::general_purpose::STANDARD};
use bili_domain::error::{DomainError, ErrorKind};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};

use crate::http::{
    HttpBytesResponse, HttpPost, HttpTransport, PostBody, RequestContext, RetryPolicy,
};
use crate::protobuf::{Reader, WireType};

fn protocol(detail: impl Into<String>) -> DomainError {
    DomainError::new(ErrorKind::Protocol, detail)
}

/// 与基线相同：64 字节本身不压缩，超过才 gzip。
pub fn frame_request(payload: &[u8]) -> Result<Vec<u8>, DomainError> {
    let compressed = payload.len() > 64;
    let body = if compressed {
        let mut writer = GzEncoder::new(Vec::new(), Compression::default());
        writer
            .write_all(payload)
            .map_err(|_| protocol("gRPC 请求压缩失败"))?;
        writer.finish().map_err(|_| protocol("gRPC 请求压缩失败"))?
    } else {
        payload.to_vec()
    };
    let len = u32::try_from(body.len()).map_err(|_| protocol("gRPC 请求超过帧长度上限"))?;
    let mut frame = Vec::with_capacity(body.len() + 5);
    frame.push(u8::from(compressed));
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend(body);
    Ok(frame)
}

/// 只接受完整的单帧；避免把网关协议变化误报成空列表。
pub fn decode_frame(frame: &[u8]) -> Result<Vec<u8>, DomainError> {
    if frame.len() < 5 {
        return Err(protocol("gRPC 帧头不足 5 字节"));
    }
    let length = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
    if length != frame.len() - 5 {
        return Err(protocol("gRPC 帧长度不匹配或存在多余帧"));
    }
    match frame[0] {
        0 => Ok(frame[5..].to_vec()),
        1 => {
            // 与 HTTP content-encoding 解压独立：这里仅处理帧内 gzip。
            let mut decoder = GzDecoder::new(&frame[5..]);
            let mut bytes = Vec::new();
            decoder
                .read_to_end(&mut bytes)
                .map_err(|_| protocol("gRPC 帧 gzip 损坏"))?;
            Ok(bytes)
        }
        _ => Err(protocol("gRPC 帧压缩标志非法")),
    }
}

fn header<'a>(response: &'a HttpBytesResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

// google.rpc.Status 只解基线实际消费的 code/message/details.Any.value。
type RpcStatus = (Option<i64>, String, Vec<Vec<u8>>);

fn status(bytes: &[u8]) -> Result<RpcStatus, DomainError> {
    let mut reader = Reader::new(bytes);
    let (mut code, mut message, mut details) = (Some(0), String::new(), Vec::new());
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (1, WireType::Varint) => code = Some(i64::from(reader.read_int32()?)),
            (2, WireType::LengthDelimited) => message = reader.read_string()?,
            (3, WireType::LengthDelimited) => {
                let mut any = Reader::new(reader.read_bytes()?);
                while !any.is_empty() {
                    let tag = any.read_tag()?;
                    if (tag.field, tag.wire) == (2, WireType::LengthDelimited) {
                        details.push(any.read_bytes()?.to_vec());
                    } else {
                        any.skip(tag.wire)?;
                    }
                }
            }
            _ => reader.skip(tag.wire)?,
        }
    }
    Ok((code, message, details))
}

/// 状态头失败优先于帧解析；内层业务码与 REST error_code 相同。
pub fn decode_response(response: HttpBytesResponse) -> Result<Vec<u8>, DomainError> {
    crate::http::check_status(response.status)?;
    let grpc_status = header(&response, "grpc-status").ok_or_else(|| {
        protocol("没有 Grpc-Status 响应头，网关可能改用 trailer，见 ADR-012 决策一")
    })?;
    if grpc_status == "0" {
        return decode_frame(&response.body);
    }
    let encoded = header(&response, "grpc-status-details-bin").unwrap_or("");
    let mut padded = encoded.to_owned();
    padded.extend(std::iter::repeat_n('=', (4 - encoded.len() % 4) % 4));
    if let Ok(bytes) = STANDARD.decode(padded) {
        if let Ok((_, _, details)) = status(&bytes) {
            let parsed: Result<Vec<_>, _> = details.iter().map(|bytes| status(bytes)).collect();
            if let Ok(parsed) = parsed {
                let message = parsed
                    .iter()
                    .map(|(_, msg, _)| msg.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(match parsed.first().and_then(|(code, _, _)| *code) {
                    Some(code) => DomainError::with_code(ErrorKind::Remote, message, code),
                    None => DomainError::new(ErrorKind::Remote, message),
                });
            }
        }
        return Err(DomainError::new(
            ErrorKind::Remote,
            String::from_utf8_lossy(&bytes),
        ));
    }
    Err(DomainError::new(
        ErrorKind::Remote,
        format!("gRPC 状态 {grpc_status}，错误详情编码无效"),
    ))
}

/// 凭据与 metadata 已由调用方构造；gRPC 不走 REST 的 csrf 注入点。
pub async fn unary<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    path: &str,
    payload: &[u8],
    retry_policy: RetryPolicy,
) -> Result<Vec<u8>, DomainError> {
    if ctx.csrf.is_some() {
        return Err(protocol("gRPC 上下文不得携带 csrf"));
    }
    let response = transport
        .post_bytes(HttpPost {
            url: format!("{}{}", ctx.base_url.trim_end_matches('/'), path),
            headers: ctx.all_headers(),
            body: PostBody::Bytes {
                content_type: "application/grpc".to_owned(),
                data: frame_request(payload)?,
            },
            timeout: ctx.timeout,
            retry_policy,
        })
        .await?;
    decode_response(response)
}
