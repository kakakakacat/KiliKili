//! 播放页附属只读接口的请求构造与解析（S3 第二批，issue #53）。
//!
//! 对应基线 `VideoHttp` 的 `onlineTotal`、`aiConclusion`、`playInfo`、
//! `videoshot`、`getVideoNoteList`（`lib/http/video.dart`）。
//!
//! ## 三处与别的接口不同的地方
//!
//! 1. **AI 总结有两层 code**。外层信封 `code == 0` 只表示请求被受理，
//!    `data.code` 才说明这次总结的状态（1 = 正在处理中）。基线的调用点
//!    正是靠内层码区分「稍后再试」与「不支持」，所以内层码必须原样带回
//!    `error_code`，见 [`parse_ai_conclusion`]。
//! 2. **缩略图在没有数据时也要算失败**。基线拿到空 `index` 时返回
//!    `Error(null)`，播放器据此不显示预览；若按成功返回空列表，进度条
//!    预览会去索引空数组。见 [`parse_video_shot`]。
//! 3. **笔记列表要 csrf**。它是只读接口却要带 csrf（服务端的要求），
//!    由 Dart 的账号体系按请求传入，Rust 不存。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::video_page::{
    AiConclusion, AiOutline, AiOutlinePart, HistoryNode, Interaction, NoteAuthor, NoteItem,
    NoteList, PlayInfo, PlaySubtitle, VideoShot, ViewPoint, to_https,
};
use bili_protocol::wbi;
use serde::Deserialize;

use crate::http::{HttpTransport, RequestContext};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const ONLINE_TOTAL: &str = "/x/player/online/total";
    pub const AI_CONCLUSION: &str = "/x/web-interface/view/conclusion/get";
    pub const PLAY_INFO: &str = "/x/player/wbi/v2";
    pub const VIDEO_SHOT: &str = "/x/player/videoshot";
    pub const NOTE_LIST: &str = "/x/note/publish/list/archive";
}

/// 一个视频的标识。`aid` 与 `bvid` 至少要有一个，服务端两种都认。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoRef {
    pub aid: Option<i64>,
    pub bvid: Option<String>,
}

impl VideoRef {
    fn push_into(&self, query: &mut Vec<(String, String)>) {
        if let Some(aid) = self.aid {
            query.push(("aid".to_owned(), aid.to_string()));
        }
        if let Some(bvid) = &self.bvid {
            query.push(("bvid".to_owned(), bvid.clone()));
        }
    }

    fn is_empty(&self) -> bool {
        self.aid.is_none() && self.bvid.as_ref().is_none_or(String::is_empty)
    }
}

/// 同时在看人数的请求。不签名。
#[must_use]
pub fn online_total_request(video: &VideoRef, cid: i64) -> PreparedRequest {
    let mut query = Vec::with_capacity(3);
    video.push_into(&mut query);
    query.push(("cid".to_owned(), cid.to_string()));
    PreparedRequest {
        path: path::ONLINE_TOTAL,
        query,
    }
}

/// AI 总结的请求。WBI 签名。
#[must_use]
pub fn ai_conclusion_request(
    bvid: &str,
    cid: i64,
    up_mid: Option<i64>,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("bvid".to_owned(), bvid.to_owned()),
        ("cid".to_owned(), cid.to_string()),
    ];
    if let Some(mid) = up_mid {
        query.push(("up_mid".to_owned(), mid.to_string()));
    }
    signed(path::AI_CONCLUSION, query, mixin_key, wts)
}

/// 播放信息要问的是哪一个分P。
///
/// 单独成结构而不是摊平成四个参数：否则 `fetch_play_info` 连上下文与签名
/// 就有八个入参，调用点很容易把 `season_id` 与 `ep_id` 传反——两者都是
/// `Option<i64>`，编译器帮不上忙。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayInfoTarget {
    pub video: VideoRef,
    pub cid: i64,
    pub season_id: Option<i64>,
    pub ep_id: Option<i64>,
}

/// 播放信息的请求。WBI 签名。
#[must_use]
pub fn play_info_request(target: &PlayInfoTarget, mixin_key: &str, wts: i64) -> PreparedRequest {
    let mut query = Vec::with_capacity(6);
    target.video.push_into(&mut query);
    query.push(("cid".to_owned(), target.cid.to_string()));
    if let Some(season_id) = target.season_id {
        query.push(("season_id".to_owned(), season_id.to_string()));
    }
    if let Some(ep_id) = target.ep_id {
        query.push(("ep_id".to_owned(), ep_id.to_string()));
    }
    signed(path::PLAY_INFO, query, mixin_key, wts)
}

/// 预览缩略图的请求。不签名。
///
/// `index=1` 是基线的固定值：要服务端连时间索引一起给，不然只有图。
#[must_use]
pub fn video_shot_request(bvid: &str, cid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::VIDEO_SHOT,
        query: vec![
            ("bvid".to_owned(), bvid.to_owned()),
            ("cid".to_owned(), cid.to_string()),
            ("index".to_owned(), "1".to_owned()),
        ],
    }
}

/// 笔记列表的请求。不签名，但要 csrf。
///
/// `oid_type=0` 表示 oid 是稿件 aid；`ps=10` 是基线的固定页长。
#[must_use]
pub fn note_list_request(
    csrf: &str,
    oid: i64,
    page: i64,
    uper_mid: Option<i64>,
) -> PreparedRequest {
    let mut query = vec![
        ("csrf".to_owned(), csrf.to_owned()),
        ("oid".to_owned(), oid.to_string()),
        ("oid_type".to_owned(), "0".to_owned()),
        ("pn".to_owned(), page.to_string()),
        ("ps".to_owned(), "10".to_owned()),
    ];
    if let Some(mid) = uper_mid {
        query.push(("uper_mid".to_owned(), mid.to_string()));
    }
    PreparedRequest {
        path: path::NOTE_LIST,
        query,
    }
}

/// 补上 `wts` 与 `w_rid`。与 [`crate::video_feed`] 同一套做法。
fn signed(
    path: &'static str,
    mut query: Vec<(String, String)>,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    query.push(("wts".to_owned(), wts.to_string()));
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest { path, query }
}

/// 取同时在看人数。返回的是**已经排好版的字符串**（如 `"1024"` 或
/// `"1.2万"`），服务端直接给成展示形态，不是数字。
///
/// # Errors
/// 传输、状态码、信封三类失败见各自的文档。
pub async fn fetch_online_total<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    video: &VideoRef,
    cid: i64,
) -> Result<String, DomainError> {
    reject_empty_ref(video)?;
    let body = crate::client::send(transport, ctx, &online_total_request(video, cid)).await?;
    parse_online_total(&body)
}

/// 取 AI 总结。
///
/// # Errors
/// 除三类常规失败外，内层 `data.code` 非 0 时也是错误，且 `error_code`
/// 带的是**内层**码（1 = AI 处理中）。
pub async fn fetch_ai_conclusion<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    bvid: &str,
    cid: i64,
    up_mid: Option<i64>,
    mixin_key: &str,
    wts: i64,
) -> Result<AiConclusion, DomainError> {
    let req = ai_conclusion_request(bvid, cid, up_mid, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_ai_conclusion(&body)
}

/// 取播放信息。
///
/// # Errors
/// 见 [`fetch_online_total`]。
pub async fn fetch_play_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    target: &PlayInfoTarget,
    mixin_key: &str,
    wts: i64,
) -> Result<PlayInfo, DomainError> {
    reject_empty_ref(&target.video)?;
    let req = play_info_request(target, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_play_info(&body)
}

/// 取进度条预览缩略图。
///
/// # Errors
/// 没有可用缩略图（`index` 为空）时也算失败，见 [`parse_video_shot`]。
pub async fn fetch_video_shot<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    bvid: &str,
    cid: i64,
) -> Result<VideoShot, DomainError> {
    let body = crate::client::send(transport, ctx, &video_shot_request(bvid, cid)).await?;
    parse_video_shot(&body)
}

/// 取笔记列表的一页。
///
/// # Errors
/// 见 [`fetch_online_total`]。
pub async fn fetch_note_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    csrf: &str,
    oid: i64,
    page: i64,
    uper_mid: Option<i64>,
) -> Result<NoteList, DomainError> {
    let req = note_list_request(csrf, oid, page, uper_mid);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_note_list(&body)
}

fn reject_empty_ref(video: &VideoRef) -> Result<(), DomainError> {
    if video.is_empty() {
        return Err(DomainError::new(
            ErrorKind::Protocol,
            "aid 与 bvid 都为空，请求不会有有效结果",
        ));
    }
    Ok(())
}

/// 解析同时在看人数。
///
/// # Errors
/// 信封失败沿用统一分类；缺 `total` 时为 `Protocol`。
pub fn parse_online_total(body: &str) -> Result<String, DomainError> {
    #[derive(Debug, Default, Deserialize)]
    #[serde(default)]
    struct Dto {
        total: Option<String>,
    }
    let dto: Dto = crate::envelope::unwrap_envelope(body)?;
    dto.total
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "响应里没有 total"))
}

/// 解析 AI 总结。
///
/// # Errors
/// 内层 `data.code` 非 0 时返回 `Remote`，并把**内层**码放进
/// `DomainError::code`——基线的调用点用它区分「AI 处理中」(1) 与其它。
pub fn parse_ai_conclusion(body: &str) -> Result<AiConclusion, DomainError> {
    #[derive(Debug, Default, Deserialize)]
    #[serde(default)]
    struct Dto {
        code: Option<i64>,
        model_result: Option<ModelResultDto>,
    }

    let dto: Dto = crate::envelope::unwrap_envelope(body)?;
    // 缺 code 时按 0 处理：基线 `res.data['data']?['code']` 为 null 时
    // 走的是 else 分支（当成失败），但那条路径只会出现在结构已变的响应上，
    // 这里让它落到下面的「没有 model_result」判断，错误更具体。
    let inner = dto.code.unwrap_or(0);
    if inner != 0 {
        return Err(DomainError::with_code(
            ErrorKind::Remote,
            format!("AI 总结不可用（code {inner}）"),
            inner,
        ));
    }

    let result = dto
        .model_result
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "响应里没有 model_result"))?;
    Ok(AiConclusion {
        summary: result.summary,
        outline: result.outline.map(|list| {
            list.into_iter()
                .map(|o| AiOutline {
                    title: o.title,
                    part_outline: o.part_outline.map(|parts| {
                        parts
                            .into_iter()
                            .map(|p| AiOutlinePart {
                                timestamp: p.timestamp,
                                content: p.content,
                            })
                            .collect()
                    }),
                })
                .collect()
        }),
    })
}

/// 解析播放信息。
///
/// # Errors
/// 信封失败沿用统一分类；结构不符为 `Protocol`。
pub fn parse_play_info(body: &str) -> Result<PlayInfo, DomainError> {
    let dto: PlayInfoDto = crate::envelope::unwrap_envelope(body)?;
    Ok(PlayInfo {
        last_play_cid: dto.last_play_cid,
        subtitles: dto.subtitle.and_then(|s| {
            s.subtitles.map(|list| {
                list.into_iter()
                    .map(|s| PlaySubtitle {
                        id: s.id,
                        lan: s.lan,
                        lan_doc: s.lan_doc,
                        subtitle_url: s.subtitle_url,
                        kind: s.kind,
                    })
                    .collect()
            })
        }),
        view_points: dto.view_points.map(|list| {
            list.into_iter()
                .map(|v| ViewPoint {
                    kind: v.kind,
                    from: v.from,
                    to: v.to,
                    content: v.content,
                    img_url: v.img_url,
                })
                .collect()
        }),
        interaction: dto.interaction.map(|i| Interaction {
            graph_version: i.graph_version,
            history_node: i.history_node.map(|n| HistoryNode {
                node_id: n.node_id,
                title: n.title,
                cid: n.cid,
            }),
        }),
    })
}

/// 解析预览缩略图。
///
/// # Errors
/// 除结构不符外，**`index` 为空时也是错误**（`Remote`）：基线在这种情况下
/// 返回 `Error(null)`，播放器据此不显示预览。按成功返回空列表的话，进度条
/// 预览会去索引空数组。
pub fn parse_video_shot(body: &str) -> Result<VideoShot, DomainError> {
    let dto: VideoShotDto = crate::envelope::unwrap_envelope(body)?;

    let index = dto.index.unwrap_or_default();
    if index.is_empty() {
        return Err(DomainError::new(ErrorKind::Remote, "这个视频没有预览图"));
    }

    Ok(VideoShot {
        pvdata: dto.pvdata,
        img_x_len: dto.img_x_len.unwrap_or_default(),
        img_y_len: dto.img_y_len.unwrap_or_default(),
        img_x_size: dto.img_x_size.unwrap_or_default(),
        img_y_size: dto.img_y_size.unwrap_or_default(),
        // 基线对每张图都调 http2https：部分机型不允许明文 HTTP，漏一张的
        // 表现是「个别视频的预览图是空白」。
        images: dto
            .image
            .unwrap_or_default()
            .iter()
            .map(|u| to_https(u))
            .collect(),
        index,
    })
}

/// 解析笔记列表。
///
/// # Errors
/// 信封失败沿用统一分类；结构不符为 `Protocol`。
pub fn parse_note_list(body: &str) -> Result<NoteList, DomainError> {
    let dto: NoteListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(NoteList {
        total: dto.page.and_then(|p| p.total),
        list: dto.list.map(|items| {
            items
                .into_iter()
                .map(|i| NoteItem {
                    cvid: i.cvid,
                    summary: i.summary,
                    pubtime: i.pubtime,
                    author: i.author.map(AuthorDto::into_domain),
                })
                .collect()
        }),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ModelResultDto {
    summary: Option<String>,
    outline: Option<Vec<OutlineDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OutlineDto {
    title: Option<String>,
    part_outline: Option<Vec<PartOutlineDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PartOutlineDto {
    timestamp: Option<i64>,
    content: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlayInfoDto {
    last_play_cid: Option<i64>,
    subtitle: Option<SubtitleInfoDto>,
    view_points: Option<Vec<ViewPointDto>>,
    interaction: Option<InteractionDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SubtitleInfoDto {
    subtitles: Option<Vec<SubtitleDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SubtitleDto {
    id: Option<i64>,
    lan: Option<String>,
    lan_doc: Option<String>,
    subtitle_url: Option<String>,
    #[serde(rename = "type")]
    kind: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewPointDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
    from: Option<i64>,
    to: Option<i64>,
    content: Option<String>,
    // 响应里是小驼峰，和同一个对象里的其它字段不一致。
    #[serde(rename = "imgUrl")]
    img_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InteractionDto {
    graph_version: Option<i64>,
    history_node: Option<HistoryNodeDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HistoryNodeDto {
    node_id: Option<i64>,
    title: Option<String>,
    cid: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VideoShotDto {
    pvdata: Option<String>,
    img_x_len: Option<i64>,
    img_y_len: Option<i64>,
    img_x_size: Option<f64>,
    img_y_size: Option<f64>,
    image: Option<Vec<String>>,
    index: Option<Vec<i64>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NoteListDto {
    list: Option<Vec<NoteItemDto>>,
    page: Option<PageDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PageDto {
    total: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NoteItemDto {
    cvid: Option<i64>,
    summary: Option<String>,
    pubtime: Option<String>,
    author: Option<AuthorDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AuthorDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
    level: Option<i64>,
    is_senior_member: Option<i64>,
    vip_info: Option<VipDto>,
    pendant: Option<PendantDto>,
    official: Option<OfficialDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VipDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
    status: Option<i64>,
    label: Option<LabelDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LabelDto {
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PendantDto {
    image: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OfficialDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
    desc: Option<String>,
}

impl AuthorDto {
    fn into_domain(self) -> NoteAuthor {
        let vip = self.vip_info.unwrap_or_default();
        let official = self.official.unwrap_or_default();
        NoteAuthor {
            mid: self.mid,
            name: self.name,
            face: self.face,
            level: self.level,
            is_senior_member: self.is_senior_member,
            vip_type: vip.kind,
            // 基线 `Vip.fromJson` 缺 status 时填 0，不是 null——会员高亮的
            // 判断是 `status > 0`，null 会让那处判断少一个分支。
            vip_status: Some(vip.status.unwrap_or(0)),
            vip_label_text: vip.label.and_then(|l| l.text),
            pendant_image: self.pendant.and_then(|p| p.image),
            official_type: official.kind,
            official_desc: official.desc,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        VideoRef, ai_conclusion_request, note_list_request, online_total_request, parse_note_list,
        parse_online_total, parse_video_shot, play_info_request, video_shot_request,
    };
    use bili_domain::error::ErrorKind;

    const KEY: &str = "0123456789abcdef0123456789abcdef";

    fn envelope(data: &str) -> String {
        format!(r#"{{"code":0,"message":"0","ttl":1,"data":{data}}}"#)
    }

    fn keys(req: &super::PreparedRequest) -> Vec<&str> {
        req.query.iter().map(|(k, _)| k.as_str()).collect()
    }

    #[test]
    fn online_total_sends_only_the_identifiers_it_was_given() {
        let req = online_total_request(
            &VideoRef {
                bvid: Some("BV1Fixture01".to_owned()),
                ..VideoRef::default()
            },
            1001,
        );
        // 基线把 aid/bvid 一起塞进 map，为空的那个由 Dio 丢掉；这里直接不发。
        assert_eq!(keys(&req), vec!["bvid", "cid"]);
    }

    #[test]
    fn ai_conclusion_omits_up_mid_when_absent() {
        let with = ai_conclusion_request("BV1Fixture01", 1, Some(20001), KEY, 1);
        assert!(keys(&with).contains(&"up_mid"));

        let without = ai_conclusion_request("BV1Fixture01", 1, None, KEY, 1);
        assert!(!keys(&without).contains(&"up_mid"));
    }

    #[test]
    fn signed_requests_sort_before_signing_and_append_w_rid() {
        let req = play_info_request(
            &super::PlayInfoTarget {
                video: VideoRef {
                    aid: Some(80433022),
                    ..VideoRef::default()
                },
                cid: 1001,
                season_id: Some(5),
                ep_id: None,
            },
            KEY,
            1_700_000_000,
        );
        assert_eq!(keys(&req), vec!["aid", "cid", "season_id", "wts", "w_rid"]);
        let (_, w_rid) = req.query.last().expect("有 w_rid");
        assert_eq!(w_rid.len(), 32);
    }

    #[test]
    fn video_shot_pins_the_baseline_index_flag() {
        let req = video_shot_request("BV1Fixture01", 1001);
        let index = req
            .query
            .iter()
            .find(|(k, _)| k == "index")
            .expect("有 index");
        // index=1 要服务端把时间索引一起给；去掉它就只有图没有时间点。
        assert_eq!(index.1, "1");
    }

    #[test]
    fn note_list_pins_the_baseline_page_size_and_oid_type() {
        let req = note_list_request("csrf-token", 80433022, 2, None);
        let get = |k: &str| {
            req.query
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
                .expect("字段存在")
        };
        assert_eq!(get("ps"), "10");
        assert_eq!(get("oid_type"), "0");
        assert_eq!(get("pn"), "2");
    }

    #[test]
    fn an_empty_shot_index_is_an_error_not_an_empty_list() {
        // 基线在这里返回 Error(null)，播放器据此不显示预览。返回成功的空
        // 列表会让进度条预览去索引空数组。
        let body = envelope(r#"{"image":[],"index":[],"img_x_len":10,"img_y_len":10}"#);
        let err = parse_video_shot(&body).expect_err("空索引应判为失败");
        assert_eq!(err.kind(), ErrorKind::Remote);
    }

    #[test]
    fn protocol_relative_shot_urls_become_https() {
        let body = envelope(r#"{"image":["//i0.example.invalid/a.jpg"],"index":[0,5]}"#);
        let shot = parse_video_shot(&body).expect("可解析");
        assert_eq!(shot.images, vec!["https://i0.example.invalid/a.jpg"]);
    }

    #[test]
    fn online_total_is_a_display_string_not_a_number() {
        // 服务端给的是排好版的字符串（"1.2万"），当成数字解析会整条失败。
        let body = envelope(r#"{"total":"1.2万","count":"8"}"#);
        assert_eq!(parse_online_total(&body).expect("可解析"), "1.2万");
    }

    #[test]
    fn a_missing_total_is_a_protocol_error() {
        let err = parse_online_total(&envelope(r#"{"count":"8"}"#)).expect_err("缺 total");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn a_note_author_without_vip_info_still_reports_status_zero() {
        // 基线 Vip.fromJson 缺 status 时填 0；会员高亮判断的是 status > 0，
        // 收成 null 会让那处判断少一个分支。
        let body = envelope(r#"{"list":[{"cvid":1,"author":{"mid":2}}],"page":{"total":1}}"#);
        let list = parse_note_list(&body).expect("可解析");
        let author = list.list.expect("有 list")[0]
            .author
            .clone()
            .expect("有 author");
        assert_eq!(author.vip_status, Some(0));
        assert_eq!(author.vip_type, None);
    }

    #[test]
    fn a_note_page_without_total_is_not_an_error() {
        let body = envelope(r#"{"list":[]}"#);
        assert_eq!(parse_note_list(&body).expect("可解析").total, None);
    }
}
