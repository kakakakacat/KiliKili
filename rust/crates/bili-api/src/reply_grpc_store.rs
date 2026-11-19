//! 「我的评论」保存下来的那些评论的存储与导入导出格式（issue #176）。
//!
//! ## 这为什么不是界面的事
//!
//! 基线把这件事交给 protobuf 生成类：本地 Hive 盒子里存
//! `ReplyInfo.writeToBuffer()` 的字节，导出 / 配置归档用
//! `toProto3Json()`，导入用 `mergeFromProto3Json()`。pb 子树随本切片删掉
//! 之后这三处就没人实现了——而它们是**用户数据**，不能跟着一起消失。
//!
//! 按 AGENTS.md §2 的判据，这是「协议没规定、但两端必须一致」的决定：
//! 换掉宿主框架时，存档格式不该重新发明一次。所以它跟着规则一起下沉。
//!
//! ## 格式：读两种，写一种
//!
//! [`StoredReply`] 的字段名刻意与 **proto3 JSON** 一致（lowerCamelCase、
//! int64 写成字符串、map 写成对象）。于是：
//!
//! - **老的导出文件**（`ReplyInfo.toProto3Json()` 的数组）能原样读进来；
//! - **老的 Hive 盒子**里是 protobuf 字节，[`decode_stored`] 认不出 JSON
//!   时按 protobuf 再试一次（见那里的说明）；
//! - 新写出去的一律是 [`StoredReply`] 的 JSON。
//!
//! 新格式是老格式的**超集**：有几个字段在 pb 里是隐含的（例如「这个链接
//! 是不是笔记」靠 `clickReport` 的前缀认），领域实体里已经算成了布尔，
//! 写回去时就直接写那个布尔。老文件没有这些字段，读的时候退回按 pb 的
//! 规则现算。
//!
//! ## 刻意没做的
//!
//! **不往回写 protobuf 字节。** 领域实体比 pb 窄（那正是 ADR-012 决策五
//! 要的），拼回一个 `ReplyInfo` 只能拼出一个缺字段的赝品，而它看上去与
//! 真的一模一样。宁可换一种一眼能认出来的格式。

use std::collections::HashMap;

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::reply_grpc::{
    ReplyCard, ReplyContent, ReplyControlInfo, ReplyEmote, ReplyEntity, ReplyMedal, ReplyMember,
    ReplyNote, ReplyParentMember, ReplyPicture, ReplyUrl, ReplyVote, ReplyVoteOption,
};
use bili_grpc::bilibili::main::community::reply::v1 as pb;
use prost::Message;
use serde::{Deserialize, Deserializer, Serialize};

/// proto3 JSON 把 int64 写成字符串（绕开 JS 的 53 位精度），但
/// `mergeFromProto3Json` 也收数字，老文件两种都可能出现。两种都收。
mod int64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Text(String),
            Number(i64),
        }
        Ok(match Either::deserialize(deserializer)? {
            // 空串按 0：proto3 JSON 不会产生空串，但手工编辑过的文件会。
            Either::Text(text) => text.trim().parse().unwrap_or_default(),
            Either::Number(number) => number,
        })
    }
}

/// 可选的 int64，缺字段与 `null` 都是「没有」。
mod int64_opt {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &Option<i64>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_str(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<i64>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Text(String),
            Number(i64),
        }
        Ok(
            Option::<Either>::deserialize(deserializer)?.and_then(|either| match either {
                Either::Text(text) => text.trim().parse().ok(),
                Either::Number(number) => Some(number),
            }),
        )
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

// ─────────────────────── 存储 DTO（proto3 JSON 形状） ───────────────────────

/// 存档里的一条评论。字段名与 `ReplyInfo` 的 proto3 JSON 一致。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredReply {
    #[serde(with = "int64")]
    pub id: i64,
    #[serde(with = "int64")]
    pub oid: i64,
    /// pb 里就叫 `type`，领域实体里叫 `kind`。存档跟 pb。
    #[serde(rename = "type", with = "int64")]
    pub kind: i64,
    #[serde(with = "int64")]
    pub mid: i64,
    #[serde(with = "int64")]
    pub root: i64,
    #[serde(with = "int64")]
    pub parent: i64,
    #[serde(with = "int64")]
    pub dialog: i64,
    #[serde(with = "int64")]
    pub ctime: i64,
    #[serde(with = "int64")]
    pub like: i64,
    #[serde(with = "int64")]
    pub count: i64,
    pub content: StoredContent,
    pub translated_content: Option<StoredContent>,
    pub member: StoredMember,
    pub member_v2: Option<StoredMemberV2>,
    pub reply_control: StoredControl,
    pub parent_reply_member: Option<StoredParentMember>,
    /// 基线存盘前 `replies.clear()`，所以老文件里恒空。照旧不存楼中楼：
    /// 一条被保存的评论是一条快照，不是一棵会继续长的树。
    #[serde(skip)]
    pub replies: Vec<StoredReply>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredContent {
    pub message: String,
    pub emotes: HashMap<String, StoredEmote>,
    /// pb 是 `map<string, Topic>`，界面只用键。写回去时值写成空对象，
    /// 老文件里的值直接丢掉。
    pub topics: HashMap<String, serde_json::Value>,
    pub at_name_to_mid: HashMap<String, StoredMid>,
    pub urls: HashMap<String, StoredUrl>,
    pub vote: Option<StoredVote>,
    pub pictures: Vec<StoredPicture>,
    pub rich_text: Option<StoredRichText>,
    pub members: HashMap<String, StoredMember>,
}

/// `at_name_to_mid` 的值：proto3 JSON 的 int64 map 值同样是字符串。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StoredMid(#[serde(with = "int64")] pub i64);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredEmote {
    #[serde(with = "int64")]
    pub size: i64,
    pub url: String,
    pub gif_url: String,
    pub webp_url: String,
    pub jump_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredUrl {
    pub title: String,
    pub prefix_icon: String,
    pub app_url_schema: String,
    pub click_report: String,
    pub extra: Option<StoredUrlExtra>,
    /// 新增字段：老文件没有，读的时候退回按 `click_report` 的前缀算。
    pub is_note: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredUrlExtra {
    pub is_word_search: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredVote {
    #[serde(with = "int64")]
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredPicture {
    pub img_src: String,
    pub img_width: f64,
    pub img_height: f64,
    pub img_size: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredRichText {
    pub note: Option<StoredRichTextNote>,
    pub opus: Option<StoredRichTextOpus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredRichTextNote {
    pub click_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredRichTextOpus {
    #[serde(with = "int64_opt")]
    pub opus_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredMember {
    #[serde(with = "int64")]
    pub mid: i64,
    pub name: String,
    pub face: String,
    #[serde(with = "int64")]
    pub level: i64,
    #[serde(with = "int64_opt")]
    pub official_verify_type: Option<i64>,
    #[serde(with = "int64")]
    pub vip_type: i64,
    #[serde(with = "int64")]
    pub vip_status: i64,
    pub garb_pendant_image: String,
    pub fans_medal_name: String,
    #[serde(with = "int64_opt")]
    pub fans_medal_level: Option<i64>,
    #[serde(with = "int64")]
    pub fans_medal_color: i64,
    #[serde(with = "int64")]
    pub fans_medal_color_name: i64,
    /// pb 是 int32，1 表示硬核会员。
    pub is_senior_member: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredMemberV2 {
    pub garb: Option<StoredGarb>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredGarb {
    pub card_image: String,
    pub card_number: Option<String>,
    pub fan_num_prefix: String,
    pub card_fan_color: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredControl {
    #[serde(with = "int64", skip_serializing_if = "is_zero")]
    pub action: i64,
    #[serde(skip_serializing_if = "is_false")]
    pub is_up_top: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub show_translation: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub up_reply: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub is_note: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub is_note_v2: bool,
    pub location: String,
    /// proto3 JSON 里是枚举名。读老文件认
    /// `TRANSLATION_SWITCH_SHOW_TRANSLATION`，写新文件写同一个串。
    pub translation_switch: String,
    pub card_labels: Vec<StoredCardLabel>,
    pub vote_option: Option<StoredVoteOption>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredCardLabel {
    pub text_content: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredVoteOption {
    /// proto3 JSON 的枚举名；0/1/2 对应 `NONE` / `RED` / `BLUE`。
    pub label_kind: StoredLabelKind,
    pub desc: String,
}

/// 投票选项的阵营。老文件里是枚举名，也可能是数字。
#[derive(Debug, Clone, Copy, Default)]
pub struct StoredLabelKind(pub i64);

impl Serialize for StoredLabelKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            1 => "RED",
            2 => "BLUE",
            _ => "NONE",
        })
    }
}

impl<'de> Deserialize<'de> for StoredLabelKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Text(String),
            Number(i64),
        }
        Ok(Self(match Either::deserialize(deserializer)? {
            Either::Text(text) => match text.as_str() {
                "RED" => 1,
                "BLUE" => 2,
                _ => 0,
            },
            Either::Number(number) => number,
        }))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredParentMember {
    #[serde(with = "int64_opt")]
    pub mid: Option<i64>,
    pub name: String,
}

const SHOW_TRANSLATION: &str = "TRANSLATION_SWITCH_SHOW_TRANSLATION";

// ─────────────────────────── DTO → 实体 ───────────────────────────

fn optional(value: String) -> Option<String> {
    Some(value).filter(|s| !s.is_empty())
}

impl StoredEmote {
    fn into_entity(self) -> ReplyEmote {
        ReplyEmote {
            size: self.size,
            // 与 `grpc_reply::project_emote` 同一条回退链。
            display_url: [self.webp_url, self.gif_url, self.url]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default(),
            jump_url: optional(self.jump_url),
        }
    }
}

impl StoredUrl {
    fn into_entity(self) -> ReplyUrl {
        let is_note = self
            .is_note
            .unwrap_or_else(|| self.click_report.starts_with("{\"cvid"));
        ReplyUrl {
            title: self.title,
            prefix_icon: optional(self.prefix_icon),
            app_url_schema: self.app_url_schema,
            is_word_search: self.extra.is_some_and(|e| e.is_word_search),
            is_note,
        }
    }
}

impl StoredMember {
    fn into_entity(self) -> ReplyMember {
        ReplyMember {
            mid: self.mid,
            name: self.name,
            face: self.face,
            level: self.level,
            is_senior: self.is_senior_member == 1,
            vip_status: self.vip_status,
            vip_type: self.vip_type,
            official_verify_type: self.official_verify_type,
            pendant_image: optional(self.garb_pendant_image),
            medal: self.fans_medal_level.map(|level| ReplyMedal {
                name: self.fans_medal_name,
                level,
                color: self.fans_medal_color,
                color_name: self.fans_medal_color_name,
            }),
        }
    }
}

impl StoredContent {
    fn into_entity(self) -> ReplyContent {
        ReplyContent {
            message: self.message,
            emotes: self
                .emotes
                .into_iter()
                .map(|(key, emote)| (key, emote.into_entity()))
                .collect(),
            topics: self.topics.into_keys().collect(),
            at_name_to_mid: self
                .at_name_to_mid
                .into_iter()
                .map(|(key, mid)| (key, mid.0))
                .collect(),
            urls: self
                .urls
                .into_iter()
                .map(|(key, url)| (key, url.into_entity()))
                .collect(),
            vote: self.vote.map(|v| ReplyVote {
                id: v.id,
                title: v.title,
            }),
            pictures: self
                .pictures
                .into_iter()
                .map(|p| ReplyPicture {
                    img_src: p.img_src,
                    img_width: p.img_width,
                    img_height: p.img_height,
                    img_size: p.img_size,
                })
                .collect(),
            note: self.rich_text.and_then(|rich| {
                let click_url = rich.note.and_then(|n| optional(n.click_url));
                let opus_id = rich.opus.and_then(|o| o.opus_id);
                (click_url.is_some() || opus_id.is_some())
                    .then_some(ReplyNote { click_url, opus_id })
            }),
            members: self
                .members
                .into_iter()
                .map(|(key, member)| (key, member.into_entity()))
                .collect(),
        }
    }
}

impl StoredControl {
    fn into_entity(self) -> ReplyControlInfo {
        ReplyControlInfo {
            action: self.action,
            is_up_top: self.is_up_top,
            show_translation: self.show_translation,
            up_reply: self.up_reply,
            is_note: self.is_note,
            is_note_v2: self.is_note_v2,
            location: optional(self.location),
            can_translate: self.translation_switch == SHOW_TRANSLATION,
            card_labels: self
                .card_labels
                .into_iter()
                .map(|label| label.text_content)
                .collect(),
            vote_option: self.vote_option.map(|o| ReplyVoteOption {
                label_kind: o.label_kind.0,
                desc: o.desc,
            }),
        }
    }
}

impl StoredReply {
    /// 存档 → 领域实体。
    #[must_use]
    pub fn into_entity(self) -> ReplyEntity {
        ReplyEntity {
            id: self.id,
            oid: self.oid,
            kind: self.kind,
            mid: self.mid,
            root: self.root,
            parent: self.parent,
            dialog: self.dialog,
            ctime: self.ctime,
            like: self.like,
            count: self.count,
            content: self.content.into_entity(),
            translated_content: self.translated_content.map(StoredContent::into_entity),
            member: self.member.into_entity(),
            card: self
                .member_v2
                .and_then(|v2| v2.garb)
                .filter(|garb| !garb.card_image.is_empty())
                .map(|garb| ReplyCard {
                    image: garb.card_image,
                    number: garb.card_number,
                    fan_num_prefix: garb.fan_num_prefix,
                    fan_color: garb.card_fan_color,
                }),
            control: self.reply_control.into_entity(),
            parent_member: self.parent_reply_member.map(|m| ReplyParentMember {
                mid: m.mid,
                name: m.name,
            }),
            replies: self
                .replies
                .into_iter()
                .map(StoredReply::into_entity)
                .collect(),
        }
    }

    /// 领域实体 → 存档。
    #[must_use]
    pub fn from_entity(entity: &ReplyEntity) -> Self {
        Self {
            id: entity.id,
            oid: entity.oid,
            kind: entity.kind,
            mid: entity.mid,
            root: entity.root,
            parent: entity.parent,
            dialog: entity.dialog,
            ctime: entity.ctime,
            like: entity.like,
            count: entity.count,
            content: StoredContent::from_entity(&entity.content),
            translated_content: entity
                .translated_content
                .as_ref()
                .map(StoredContent::from_entity),
            member: StoredMember::from_entity(&entity.member),
            member_v2: entity.card.as_ref().map(|card| StoredMemberV2 {
                garb: Some(StoredGarb {
                    card_image: card.image.clone(),
                    card_number: card.number.clone(),
                    fan_num_prefix: card.fan_num_prefix.clone(),
                    card_fan_color: card.fan_color.clone(),
                }),
            }),
            reply_control: StoredControl::from_entity(&entity.control),
            parent_reply_member: entity.parent_member.as_ref().map(|m| StoredParentMember {
                mid: m.mid,
                name: m.name.clone(),
            }),
            replies: Vec::new(),
        }
    }
}

impl StoredContent {
    fn from_entity(content: &ReplyContent) -> Self {
        Self {
            message: content.message.clone(),
            emotes: content
                .emotes
                .iter()
                .map(|(key, emote)| {
                    (
                        key.clone(),
                        StoredEmote {
                            size: emote.size,
                            url: emote.display_url.clone(),
                            gif_url: String::new(),
                            webp_url: String::new(),
                            jump_url: emote.jump_url.clone().unwrap_or_default(),
                        },
                    )
                })
                .collect(),
            topics: content
                .topics
                .iter()
                .map(|topic| (topic.clone(), serde_json::Value::Object(Default::default())))
                .collect(),
            at_name_to_mid: content
                .at_name_to_mid
                .iter()
                .map(|(name, mid)| (name.clone(), StoredMid(*mid)))
                .collect(),
            urls: content
                .urls
                .iter()
                .map(|(key, url)| {
                    (
                        key.clone(),
                        StoredUrl {
                            title: url.title.clone(),
                            prefix_icon: url.prefix_icon.clone().unwrap_or_default(),
                            app_url_schema: url.app_url_schema.clone(),
                            click_report: String::new(),
                            extra: Some(StoredUrlExtra {
                                is_word_search: url.is_word_search,
                            }),
                            // 显式写出来，不指望读的那一方再从 click_report 猜。
                            is_note: Some(url.is_note),
                        },
                    )
                })
                .collect(),
            vote: content.vote.as_ref().map(|v| StoredVote {
                id: v.id,
                title: v.title.clone(),
            }),
            pictures: content
                .pictures
                .iter()
                .map(|p| StoredPicture {
                    img_src: p.img_src.clone(),
                    img_width: p.img_width,
                    img_height: p.img_height,
                    img_size: p.img_size,
                })
                .collect(),
            rich_text: content.note.as_ref().map(|note| StoredRichText {
                note: note
                    .click_url
                    .clone()
                    .map(|click_url| StoredRichTextNote { click_url }),
                opus: note.opus_id.map(|opus_id| StoredRichTextOpus {
                    opus_id: Some(opus_id),
                }),
            }),
            members: content
                .members
                .iter()
                .map(|(key, member)| (key.clone(), StoredMember::from_entity(member)))
                .collect(),
        }
    }
}

impl StoredMember {
    fn from_entity(member: &ReplyMember) -> Self {
        Self {
            mid: member.mid,
            name: member.name.clone(),
            face: member.face.clone(),
            level: member.level,
            official_verify_type: member.official_verify_type,
            vip_type: member.vip_type,
            vip_status: member.vip_status,
            garb_pendant_image: member.pendant_image.clone().unwrap_or_default(),
            fans_medal_name: member
                .medal
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_default(),
            fans_medal_level: member.medal.as_ref().map(|m| m.level),
            fans_medal_color: member.medal.as_ref().map_or(0, |m| m.color),
            fans_medal_color_name: member.medal.as_ref().map_or(0, |m| m.color_name),
            is_senior_member: i64::from(member.is_senior),
        }
    }
}

impl StoredControl {
    fn from_entity(control: &ReplyControlInfo) -> Self {
        Self {
            action: control.action,
            is_up_top: control.is_up_top,
            show_translation: control.show_translation,
            up_reply: control.up_reply,
            is_note: control.is_note,
            is_note_v2: control.is_note_v2,
            location: control.location.clone().unwrap_or_default(),
            translation_switch: if control.can_translate {
                SHOW_TRANSLATION.to_owned()
            } else {
                String::new()
            },
            card_labels: control
                .card_labels
                .iter()
                .map(|text| StoredCardLabel {
                    text_content: text.clone(),
                })
                .collect(),
            vote_option: control.vote_option.as_ref().map(|o| StoredVoteOption {
                label_kind: StoredLabelKind(o.label_kind),
                desc: o.desc.clone(),
            }),
        }
    }
}

// ─────────────────────────── 对外的四个口子 ───────────────────────────

fn format_error(detail: impl Into<String>) -> DomainError {
    DomainError::new(ErrorKind::Protocol, detail)
}

/// 读一条本地保存的评论。
///
/// **两种格式都认**：先按新格式（[`StoredReply`] 的 JSON）解；解不开再按
/// 老格式（`ReplyInfo` 的 protobuf 字节）解一次。两者不会互相误判——JSON
/// 必须以 `{` 开头，而 protobuf 的第一个字节是 tag（`0x0a` 起步）。
///
/// 这条退路是**一次性迁移**：盒子里的老字节会在用户下次保存时被新格式
/// 覆盖。等到可以认定没有老数据了再删——在那之前删掉它等于清空用户的
/// 「我的评论」。
///
/// # Errors
/// 两种格式都解不开时返回协议错误。
pub fn decode_stored(bytes: &[u8]) -> Result<ReplyEntity, DomainError> {
    if let Ok(stored) = serde_json::from_slice::<StoredReply>(bytes) {
        return Ok(stored.into_entity());
    }
    let reply = pb::ReplyInfo::decode(bytes)
        .map_err(|_| format_error("本地保存的评论既不是 JSON 也不是 protobuf"))?;
    Ok(crate::grpc_reply::project_reply(&reply))
}

/// 写一条本地保存的评论。
///
/// # Errors
/// 序列化失败（实际上只可能是内存不足）。
pub fn encode_stored(entity: &ReplyEntity) -> Result<Vec<u8>, DomainError> {
    serde_json::to_vec(&StoredReply::from_entity(entity))
        .map_err(|e| format_error(format!("评论存档序列化失败：{e}")))
}

/// 导出成一个 JSON 数组（供「导出至剪贴板 / 本地文件」与配置归档使用）。
///
/// # Errors
/// 任何一条解不开就整体失败：导出一份少了几条的存档，比导不出来更糟。
pub fn export_json(items: &[Vec<u8>]) -> Result<String, DomainError> {
    let replies: Result<Vec<_>, _> = items
        .iter()
        .map(|bytes| decode_stored(bytes).map(|e| StoredReply::from_entity(&e)))
        .collect();
    serde_json::to_string_pretty(&replies?)
        .map_err(|e| format_error(format!("评论存档导出失败：{e}")))
}

/// 从 JSON 数组导入，返回 `(盒子的键, 字节)`。键是 `id` 的十进制串，与
/// 基线 `MyReply.json2ReplyProto` 一致。
///
/// # Errors
/// 不是 JSON 数组，或数组元素不是一条评论。
pub fn import_json(json: &str) -> Result<Vec<(String, Vec<u8>)>, DomainError> {
    let replies: Vec<StoredReply> = serde_json::from_str(json)
        .map_err(|e| format_error(format!("评论存档不是合法的 JSON 数组：{e}")))?;
    replies
        .into_iter()
        .map(|stored| {
            let key = stored.id.to_string();
            encode_stored(&stored.into_entity()).map(|bytes| (key, bytes))
        })
        .collect()
}

// ─────────────────────── REST 发评论返回的那条 ───────────────────────

/// `/x/v2/reply/add` 返回的 `data.reply`。
///
/// 这是**另一种形状**：REST 的评论 JSON（`uname`/`avatar`/`level_info`…），
/// 不是 proto3 JSON。基线 `RequestUtils.replyCast` 先把它改写成 pb 的字段
/// 名再 `mergeFromProto3Json`，这里直接一步到位。
///
/// 请求本身还在宿主侧（写操作按 ADR-013 单独迁），这里只接管**映射**：
/// 字段改名是协议适配，换掉宿主框架时不该再写一遍。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RestReply {
    #[serde(with = "int64")]
    rpid: i64,
    #[serde(with = "int64")]
    oid: i64,
    #[serde(rename = "type", with = "int64")]
    kind: i64,
    #[serde(with = "int64")]
    mid: i64,
    #[serde(with = "int64")]
    root: i64,
    #[serde(with = "int64")]
    parent: i64,
    #[serde(with = "int64")]
    dialog: i64,
    #[serde(with = "int64")]
    ctime: i64,
    #[serde(with = "int64")]
    like: i64,
    #[serde(with = "int64")]
    count: i64,
    content: RestContent,
    member: RestMember,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RestContent {
    message: String,
    /// REST 这里叫 `emote`（单数），值里的尺寸埋在 `meta.size`。
    emote: HashMap<String, RestEmote>,
    #[serde(rename = "at_name_to_mid")]
    at_name_to_mid: HashMap<String, StoredMid>,
    pictures: Vec<StoredPicture>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RestEmote {
    url: String,
    meta: RestEmoteMeta,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RestEmoteMeta {
    #[serde(with = "int64")]
    size: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RestMember {
    #[serde(with = "int64")]
    mid: i64,
    uname: String,
    avatar: String,
    level_info: RestLevelInfo,
    vip: RestVip,
    official_verify: RestOfficialVerify,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RestLevelInfo {
    #[serde(with = "int64")]
    current_level: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RestVip {
    #[serde(rename = "vipStatus", with = "int64")]
    status: i64,
    #[serde(rename = "vipType", with = "int64")]
    kind: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RestOfficialVerify {
    #[serde(rename = "type", with = "int64_opt")]
    kind: Option<i64>,
}

/// 把 `/x/v2/reply/add` 返回的那条评论变成领域实体。
///
/// # Errors
/// 响应不是预期形状的 JSON。
pub fn from_rest_reply(json: &str) -> Result<ReplyEntity, DomainError> {
    let rest: RestReply =
        serde_json::from_str(json).map_err(|e| format_error(format!("发表的评论解析失败：{e}")))?;
    Ok(ReplyEntity {
        id: rest.rpid,
        oid: rest.oid,
        kind: rest.kind,
        mid: rest.mid,
        root: rest.root,
        parent: rest.parent,
        dialog: rest.dialog,
        ctime: rest.ctime,
        like: rest.like,
        count: rest.count,
        content: ReplyContent {
            message: rest.content.message,
            emotes: rest
                .content
                .emote
                .into_iter()
                .map(|(key, emote)| {
                    (
                        key,
                        ReplyEmote {
                            size: emote.meta.size,
                            display_url: emote.url,
                            jump_url: None,
                        },
                    )
                })
                .collect(),
            at_name_to_mid: rest
                .content
                .at_name_to_mid
                .into_iter()
                .map(|(name, mid)| (name, mid.0))
                .collect(),
            pictures: rest
                .content
                .pictures
                .into_iter()
                .map(|p| ReplyPicture {
                    img_src: p.img_src,
                    img_width: p.img_width,
                    img_height: p.img_height,
                    img_size: p.img_size,
                })
                .collect(),
            ..ReplyContent::default()
        },
        member: ReplyMember {
            mid: rest.member.mid,
            name: rest.member.uname,
            face: rest.member.avatar,
            level: rest.member.level_info.current_level,
            is_senior: false,
            vip_status: rest.member.vip.status,
            vip_type: rest.member.vip.kind,
            official_verify_type: rest.member.official_verify.kind,
            pendant_image: None,
            medal: None,
        },
        ..ReplyEntity::default()
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        StoredReply, decode_stored, encode_stored, export_json, from_rest_reply, import_json,
    };
    use bili_grpc::bilibili::main::community::reply::v1 as pb;
    use prost::Message;

    #[test]
    fn a_round_trip_keeps_what_the_ui_shows() {
        let original = decode_stored(
            r#"{"id":"7","oid":"9","type":"1","mid":"3","ctime":"1700000000",
                 "like":"12","content":{"message":"你好",
                 "emotes":{"[doge]":{"size":"1","webpUrl":"w","gifUrl":"g","url":"u"}},
                 "atNameToMid":{"阿强":"42"},
                 "urls":{"av1":{"title":"稿件","clickReport":"{\"cvid\":1}"}}},
                 "member":{"mid":"3","name":"阿明","fansMedalLevel":"5"},
                 "replyControl":{"isUpTop":true,"translationSwitch":"TRANSLATION_SWITCH_SHOW_TRANSLATION"}}"#
                .as_bytes(),
        )
        .expect("应能读进来");

        assert_eq!(original.content.emotes["[doge]"].display_url, "w");
        assert_eq!(original.content.at_name_to_mid["阿强"], 42);
        assert!(
            original.content.urls["av1"].is_note,
            "按 clickReport 前缀认笔记"
        );
        assert_eq!(original.member.medal.as_ref().expect("有粉丝牌").level, 5);
        assert!(original.control.is_up_top);
        assert!(original.control.can_translate);

        let again =
            decode_stored(&encode_stored(&original).expect("应能写出去")).expect("应能再读回来");
        assert_eq!(again, original);
    }

    /// 老盒子里是 protobuf 字节，不能因为换格式就读不出来。
    #[test]
    fn the_old_protobuf_bytes_are_still_readable() {
        let bytes = pb::ReplyInfo {
            id: Some(7),
            mid: Some(3),
            content: Some(pb::Content {
                message: Some("旧数据".into()),
                ..pb::Content::default()
            }),
            ..pb::ReplyInfo::default()
        }
        .encode_to_vec();

        let entity = decode_stored(&bytes).expect("老格式要能读");
        assert_eq!(entity.id, 7);
        assert_eq!(entity.content.message, "旧数据");
    }

    /// 老导出文件里 int64 是字符串；手工编辑过的可能是数字。两种都收。
    #[test]
    fn int64_accepts_both_strings_and_numbers() {
        let items = import_json(r#"[{"id":7,"oid":"9"},{"id":"8"}]"#).expect("应能导入");
        assert_eq!(
            items
                .iter()
                .map(|(key, _)| key.as_str())
                .collect::<Vec<_>>(),
            vec!["7", "8"]
        );
        let first = decode_stored(&items[0].1).expect("应能读回来");
        assert_eq!(first.oid, 9);
    }

    #[test]
    fn export_writes_int64_as_strings() {
        let bytes = encode_stored(
            &StoredReply {
                id: 9_007_199_254_740_993,
                ..StoredReply::default()
            }
            .into_entity(),
        )
        .expect("应能写出去");
        let json = export_json(&[bytes]).expect("应能导出");
        assert!(
            json.contains("\"9007199254740993\""),
            "超过 53 位的 id 必须写成字符串，否则导出的文件在别处会掉精度：{json}"
        );
    }

    #[test]
    fn a_broken_entry_fails_the_whole_export() {
        let err = export_json(&[b"not a reply at all".to_vec()]).expect_err("应当失败");
        assert!(err.detail().contains("本地保存的评论"));
    }

    #[test]
    fn the_rest_shape_is_remapped_field_by_field() {
        let entity = from_rest_reply(
            r#"{"rpid":7,"oid":9,"type":1,"mid":3,"ctime":1700000000,
                "content":{"message":"发出去了",
                           "emote":{"[doge]":{"url":"u","meta":{"size":1}}},
                           "at_name_to_mid":{"阿强":42}},
                "member":{"mid":3,"uname":"阿明","avatar":"face.jpg",
                          "level_info":{"current_level":6},
                          "vip":{"vipStatus":1,"vipType":2},
                          "official_verify":{"type":0}}}"#,
        )
        .expect("应能解析");

        assert_eq!(entity.id, 7, "REST 的 rpid 就是实体的 id");
        assert_eq!(entity.member.name, "阿明");
        assert_eq!(entity.member.face, "face.jpg");
        assert_eq!(entity.member.level, 6);
        assert_eq!(entity.member.vip_status, 1);
        assert_eq!(entity.member.vip_type, 2);
        assert_eq!(
            entity.member.official_verify_type,
            Some(0),
            "0 是个人认证，不是「没有认证」"
        );
        assert_eq!(
            entity.content.emotes["[doge]"].size, 1,
            "尺寸埋在 meta.size"
        );
        assert_eq!(entity.content.at_name_to_mid["阿强"], 42);
    }
}
