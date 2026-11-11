//! 播放页附属只读接口的 Dart 导出（S3 第二批 / issue #53）。
//!
//! 形态照 `sponsor_block.rs` 与 `video_feed.rs`：typed 结构直连，失败用值
//! 不用异常，凭据与设置按请求传入（ADR-011）。
//!
//! ## `error_code` 在本模块有两种含义
//!
//! 多数接口是业务信封的 `code`；**AI 总结是内层的 `data.code`**——外层
//! `code == 0` 只说明请求被受理，内层 1 表示「正在处理中」。基线的调用点
//! 正是靠这个码区分「稍后再试」与「不支持」，所以它必须原样带回。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_api::video_page;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;
use bili_domain::video_page as domain;

use crate::api::init;
use crate::api::transport::TransportSettings;

/// AI 视频总结。
#[derive(Debug, Clone)]
pub struct AiConclusion {
    pub summary: Option<String>,
    pub outline: Option<Vec<AiOutline>>,
}

/// 总结里的一段。
#[derive(Debug, Clone)]
pub struct AiOutline {
    pub title: Option<String>,
    pub part_outline: Option<Vec<AiOutlinePart>>,
}

/// 段里的一条，`timestamp` 是秒。
#[derive(Debug, Clone)]
pub struct AiOutlinePart {
    pub timestamp: Option<i64>,
    pub content: Option<String>,
}

/// 进度条预览缩略图（雪碧图）。
#[derive(Debug, Clone)]
pub struct VideoShot {
    pub pvdata: Option<String>,
    pub img_x_len: i64,
    pub img_y_len: i64,
    pub img_x_size: f64,
    pub img_y_size: f64,
    /// 已统一成 https。
    pub images: Vec<String>,
    /// 每一格对应的时间点（**秒**）。保证非空：空的时候整条请求就失败了。
    ///
    /// 这里刻意收成 `i32`：FRB 把 `Vec<i64>` 映射成元素是 `BigInt` 的
    /// `Int64List`，而这个列表在播放器里是每次拖动进度条都要遍历的热路径，
    /// 每个元素都变成 BigInt 既慢又让调用点难写。视频时长用秒计不可能超过
    /// `i32`（约 68 年），收窄是安全的。领域层仍是 `i64`。
    pub index: Vec<i32>,
}

/// 播放信息。
#[derive(Debug, Clone)]
pub struct PlayInfo {
    pub last_play_cid: Option<i64>,
    /// 原样顺序。「中文优先、AI 靠后」的排序是界面偏好，留给 Dart。
    pub subtitles: Option<Vec<PlaySubtitle>>,
    pub view_points: Option<Vec<ViewPoint>>,
    pub interaction: Option<Interaction>,
}

/// 一条字幕。
#[derive(Debug, Clone)]
pub struct PlaySubtitle {
    pub id: Option<i64>,
    pub lan: Option<String>,
    /// 服务端给的语言名，**不含**基线补的「（AI）」后缀。
    pub lan_doc: Option<String>,
    pub subtitle_url: Option<String>,
    /// `1` 表示 AI 生成。
    pub kind: Option<i64>,
}

/// 分段看点。
#[derive(Debug, Clone)]
pub struct ViewPoint {
    /// 基线只在 `kind == 2` 时画分段进度条。
    pub kind: Option<i64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub content: Option<String>,
    pub img_url: Option<String>,
}

/// 互动视频（剧情树）。
#[derive(Debug, Clone)]
pub struct Interaction {
    pub graph_version: Option<i64>,
    pub history_node: Option<HistoryNode>,
}

/// 上次停在剧情树的哪个节点。
#[derive(Debug, Clone)]
pub struct HistoryNode {
    pub node_id: Option<i64>,
    pub title: Option<String>,
    pub cid: Option<i64>,
}

/// 笔记列表的一页。
#[derive(Debug, Clone)]
pub struct NoteList {
    pub total: Option<i64>,
    pub list: Option<Vec<NoteItem>>,
}

/// 一条笔记。
#[derive(Debug, Clone)]
pub struct NoteItem {
    pub cvid: Option<i64>,
    pub summary: Option<String>,
    /// 服务端给的展示用时间串，不是时间戳。
    pub pubtime: Option<String>,
    pub author: Option<NoteAuthor>,
}

/// 笔记作者。
#[derive(Debug, Clone)]
pub struct NoteAuthor {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub level: Option<i64>,
    pub is_senior_member: Option<i64>,
    pub vip_type: Option<i64>,
    pub vip_status: Option<i64>,
    pub vip_label_text: Option<String>,
    pub pendant_image: Option<String>,
    pub official_type: Option<i64>,
    pub official_desc: Option<String>,
}

/// 返回一个字符串的结果（同时在看人数，服务端已排好版）。
#[derive(Debug, Clone)]
pub struct VideoPageTextResult {
    pub data: Option<String>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// AI 总结的结果。`error_code` 是**内层** `data.code`（1 = 处理中）。
#[derive(Debug, Clone)]
pub struct AiConclusionResult {
    pub data: Option<AiConclusion>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 播放信息的结果。
#[derive(Debug, Clone)]
pub struct PlayInfoResult {
    pub data: Option<PlayInfo>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 预览缩略图的结果。没有缩略图时 `data` 为空且带一个 `remote` 失败。
#[derive(Debug, Clone)]
pub struct VideoShotResult {
    pub data: Option<VideoShot>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 笔记列表的结果。
#[derive(Debug, Clone)]
pub struct NoteListResult {
    pub data: Option<NoteList>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 一次请求所需的公共参数。
#[derive(Debug, Clone)]
pub struct VideoPageContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// WBI mixin key。不签名的接口（在看人数、缩略图、笔记）传空串即可。
    pub mixin_key: String,
    /// Unix 秒，由 Dart 注入。
    pub wts: i64,
}

impl VideoPageContext {
    fn split(self) -> (RequestContext, ReqwestTransport, String, i64) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.mixin_key, self.wts)
    }
}

/// 取同时在看人数。`aid` 与 `bvid` 至少给一个。
pub async fn fetch_online_total(
    context: VideoPageContext,
    aid: Option<i64>,
    bvid: Option<String>,
    cid: i64,
) -> VideoPageTextResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let video = video_page::VideoRef { aid, bvid };
            video_page::fetch_online_total(&transport, &ctx, &video, cid).await
        })
        .await;

    match flatten(joined) {
        Ok(total) => VideoPageTextResult {
            data: Some(total),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => VideoPageTextResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取 AI 总结。失败时 `error_code` 是内层 `data.code`。
pub async fn fetch_ai_conclusion(
    context: VideoPageContext,
    bvid: String,
    cid: i64,
    up_mid: Option<i64>,
) -> AiConclusionResult {
    let (ctx, transport, mixin_key, wts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            video_page::fetch_ai_conclusion(&transport, &ctx, &bvid, cid, up_mid, &mixin_key, wts)
                .await
        })
        .await;

    match flatten(joined) {
        Ok(c) => AiConclusionResult {
            data: Some(AiConclusion::from_domain(c)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => AiConclusionResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取播放信息。
pub async fn fetch_play_info(
    context: VideoPageContext,
    aid: Option<i64>,
    bvid: Option<String>,
    cid: i64,
    season_id: Option<i64>,
    ep_id: Option<i64>,
) -> PlayInfoResult {
    let (ctx, transport, mixin_key, wts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let target = video_page::PlayInfoTarget {
                video: video_page::VideoRef { aid, bvid },
                cid,
                season_id,
                ep_id,
            };
            video_page::fetch_play_info(&transport, &ctx, &target, &mixin_key, wts).await
        })
        .await;

    match flatten(joined) {
        Ok(info) => PlayInfoResult {
            data: Some(PlayInfo::from_domain(info)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PlayInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取进度条预览缩略图。这个视频没有缩略图时也是失败（`remote`）。
pub async fn fetch_video_shot(
    context: VideoPageContext,
    bvid: String,
    cid: i64,
) -> VideoShotResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { video_page::fetch_video_shot(&transport, &ctx, &bvid, cid).await })
        .await;

    match flatten(joined) {
        Ok(shot) => VideoShotResult {
            data: Some(VideoShot {
                pvdata: shot.pvdata,
                img_x_len: shot.img_x_len,
                img_y_len: shot.img_y_len,
                img_x_size: shot.img_x_size,
                img_y_size: shot.img_y_size,
                images: shot.images,
                index: shot
                    .index
                    .into_iter()
                    // 见字段文档：秒数不可能溢出 i32，真溢出了也只是预览图
                    // 取错一格，不值得让整条请求失败。
                    .map(|v| i32::try_from(v).unwrap_or(i32::MAX))
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => VideoShotResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取笔记列表的一页。`csrf` 由 Dart 的账号体系给出，Rust 不存。
pub async fn fetch_note_list(
    context: VideoPageContext,
    csrf: String,
    oid: i64,
    page: i64,
    uper_mid: Option<i64>,
) -> NoteListResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            video_page::fetch_note_list(&transport, &ctx, &csrf, oid, page, uper_mid).await
        })
        .await;

    match flatten(joined) {
        Ok(list) => NoteListResult {
            data: Some(NoteList {
                total: list.total,
                list: list
                    .list
                    .map(|items| items.into_iter().map(NoteItem::from_domain).collect()),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => NoteListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 把 `JoinHandle` 的两层结果压平。任务 panic 不传给 Dart。
fn flatten<T>(
    joined: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    match joined {
        Ok(inner) => inner,
        Err(e) => Err(DomainError::new(
            bili_domain::error::ErrorKind::Network,
            format!("取数任务未完成: {e}"),
        )),
    }
}

impl AiConclusion {
    fn from_domain(c: domain::AiConclusion) -> Self {
        Self {
            summary: c.summary,
            outline: c.outline.map(|list| {
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
        }
    }
}

impl PlayInfo {
    fn from_domain(i: domain::PlayInfo) -> Self {
        Self {
            last_play_cid: i.last_play_cid,
            subtitles: i.subtitles.map(|list| {
                list.into_iter()
                    .map(|s| PlaySubtitle {
                        id: s.id,
                        lan: s.lan,
                        lan_doc: s.lan_doc,
                        subtitle_url: s.subtitle_url,
                        kind: s.kind,
                    })
                    .collect()
            }),
            view_points: i.view_points.map(|list| {
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
            interaction: i.interaction.map(|i| Interaction {
                graph_version: i.graph_version,
                history_node: i.history_node.map(|n| HistoryNode {
                    node_id: n.node_id,
                    title: n.title,
                    cid: n.cid,
                }),
            }),
        }
    }
}

impl NoteItem {
    fn from_domain(i: domain::NoteItem) -> Self {
        Self {
            cvid: i.cvid,
            summary: i.summary,
            pubtime: i.pubtime,
            author: i.author.map(|a| NoteAuthor {
                mid: a.mid,
                name: a.name,
                face: a.face,
                level: a.level,
                is_senior_member: a.is_senior_member,
                vip_type: a.vip_type,
                vip_status: a.vip_status,
                vip_label_text: a.vip_label_text,
                pendant_image: a.pendant_image,
                official_type: a.official_type,
                official_desc: a.official_desc,
            }),
        }
    }
}
