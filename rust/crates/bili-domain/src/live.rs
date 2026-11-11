//! 直播只读接口的领域实体（S11a 第一批 / issue #61）。
//!
//! 覆盖：分区列表（全部分区、某一级分区下的二级分区、收藏分区）、
//! 列表页（首页推荐、分区二级列表、关注的直播）、H5 房间信息。
//!
//! **不在本模块**：
//! - 取流（`liveRoomInfo` 的 `playurl_info`）——它缠着共享的
//!   `BaseQuality` 体系与播放器的画质选择器，留到 S11a 第二批；
//! - 弹幕长连接（S11b / issue #62）——阻塞在 ADR-012（#49）；
//! - 发弹幕、屏蔽词、点赞上报等写操作——归 ADR-013（#50）。
//!
//! ## 为什么卡片只有一个类型
//!
//! 基线里首页推荐的列表元素是 `LiveCardList`（外面套一层 `card_data`），
//! 分区二级列表的元素是 `CardLiveItem`（直接就是卡片），于是渲染处要写
//! `if (item is LiveCardList) item.cardData!.smallCardV1! else item` 这样
//! 的运行时类型判断。两者在协议上是同一张卡片，只是一个多包了两层。
//! 这里在解析时就把壳拆掉，两条接口都产出 [`LiveRoomCard`]。

use serde::Serialize;

/// 直播分区。
///
/// `id` 与 `parent_id` 在基线里是 `dynamic`——不同接口给的 JSON 类型不
/// 一致（见 `bili-api` 侧 `number_or_string` 的说明）。这里统一成 `i64`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveArea {
    pub id: i64,
    pub name: Option<String>,
    pub pic: Option<String>,
    pub parent_id: i64,
    pub parent_name: Option<String>,
}

/// 一级分区及其下的二级分区。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveAreaGroup {
    /// 基线缺失时取空串（`json['name'] ?? ''`），这里照搬：分组标题是
    /// TabBar 的文字，`null` 与空串在界面上没有区别。
    pub name: String,
    pub areas: Vec<LiveArea>,
}

/// 直播间卡片。首页推荐、分区二级列表、分区入口、关注区块共用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveRoomCard {
    pub roomid: Option<i64>,
    pub uid: Option<i64>,
    pub uname: Option<String>,
    pub face: Option<String>,
    pub cover: Option<String>,
    /// 基线是个 getter：`_systemCover ?? cover`，且 `system_cover` 为空串
    /// 时按缺失处理。FRB 的结构没有 getter，所以回落在解析时就做掉——
    /// 留给 Dart 会变成「每个调用点自己记得回落」，迟早漏一处。
    pub system_cover: Option<String>,
    pub title: Option<String>,
    pub area_name: Option<String>,
    pub area_v2_id: Option<i64>,
    pub area_v2_parent_id: Option<i64>,
    /// 基线 `watched_show.text_large`，形如「1.2万人看过」。
    pub watched_text: Option<String>,
    pub feedback: Vec<LiveFeedback>,
}

/// 卡片上的「不感兴趣」反馈入口。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveFeedback {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// 响应里的键是 `type`，Rust 的关键字，故改名。
    pub kind: Option<String>,
    pub reasons: Vec<LiveFeedbackReason>,
}

/// 反馈里的一条理由。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveFeedbackReason {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub id_type: Option<String>,
    pub reason_id: Option<i64>,
}

/// 直播首页推荐的一屏。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveIndexPage {
    /// `card_type == "small_card_v1"` 的那些卡片，即正文列表。
    pub cards: Vec<LiveRoomCard>,
    /// 基线是 `has_more == 0` 判结束，这里直接给布尔。
    pub has_more: bool,
    /// `card_type == "my_idol_v1"`。未登录或没有关注时为 `None`。
    pub follow: Option<LiveFollowSection>,
    /// `card_type == "area_entrance_v3"`，即顶部那排分区入口。
    ///
    /// 服务端不下发时回落到 [`default_area_entrance`]，与基线的
    /// `areaItem ??= LiveCardList.kAreaEntrance` 一致——这排入口是整页
    /// 的导航骨架，缺了页面就只剩一个「推荐」。
    pub area_entrance: Vec<LiveRoomCard>,
}

/// 首页的「我的关注」区块。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveFollowSection {
    /// 正在直播的关注数。基线缺失时按 0 算。
    pub total_count: i64,
    pub items: Vec<LiveRoomCard>,
}

/// 分区二级列表的一屏。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveSecondPage {
    /// 该分区的总房间数，用于判断是否已到末页。
    pub count: Option<i64>,
    pub cards: Vec<LiveRoomCard>,
    pub tags: Vec<LiveSecondTag>,
}

/// 分区二级列表的排序标签。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveSecondTag {
    pub name: Option<String>,
    pub sort_type: Option<String>,
}

/// 「我的关注」整页（与首页的关注区块不是同一条接口）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveFollowPage {
    pub title: Option<String>,
    pub page_size: Option<i64>,
    pub total_page: Option<i64>,
    /// **只含正在直播的**。基线在 `fromJson` 里就按
    /// `live_status == 1` 过滤掉了未开播的，这里照搬：分页的
    /// `page_size` 是服务端按未过滤的条数给的，过滤放到 UI 会让
    /// 「这一页只有两条」看起来像加载失败。
    pub items: Vec<LiveFollowItem>,
    pub count: Option<i64>,
    pub live_count: Option<i64>,
}

/// 关注列表里的一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveFollowItem {
    pub roomid: Option<i64>,
    pub uname: Option<String>,
    pub title: Option<String>,
    pub area_name: Option<String>,
    /// 形如「1.2万」，基线 `text_small`。
    pub text_small: Option<String>,
    pub room_cover: Option<String>,
}

/// H5 房间信息：房间 + 主播 + 看过人数。
///
/// 基线分三层（`room_info` / `anchor_info.base_info` / `watched_show`），
/// 调用点因此写出 `roomInfoH5.anchorInfo!.baseInfo!.face` 这样连着两个
/// `!` 的取值。协议上这三层没有各自的含义，摊平成一层后每个字段各自
/// 可空，两处强制解包随之消失。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveRoomH5 {
    pub uid: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub app_background: Option<String>,
    pub anchor_name: Option<String>,
    pub anchor_face: Option<String>,
    /// `watched_show.text_large`。
    pub watched_text: Option<String>,
}

/// 分区入口的兜底数据。
///
/// 逐条复制自基线 `CardDataItem.kAreaEntrance`（`card_data_item.dart`）。
/// 这是一份**客户端内置的导航表**而非协议数据，之所以跟着接口一起迁：
/// 它只在 `area_entrance_v3` 缺失时才用得上，判断缺失的地方在这里。
///
/// `area_v2_id == 0` 表示「该一级分区下不限二级分区」，不是缺省值。
#[must_use]
pub fn default_area_entrance() -> Vec<LiveRoomCard> {
    const ENTRIES: [(i64, i64, &str); 14] = [
        (0, 0, "人气"),
        (0, 9, "虚拟主播"),
        (0, 14, "聊天室"),
        (145, 1, "颜值"),
        (0, 1, "娱乐"),
        (86, 2, "英雄联盟"),
        (35, 3, "王者荣耀"),
        (0, 5, "电台"),
        (89, 2, "CS2"),
        (0, 15, "互动玩法"),
        (0, 6, "单机游戏"),
        (21, 1, "视频唱见"),
        (301_000, 301, "帮我玩"),
        (1013, 1, "团播"),
    ];

    ENTRIES
        .iter()
        .map(|&(area_id, parent_id, title)| LiveRoomCard {
            roomid: None,
            uid: None,
            uname: None,
            face: None,
            cover: None,
            system_cover: None,
            title: Some(title.to_owned()),
            area_name: None,
            area_v2_id: Some(area_id),
            area_v2_parent_id: Some(parent_id),
            watched_text: None,
            feedback: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_area_entrance_matches_the_baseline_table() {
        let entries = default_area_entrance();
        assert_eq!(entries.len(), 14, "基线内置 14 个分区入口");

        // 第一个是「人气」，它的两个 id 都是 0——既是列表头也是
        // 「不选任何分区」的那一项，写错会让首页默认落到别的分区。
        assert_eq!(entries[0].title.as_deref(), Some("人气"));
        assert_eq!(entries[0].area_v2_id, Some(0));
        assert_eq!(entries[0].area_v2_parent_id, Some(0));

        // 「颜值」是少数几个二级 id 非 0 的，拿它钉住「0 不是缺省值」。
        let beauty = entries
            .iter()
            .find(|e| e.title.as_deref() == Some("颜值"))
            .expect("基线表里有颜值");
        assert_eq!(beauty.area_v2_id, Some(145));
        assert_eq!(beauty.area_v2_parent_id, Some(1));
    }
}
