//! 直播消息的批量直连桥；协议和连接状态全部留在数据层。
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};

use bili_api::live_socket::SocketSession;
use bili_domain::error::{DomainError, ErrorKind};
use flutter_rust_bridge::frb;

use super::init;
use super::live::LiveContext;
use super::transport::TransportSettings;

static NEXT_ID: AtomicI64 = AtomicI64::new(1);
static SESSIONS: OnceLock<Mutex<HashMap<i64, (i64, SocketSession)>>> = OnceLock::new();
fn sessions() -> &'static Mutex<HashMap<i64, (i64, SocketSession)>> {
    SESSIONS.get_or_init(Default::default)
}

#[derive(Debug, Clone)]
pub struct LiveSocketHandle {
    pub session_id: i64,
    pub generation: i64,
}
#[derive(Debug, Clone)]
pub struct LiveSocketOpenResult {
    pub data: Option<LiveSocketHandle>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct LiveSocketBatch {
    pub session_id: i64,
    pub generation: i64,
    pub events: Vec<LiveEvent>,
    pub dropped: u64,
    pub ended: bool,
}
#[derive(Debug, Clone)]
pub struct LiveSocketPollResult {
    pub data: Option<LiveSocketBatch>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct LiveSocketActionResult {
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[frb(sync)]
pub fn live_socket_open(
    servers: Vec<String>,
    room_id: i64,
    uid: i64,
    token: String,
    generation: i64,
    transport: TransportSettings,
    rules: LiveBlockRules,
) -> LiveSocketOpenResult {
    if servers.is_empty() || room_id <= 0 {
        return LiveSocketOpenResult {
            data: None,
            error_kind: Some("protocol".into()),
            error_detail: Some("直播弹幕缺少房间或服务器".into()),
            error_code: None,
        };
    }
    let session_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let _guard = init::runtime().enter();
    let session = SocketSession::start(servers, room_id, uid, token, transport.to_config());
    session.set_rules(rules.into());
    sessions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(session_id, (generation, session));
    LiveSocketOpenResult {
        data: Some(LiveSocketHandle {
            session_id,
            generation,
        }),
        error_kind: None,
        error_detail: None,
        error_code: None,
    }
}

#[frb(sync)]
pub fn live_socket_poll(session_id: i64, generation: i64) -> LiveSocketPollResult {
    let guard = sessions().lock().unwrap_or_else(|e| e.into_inner());
    let Some((stored_generation, session)) = guard
        .get(&session_id)
        .filter(|(stored, _)| *stored == generation)
    else {
        return LiveSocketPollResult {
            data: None,
            error_kind: Some("cancelled".into()),
            error_detail: Some("直播弹幕会话已失效".into()),
            error_code: None,
        };
    };
    let batch = session.poll();
    let error = batch.error;
    LiveSocketPollResult {
        data: Some(LiveSocketBatch {
            session_id,
            generation: *stored_generation,
            events: batch.events.into_iter().map(Into::into).collect(),
            dropped: batch.dropped,
            ended: batch.ended,
        }),
        error_kind: error.as_ref().map(|e| e.kind().to_string()),
        error_detail: error.as_ref().map(|e| e.detail().to_owned()),
        error_code: error.and_then(|e| e.code()),
    }
}

#[frb(sync)]
pub fn live_socket_set_rules(
    session_id: i64,
    generation: i64,
    rules: LiveBlockRules,
) -> LiveSocketActionResult {
    let guard = sessions().lock().unwrap_or_else(|e| e.into_inner());
    match guard
        .get(&session_id)
        .filter(|(stored, _)| *stored == generation)
    {
        Some((_, session)) => {
            session.set_rules(rules.into());
            LiveSocketActionResult {
                error_kind: None,
                error_detail: None,
                error_code: None,
            }
        }
        None => LiveSocketActionResult {
            error_kind: Some("cancelled".into()),
            error_detail: Some("直播弹幕会话已失效".into()),
            error_code: None,
        },
    }
}

#[frb(sync)]
pub fn live_socket_close(session_id: i64, generation: i64) -> LiveSocketActionResult {
    let mut guard = sessions().lock().unwrap_or_else(|e| e.into_inner());
    // 过期 close 不能终止同 id 的新代际。重复 close 是幂等成功。
    if guard
        .get(&session_id)
        .is_some_and(|(stored, _)| *stored == generation)
    {
        guard.remove(&session_id);
    }
    LiveSocketActionResult {
        error_kind: None,
        error_detail: None,
        error_code: None,
    }
}

// 领域实体的桥接投影；转换不加入宿主规则。
use super::live::LiveMedal;
#[derive(Debug, Clone)]
pub struct LiveDanmakuToken {
    pub token: String,
    pub servers: Vec<String>,
}
impl From<bili_domain::live_danmaku::LiveDanmakuToken> for LiveDanmakuToken {
    fn from(value: bili_domain::live_danmaku::LiveDanmakuToken) -> Self {
        Self {
            token: value.token,
            servers: value.servers,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveDanmakuEmote {
    pub is_official: bool,
    pub url: String,
    pub emoticon_unique: String,
    pub width: f64,
    pub height: f64,
}
impl From<bili_domain::live_danmaku::LiveDanmakuEmote> for LiveDanmakuEmote {
    fn from(value: bili_domain::live_danmaku::LiveDanmakuEmote) -> Self {
        Self {
            is_official: value.is_official,
            url: value.url,
            emoticon_unique: value.emoticon_unique,
            width: value.width,
            height: value.height,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveDanmakuEmoteEntry {
    pub key: String,
    pub emote: LiveDanmakuEmote,
}
impl From<bili_domain::live_danmaku::LiveDanmakuEmoteEntry> for LiveDanmakuEmoteEntry {
    fn from(value: bili_domain::live_danmaku::LiveDanmakuEmoteEntry) -> Self {
        Self {
            key: value.key,
            emote: value.emote.into(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveDanmakuReply {
    pub mid: i64,
    pub name: Option<String>,
}
impl From<bili_domain::live_danmaku::LiveDanmakuReply> for LiveDanmakuReply {
    fn from(value: bili_domain::live_danmaku::LiveDanmakuReply) -> Self {
        Self {
            mid: value.mid,
            name: value.name,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveMessageSegment {
    pub text: String,
    pub emote_index: Option<u32>,
}
impl From<bili_domain::live_danmaku::LiveMessageSegment> for LiveMessageSegment {
    fn from(value: bili_domain::live_danmaku::LiveMessageSegment) -> Self {
        Self {
            text: value.text,
            emote_index: value.emote_index,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveDanmakuMessage {
    pub id: String,
    pub mid: i64,
    pub dm_type: i64,
    pub ts: i64,
    pub ct: String,
    pub color: i64,
    pub mode: i64,
    pub name: String,
    pub text: String,
    pub segments: Vec<LiveMessageSegment>,
    pub emotes: Vec<LiveDanmakuEmoteEntry>,
    pub uemote: Option<LiveDanmakuEmote>,
    pub reply: Option<LiveDanmakuReply>,
    pub medal: Option<LiveMedal>,
}
impl From<bili_domain::live_danmaku::LiveDanmakuMessage> for LiveDanmakuMessage {
    fn from(value: bili_domain::live_danmaku::LiveDanmakuMessage) -> Self {
        Self {
            id: value.id,
            mid: value.mid,
            dm_type: value.dm_type,
            ts: value.ts,
            ct: value.ct,
            color: value.color,
            mode: value.mode,
            name: value.name,
            text: value.text,
            segments: value.segments.into_iter().map(Into::into).collect(),
            emotes: value.emotes.into_iter().map(Into::into).collect(),
            uemote: value.uemote.map(Into::into),
            reply: value.reply.map(Into::into),
            medal: value.medal.map(Into::into),
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveSuperChatUser {
    pub face: String,
    pub face_frame: Option<String>,
    pub uname: String,
    pub name_color: String,
}
impl From<bili_domain::live_danmaku::LiveSuperChatUser> for LiveSuperChatUser {
    fn from(value: bili_domain::live_danmaku::LiveSuperChatUser) -> Self {
        Self {
            face: value.face,
            face_frame: value.face_frame,
            uname: value.uname,
            name_color: value.name_color,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveSuperChat {
    pub id: i64,
    pub uid: i64,
    pub price: i64,
    pub background_image: Option<String>,
    pub background_color: String,
    pub background_bottom_color: String,
    pub background_price_color: String,
    pub message_font_color: String,
    pub start_time: i64,
    pub end_time: i64,
    pub message: String,
    pub token: String,
    pub ts: i64,
    pub user_info: LiveSuperChatUser,
    pub medal: Option<LiveMedal>,
    pub room_id: i64,
}
impl From<bili_domain::live_danmaku::LiveSuperChat> for LiveSuperChat {
    fn from(value: bili_domain::live_danmaku::LiveSuperChat) -> Self {
        Self {
            id: value.id,
            uid: value.uid,
            price: value.price,
            background_image: value.background_image,
            background_color: value.background_color,
            background_bottom_color: value.background_bottom_color,
            background_price_color: value.background_price_color,
            message_font_color: value.message_font_color,
            start_time: value.start_time,
            end_time: value.end_time,
            message: value.message,
            token: value.token,
            ts: value.ts,
            user_info: value.user_info.into(),
            medal: value.medal.map(Into::into),
            room_id: value.room_id,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveEvent {
    pub kind: LiveEventKind,
    pub danmaku: Option<LiveDanmakuMessage>,
    pub super_chat: Option<LiveSuperChat>,
    pub text: Option<String>,
    pub count: Option<i64>,
}
impl From<bili_domain::live_danmaku::LiveEvent> for LiveEvent {
    fn from(value: bili_domain::live_danmaku::LiveEvent) -> Self {
        Self {
            kind: value.kind.into(),
            danmaku: value.danmaku.map(Into::into),
            super_chat: value.super_chat.map(Into::into),
            text: value.text,
            count: value.count,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveBlockRules {
    pub keywords: Vec<String>,
    pub uids: Vec<i64>,
}
impl From<bili_domain::live_danmaku::LiveBlockRules> for LiveBlockRules {
    fn from(value: bili_domain::live_danmaku::LiveBlockRules) -> Self {
        Self {
            keywords: value.keywords,
            uids: value.uids,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum LiveEventKind {
    Danmaku,
    SuperChat,
    WatchedChange,
    OnlineRankCount,
    RoomChange,
}
impl From<bili_domain::live_danmaku::LiveEventKind> for LiveEventKind {
    fn from(value: bili_domain::live_danmaku::LiveEventKind) -> Self {
        match value {
            bili_domain::live_danmaku::LiveEventKind::Danmaku => Self::Danmaku,
            bili_domain::live_danmaku::LiveEventKind::SuperChat => Self::SuperChat,
            bili_domain::live_danmaku::LiveEventKind::WatchedChange => Self::WatchedChange,
            bili_domain::live_danmaku::LiveEventKind::OnlineRankCount => Self::OnlineRankCount,
            bili_domain::live_danmaku::LiveEventKind::RoomChange => Self::RoomChange,
        }
    }
}
impl From<bili_domain::live::LiveMedal> for LiveMedal {
    fn from(value: bili_domain::live::LiveMedal) -> Self {
        Self {
            name: value.name,
            level: value.level,
            id: value.id,
            ruid: value.ruid,
            color_start: value.color_start,
            color_text: value.color_text,
        }
    }
}
impl From<LiveBlockRules> for bili_domain::live_danmaku::LiveBlockRules {
    fn from(value: LiveBlockRules) -> Self {
        Self {
            keywords: value.keywords,
            uids: value.uids,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LiveDanmakuTokenResult {
    pub data: Option<LiveDanmakuToken>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}
pub async fn fetch_live_danmaku_token(
    context: LiveContext,
    room_id: i64,
    mixin_key: String,
    wts: i64,
) -> LiveDanmakuTokenResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            bili_api::live_danmaku::fetch_token(&transport, &ctx, room_id, &mixin_key, wts).await
        })
        .await;
    let result = joined.unwrap_or_else(|_| {
        Err(DomainError::new(
            ErrorKind::Protocol,
            "直播消息任务异常终止",
        ))
    });
    match result {
        Ok(value) => LiveDanmakuTokenResult {
            data: Some(value.into()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveDanmakuTokenResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}
#[derive(Debug, Clone)]
pub struct LiveDanmakuPrefetchResult {
    pub data: Option<Vec<LiveDanmakuMessage>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}
pub async fn fetch_live_danmaku_prefetch(
    context: LiveContext,
    room_id: i64,
    user_agent: String,
) -> LiveDanmakuPrefetchResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            bili_api::live_danmaku::fetch_prefetch(&transport, &ctx, room_id, &user_agent).await
        })
        .await;
    let result = joined.unwrap_or_else(|_| {
        Err(DomainError::new(
            ErrorKind::Protocol,
            "直播消息任务异常终止",
        ))
    });
    match result {
        Ok(value) => LiveDanmakuPrefetchResult {
            data: Some(value.into_iter().map(Into::into).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveDanmakuPrefetchResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}
#[derive(Debug, Clone)]
pub struct LiveSuperChatResult {
    pub data: Option<Vec<LiveSuperChat>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}
pub async fn fetch_live_super_chat(context: LiveContext, room_id: i64) -> LiveSuperChatResult {
    let (ctx, transport) = context.split();
    let joined =
        init::runtime()
            .spawn(async move {
                bili_api::live_danmaku::fetch_super_chat(&transport, &ctx, room_id).await
            })
            .await;
    let result = joined.unwrap_or_else(|_| {
        Err(DomainError::new(
            ErrorKind::Protocol,
            "直播消息任务异常终止",
        ))
    });
    match result {
        Ok(value) => LiveSuperChatResult {
            data: Some(value.into_iter().map(Into::into).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveSuperChatResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 历史预取在请求完成时使用最新屏蔽设置，一次跨桥返回整批保留掩码。
/// 避免把过滤决策搬回宿主，也不必把整棵表情/勋章数据再编码回 Rust。
#[derive(Debug, Clone)]
pub struct LiveFilterCandidate {
    pub text: String,
    pub mid: i64,
}

#[frb(sync)]
pub fn live_danmaku_filter_mask(
    candidates: Vec<LiveFilterCandidate>,
    rules: LiveBlockRules,
) -> Vec<bool> {
    let rules: bili_domain::live_danmaku::LiveBlockRules = rules.into();
    candidates
        .into_iter()
        .map(|candidate| !rules.is_blocked(&candidate.text, candidate.mid))
        .collect()
}
