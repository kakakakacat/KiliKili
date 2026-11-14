//! 动态 gRPC 接口。当前先迁首页红点，其他接口随 #55 后续路径接入。

use bili_domain::{
    dynamic::UnreadCount,
    error::{DomainError, ErrorKind},
};
use prost::Message;

use crate::{
    grpc,
    http::{HttpTransport, RequestContext, RetryPolicy},
};

/// 与 Dart 的 `DynRedReq(tabOffset: [TabOffset(tab: 1)])` 字节一致。
/// 这里只需四个字段，保留局部消息定义，避免引入整棵动态协议树。
#[derive(Clone, PartialEq, Message)]
struct TabOffset {
    #[prost(int32, optional, tag = "1")]
    tab: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct DynRedReq {
    #[prost(message, repeated, tag = "1")]
    tab_offset: Vec<TabOffset>,
}

#[derive(Clone, PartialEq, Message)]
struct DynRedItem {
    #[prost(int64, optional, tag = "1")]
    count: Option<i64>,
}

#[derive(Clone, PartialEq, Message)]
struct DynRedReply {
    #[prost(message, optional, tag = "2")]
    dyn_red_item: Option<DynRedItem>,
}

pub const DYN_RED_PATH: &str = "/bilibili.app.dynamic.v1.Dynamic/DynRed";

#[must_use]
pub fn dyn_red_request() -> Vec<u8> {
    DynRedReq {
        tab_offset: vec![TabOffset { tab: Some(1) }],
    }
    .encode_to_vec()
}

/// protobuf.dart 对缺失的嵌套消息和 int64 标量均返回零值。
/// 解码失败才是协议错误，不把它误当成没有新动态。
pub fn parse_dyn_red(bytes: &[u8]) -> Result<UnreadCount, DomainError> {
    let reply = DynRedReply::decode(bytes)
        .map_err(|_| DomainError::new(ErrorKind::Protocol, "动态红点响应 protobuf 解码失败"))?;
    Ok(UnreadCount {
        count: reply.dyn_red_item.and_then(|item| item.count).unwrap_or(0),
    })
}

pub async fn fetch_dyn_red<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
) -> Result<UnreadCount, DomainError> {
    let bytes = grpc::unary(
        transport,
        context,
        DYN_RED_PATH,
        &dyn_red_request(),
        RetryPolicy::Read,
    )
    .await?;
    parse_dyn_red(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_matches_dart_protobuf() {
        assert_eq!(dyn_red_request(), [0x0a, 0x02, 0x08, 0x01]);
    }

    #[test]
    fn count_and_missing_fields_match_protobuf_defaults() {
        assert!(matches!(
            parse_dyn_red(&[0x12, 0x02, 0x08, 0x2a]),
            Ok(UnreadCount { count: 42 })
        ));
        assert!(matches!(parse_dyn_red(&[]), Ok(UnreadCount { count: 0 })));
        assert!(matches!(
            parse_dyn_red(&[0x12, 0x00]),
            Ok(UnreadCount { count: 0 })
        ));
        assert!(parse_dyn_red(&[0x12, 0x02, 0x08]).is_err());
    }
}
