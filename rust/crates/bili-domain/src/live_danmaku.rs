//! 直播弹幕及醒目留言的宿主无关实体与屏蔽规则。
use crate::live::LiveMedal;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveDanmakuToken {
    pub token: String,
    pub servers: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveDanmakuEmote {
    pub is_official: bool,
    pub url: String,
    pub emoticon_unique: String,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveDanmakuEmoteEntry {
    pub key: String,
    pub emote: LiveDanmakuEmote,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveDanmakuReply {
    pub mid: i64,
    pub name: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveMessageSegment {
    pub text: String,
    pub emote: Option<LiveDanmakuEmote>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveSuperChatUser {
    pub face: String,
    pub face_frame: Option<String>,
    pub uname: String,
    pub name_color: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LiveEventKind {
    Danmaku,
    SuperChat,
    WatchedChange,
    OnlineRankCount,
    RoomChange,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveEvent {
    pub kind: LiveEventKind,
    pub danmaku: Option<LiveDanmakuMessage>,
    pub super_chat: Option<LiveSuperChat>,
    pub text: Option<String>,
    pub count: Option<i64>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LiveBlockRules {
    pub keywords: Vec<String>,
    pub uids: Vec<i64>,
}
impl LiveBlockRules {
    #[must_use]
    pub fn is_blocked(&self, text: &str, uid: i64) -> bool {
        self.keywords.iter().any(|key| text.contains(key)) || self.uids.contains(&uid)
    }
}
