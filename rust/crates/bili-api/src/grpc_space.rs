//! 空间内的 gRPC 投稿搜索（S6 / issue #56）。
//!
//! 对应基线 `lib/grpc/space.dart` 的 `SpaceGrpc.searchArchive`。
//!
//! ## 为什么只生成请求、手写解析响应
//!
//! 同 [`crate::grpc_danmaku`]：`SearchArchiveReply` 的条目里嵌着
//! `bilibili.app.archive.v1.Arc`，那是个上百字段的大消息树，而界面只读
//! 其中十来个。把整棵树生成进来，等于让 `bili-grpc` 背上一堆没人用的
//! 类型，还要跟着上游一起漂。请求消息只有四个标量，走 prost；响应用
//! [`crate::protobuf::Reader`] 按字段号挑着读。
//!
//! 字段号的出处是基线 `lib/grpc/bilibili/app/{interfaces,archive}/v1.pb.dart`
//! 的 `BuilderInfo`，不是猜的。

use bili_domain::error::DomainError;
use bili_domain::space::{SpaceSearchArchiveItem, SpaceSearchArchivePage};
use bili_grpc::bilibili::app::interfaces::v1::SearchArchiveReq;
use prost::Message;

use crate::http::{HttpTransport, RequestContext, RetryPolicy};
use crate::protobuf::{Reader, WireType};

/// gRPC 方法路径。照抄基线 `lib/grpc/url.dart`。
///
/// **注意是 `interface` 而不是 `interfaces`**：消息的 proto 包名是
/// `bilibili.app.interfaces.v1`（见 `lib/grpc/bilibili/app/interfaces/`），
/// 而服务挂在少一个 s 的那个名字下。两者不一致是服务端的事实，不是笔误；
/// 按着 proto 包名拼会得到 404。
pub const SEARCH_ARCHIVE: &str = "/bilibili.app.interface.v1.Space/SearchArchive";

/// 投稿搜索的请求体。
///
/// `player_args` 基线不设，这里也不设——它是取流相关的参数，与搜索无关。
#[must_use]
pub fn search_archive_request(mid: i64, keyword: &str, page: i64, page_size: i64) -> Vec<u8> {
    SearchArchiveReq {
        keyword: Some(keyword.to_owned()),
        mid: Some(mid),
        pn: Some(page),
        ps: Some(page_size),
        player_args: None,
    }
    .encode_to_vec()
}

/// `bilibili.app.archive.v1.Dimension`：宽、高、旋转。
#[derive(Default)]
struct Dimension {
    width: i64,
    height: i64,
    rotate: i64,
}

impl Dimension {
    /// 竖屏判定。照抄基线 `DimensionExt.isVertical`：旋转过的稿件宽高要
    /// 反过来看。
    const fn is_vertical(&self) -> bool {
        if self.rotate == 1 {
            self.width > self.height
        } else {
            self.height > self.width
        }
    }
}

#[derive(Default)]
struct InnerArc {
    aid: Option<i64>,
    title: Option<String>,
    pic: Option<String>,
    pubdate: Option<i64>,
    duration: Option<i64>,
    first_cid: Option<i64>,
    redirect_url: Option<String>,
    view: Option<i64>,
    danmaku: Option<i64>,
    author_name: Option<String>,
    dimension: Dimension,
}

/// `bilibili.app.archive.v1.Stat` 的 `view`(2) 与 `danmaku`(3)。
fn parse_stat(bytes: &[u8]) -> Result<(Option<i64>, Option<i64>), DomainError> {
    let mut reader = Reader::new(bytes);
    let (mut view, mut danmaku) = (None, None);
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (2, WireType::Varint) => view = Some(i64::from(reader.read_int32()?)),
            (3, WireType::Varint) => danmaku = Some(i64::from(reader.read_int32()?)),
            _ => reader.skip(tag.wire)?,
        }
    }
    Ok((view, danmaku))
}

/// `bilibili.app.archive.v1.Author` 的 `name`(2)。
fn parse_author_name(bytes: &[u8]) -> Result<Option<String>, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut name = None;
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (2, WireType::LengthDelimited) => name = Some(reader.read_string()?),
            _ => reader.skip(tag.wire)?,
        }
    }
    Ok(name)
}

fn parse_dimension(bytes: &[u8]) -> Result<Dimension, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut dimension = Dimension::default();
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (1, WireType::Varint) => dimension.width = reader.read_int64()?,
            (2, WireType::Varint) => dimension.height = reader.read_int64()?,
            (3, WireType::Varint) => dimension.rotate = reader.read_int64()?,
            _ => reader.skip(tag.wire)?,
        }
    }
    Ok(dimension)
}

/// `bilibili.app.archive.v1.Arc`。只读界面用得上的那些字段号。
fn parse_inner_arc(bytes: &[u8]) -> Result<InnerArc, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut arc = InnerArc::default();
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (1, WireType::Varint) => arc.aid = Some(reader.read_int64()?),
            (6, WireType::LengthDelimited) => arc.pic = Some(reader.read_string()?),
            (7, WireType::LengthDelimited) => arc.title = Some(reader.read_string()?),
            (8, WireType::Varint) => arc.pubdate = Some(reader.read_int64()?),
            (16, WireType::Varint) => arc.duration = Some(reader.read_int64()?),
            (19, WireType::LengthDelimited) => arc.redirect_url = Some(reader.read_string()?),
            (22, WireType::LengthDelimited) => {
                arc.author_name = parse_author_name(reader.read_bytes()?)?;
            }
            (23, WireType::LengthDelimited) => {
                let (view, danmaku) = parse_stat(reader.read_bytes()?)?;
                arc.view = view;
                arc.danmaku = danmaku;
            }
            (26, WireType::Varint) => arc.first_cid = Some(reader.read_int64()?),
            (27, WireType::LengthDelimited) => {
                arc.dimension = parse_dimension(reader.read_bytes()?)?;
            }
            _ => reader.skip(tag.wire)?,
        }
    }
    Ok(arc)
}

/// `bilibili.app.interfaces.v1.Arc`：外层只用 `archive`(1)、`uri`(2)、
/// `is_pugv`(7)。
fn parse_arc(bytes: &[u8]) -> Result<SpaceSearchArchiveItem, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut inner = InnerArc::default();
    let mut uri = None;
    let mut is_pugv = false;
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (1, WireType::LengthDelimited) => inner = parse_inner_arc(reader.read_bytes()?)?,
            (2, WireType::LengthDelimited) => uri = Some(reader.read_string()?),
            (7, WireType::Varint) => is_pugv = reader.read_varint()? != 0,
            _ => reader.skip(tag.wire)?,
        }
    }
    Ok(SpaceSearchArchiveItem {
        aid: inner.aid,
        title_spans: inner
            .title
            .as_deref()
            .map(crate::search_text::parse_title_spans)
            .unwrap_or_default(),
        cover: inner.pic,
        pubdate: inner.pubdate,
        duration: inner.duration,
        first_cid: inner.first_cid,
        view: inner.view,
        danmaku: inner.danmaku,
        author_name: inner.author_name,
        is_vertical: inner.dimension.is_vertical(),
        // 空串按没有：基线判的是 `hasRedirectUrl()`，而 proto3 的空串与
        // 未设置在 wire 上就是一回事。
        redirect_url: inner.redirect_url.filter(|s| !s.is_empty()),
        uri: uri.filter(|s| !s.is_empty()),
        is_pugv,
    })
}

/// 解析一页 gRPC 投稿搜索（`SearchArchiveReply`）。
///
/// # Errors
/// 字段类型与协议不符时按协议错误返回。
pub fn parse_search_archive(bytes: &[u8]) -> Result<SpaceSearchArchivePage, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut page = SpaceSearchArchivePage::default();
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (1, WireType::LengthDelimited) => page.items.push(parse_arc(reader.read_bytes()?)?),
            (2, WireType::Varint) => page.total = Some(reader.read_int64()?),
            _ => reader.skip(tag.wire)?,
        }
    }
    Ok(page)
}

/// 取一页 gRPC 投稿搜索。
///
/// # Errors
/// 传输、网关状态、解析三类失败。
pub async fn fetch_search_archive<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    mid: i64,
    keyword: &str,
    page: i64,
    page_size: i64,
) -> Result<SpaceSearchArchivePage, DomainError> {
    let bytes = crate::grpc::unary(
        transport,
        context,
        SEARCH_ARCHIVE,
        &search_archive_request(mid, keyword, page, page_size),
        RetryPolicy::Read,
    )
    .await?;
    parse_search_archive(&bytes)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{parse_search_archive, search_archive_request};

    /// 只设基线设过的那四个字段。
    #[test]
    fn the_request_carries_only_the_four_baseline_fields() {
        let bytes = search_archive_request(100, "关键词", 2, 20);
        let decoded =
            <bili_grpc::bilibili::app::interfaces::v1::SearchArchiveReq as prost::Message>::decode(
                bytes.as_slice(),
            )
            .expect("应能解回来");
        assert_eq!(decoded.mid, Some(100));
        assert_eq!(decoded.keyword.as_deref(), Some("关键词"));
        assert_eq!(decoded.pn, Some(2));
        assert_eq!(decoded.ps, Some(20));
        assert!(decoded.player_args.is_none(), "基线不设 player_args");
    }

    /// 未知字段要跳过，不能把整页判成协议错误——上游随时会加字段。
    #[test]
    fn unknown_fields_are_skipped() {
        // field 9 (varint) 是个这里不认识的字段。
        let bytes = [0x48, 0x01, 0x10, 0x05];
        let page = parse_search_archive(&bytes).expect("应解析成功");
        assert_eq!(page.total, Some(5));
        assert!(page.items.is_empty());
    }
}
