//! 动态业务实体。协议消息只在 `bili-api` 内部出现。

use serde::{Deserialize, Serialize};

/// 首页动态红点的有效更新数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UnreadCount {
    pub count: i64,
}

/// 动态点赞列表中的作者信息。只保留调用点消费的字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DynamicAuthor {
    pub mid: i64,
    pub name: String,
    pub face: String,
    pub sign: String,
    pub level: i32,
    pub is_senior_member: bool,
    pub pendant_image: Option<String>,
    pub vip_status: Option<i32>,
    pub vip_type: Option<i32>,
    pub official_type: Option<i32>,
}

/// 点赞列表与下一页游标。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LikeListPage {
    pub items: Vec<DynamicAuthor>,
    pub has_more: bool,
    pub total_count: i64,
}

/// 预约卡片在创建页展示的数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReserveInfo {
    pub id: i64,
    pub title: String,
    pub live_plan_start_time: i64,
}

/// 动态预约按钮操作后的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReserveAction {
    pub final_btn_status: Option<i32>,
    pub reserve_update: Option<i32>,
    pub desc_update: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteOption {
    pub opt_idx: Option<i32>,
    pub opt_desc: Option<String>,
    #[serde(default)]
    pub cnt: i64,
    pub img_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteInfo {
    pub choice_cnt: Option<i32>,
    pub default_share: Option<i32>,
    pub desc: Option<String>,
    pub end_time: Option<i64>,
    pub status: Option<i32>,
    pub uid: Option<i64>,
    pub vote_id: Option<i64>,
    #[serde(default)]
    pub join_num: i64,
    pub title: Option<String>,
    pub ctime: Option<i64>,
    pub my_votes: Option<Vec<i32>>,
    #[serde(default)]
    pub options: Vec<VoteOption>,
    pub options_cnt: Option<i32>,
    pub voter_level: Option<i32>,
    pub face: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub vote_type: Option<i32>,
    pub vote_publisher: Option<i64>,
    pub duration: Option<i64>,
    pub only_fans_level: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolloweeVote {
    pub uid: i64,
    pub name: String,
    pub face: String,
    pub votes: Vec<i32>,
    pub ctime: i64,
}
