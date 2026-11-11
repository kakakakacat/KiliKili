//! 黑名单列表的请求构造与解析（S15 / issue #66）。
//!
//! 对应基线 `BlackHttp.blackList`（`lib/http/black.dart`）。
//!
//! 这是个**只读却要 csrf** 的接口（和笔记列表一样，是服务端的要求）。
//! csrf 由 Dart 的账号体系按请求传入，Rust 不存凭据。

use bili_domain::error::DomainError;
use bili_domain::user_list::{BlackList, BlackListItem};
use serde::Deserialize;

use crate::http::{HttpTransport, RequestContext};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const BLACK_LIST: &str = "/x/relation/blacks";
}

/// 黑名单一页的请求。
///
/// `re_version=0` 与 `jsonp=jsonp` 是基线写死的固定值，照搬——服务端对
/// 这个老接口认这两个参数，去掉未必还能工作，而这不是本次迁移要验证的事。
#[must_use]
pub fn black_list_request(page: i64, page_size: i64, csrf: &str) -> PreparedRequest {
    PreparedRequest {
        path: path::BLACK_LIST,
        query: vec![
            ("pn".to_owned(), page.to_string()),
            ("ps".to_owned(), page_size.to_string()),
            ("re_version".to_owned(), "0".to_owned()),
            ("jsonp".to_owned(), "jsonp".to_owned()),
            ("csrf".to_owned(), csrf.to_owned()),
        ],
    }
}

/// 取一页黑名单。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_black_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page: i64,
    page_size: i64,
    csrf: &str,
) -> Result<BlackList, DomainError> {
    let req = black_list_request(page, page_size, csrf);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_black_list(&body)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BlackListDto {
    list: Option<Vec<BlackListItemDto>>,
    total: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BlackListItemDto {
    mid: Option<i64>,
    mtime: Option<i64>,
    uname: Option<String>,
    face: Option<String>,
}

/// 解析黑名单。
///
/// # Errors
/// 信封失败沿用统一分类。`list` 缺失按空列表处理：黑名单为空与这一页
/// 没有数据在基线里都是 `list == null`，不是错误。
pub fn parse_black_list(body: &str) -> Result<BlackList, DomainError> {
    let dto: BlackListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(BlackList {
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(|i| BlackListItem {
                mid: i.mid,
                mtime: i.mtime,
                uname: i.uname,
                face: i.face,
            })
            .collect(),
        total: dto.total,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn request_carries_the_baseline_fixed_params() {
        let q = black_list_request(2, 50, "token").query_string();
        assert!(q.contains("pn=2") && q.contains("ps=50"), "{q}");
        assert!(
            q.contains("re_version=0") && q.contains("jsonp=jsonp"),
            "{q}"
        );
        assert!(q.contains("csrf=token"), "{q}");
    }

    #[test]
    fn empty_list_is_not_an_error() {
        let list = parse_black_list(r#"{"code":0,"message":"0","data":{"total":0}}"#)
            .expect("空黑名单不是错误");
        assert!(list.list.is_empty());
        assert_eq!(list.total, Some(0));
    }
}
