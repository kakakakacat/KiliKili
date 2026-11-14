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
