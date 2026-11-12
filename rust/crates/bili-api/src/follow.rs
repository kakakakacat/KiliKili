//! 关注分组的写操作（S15 / issue #66，写语义见 ADR-013）。
//!
//! 对应基线 `FollowHttp.sortFollowTag`（`lib/http/follow.dart`）。同文件里的
//! `followings` 是只读的分页列表，与 `FanHttp.fans` 共用 `FollowData` 模型，
//! **两个生产者得一起迁**，所以留在本片之外——按 ADR-011 决策六，共享模型
//! 要等它的所有生产者都迁完才删得掉。
//!
//! ## 这条接口特别在哪
//!
//! 它是目前唯一一条**参数同时分布在查询串与表单里**的写操作：
//! `x-bili-device-req-json` 在 URL 上，`tagids` 在体里。`PreparedWrite` 的
//! `query` 字段就是为它加的——「写操作的参数都在体里」只是惯例，不是协议
//! 保证。

use bili_domain::error::DomainError;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::PreparedWrite;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const SORT_FOLLOW_TAG: &str = "/x/relation/tags/update_sort";
}

/// 基线原样发送的设备标识串。
///
/// 不是可省的装饰：缺了它服务端会按另一套风控规则处理。内容是固定常量，
/// 与基线 `lib/http/follow.dart` 一字不差。
const DEVICE_REQ_JSON: &str = r#"{"platform":"web","device":"pc","spmid":"333.1387"}"#;

/// 关注分组排序的写请求。
///
/// `tagids` 是逗号分隔的分组 id，顺序即目标顺序。
///
/// 定级 [`Idempotency::Idempotent`]：整串顺序是**置为**语义，重发落到同一
/// 个顺序。
#[must_use]
pub fn sort_follow_tag_request(tagids: &str) -> PreparedWrite {
    PreparedWrite {
        path: path::SORT_FOLLOW_TAG,
        query: vec![(
            "x-bili-device-req-json".to_owned(),
            DEVICE_REQ_JSON.to_owned(),
        )],
        form: vec![("tagids".to_owned(), tagids.to_owned())],
        idempotency: Idempotency::Idempotent,
    }
}

/// 提交关注分组的新顺序。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn sort_follow_tag<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    tagids: &str,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &sort_follow_tag_request(tagids)).await?;
    crate::envelope::check_envelope(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{DEVICE_REQ_JSON, path, sort_follow_tag_request};
    use crate::http::Idempotency;

    #[test]
    fn request_shape_is_pinned() {
        let req = sort_follow_tag_request("3,1,2");
        assert_eq!(req.path, path::SORT_FOLLOW_TAG);
        assert_eq!(req.form_string(), "tagids=3,1,2");
        assert_eq!(
            req.query,
            vec![(
                "x-bili-device-req-json".to_owned(),
                DEVICE_REQ_JSON.to_owned()
            )],
            "设备标识串在查询串上，不在表单里——这条是唯一的例外，别顺手挪进体里"
        );
        assert_eq!(
            req.idempotency,
            Idempotency::Idempotent,
            "整串顺序是置为语义"
        );
    }
}
