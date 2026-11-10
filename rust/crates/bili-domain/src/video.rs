//! 视频详情实体。
//!
//! 这是**领域实体**，不是接口响应的镜像：字段名、可空性与类型由业务
//! 需要决定，而非由服务端 JSON 决定。接口字段到此处的映射（含历史遗留
//! 的怪异之处）全部关在 `bili-api` 的 DTO 层，协议变更不应传导到这里
//! （RFC-0001 §5.3「通过协议适配层修复，避免污染 Domain」）。
//!
//! ## 为什么只派生 `Serialize`
//!
//! fixture 对照需要把实体**规范化地**序列化出来与 Dart 基线逐字段比对
//! （`bili-api/tests/video_detail_golden.rs`）。但刻意不派生
//! `Deserialize`：一旦实体能从 JSON 直接反序列化，就会有人跳过 DTO 层
//! 直接解析接口响应，协议的脏东西便会渗进领域层。序列化是单向的出口，
//! 不构成入口。
//!
//! ## 可空性
//!
//! 大量字段是 `Option`，这并非偷懒：基线 Dart 模型里它们都是可空的，
//! 服务端也确实会在不同视频形态（番剧跳转、充电专属、互动视频）下省略
//! 字段。此处收紧成非空会让解析在真实数据上失败，而失败的形式是整个
//! 详情页打不开——比字段缺失严重得多。

use serde::Serialize;

// 这些实体刻意**不加** `#[non_exhaustive]`：它们由同一 workspace 内的
// `bili-api` 逐字段构造，加了只会把构造改成「先 Default 再赋值」的样板，
// 反而让「新增字段忘了映射」从编译错误退化成静默的默认值。

/// 视频详情。对应接口 `/x/web-interface/wbi/view` 的 `data`。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoDetail {
    pub bvid: Option<String>,
    pub aid: Option<i64>,
    /// 分 P 数量。
    pub videos: Option<i64>,
    pub copyright: Option<i64>,
    /// 封面地址。
    pub pic: Option<String>,
    pub title: Option<String>,
    /// 发布时间（Unix 秒）。
    pub pubdate: Option<i64>,
    /// 投稿时间（Unix 秒）。
    pub ctime: Option<i64>,
    pub desc: Option<String>,
    pub desc_v2: Option<Vec<DescSegment>>,
    /// 时长（秒）。
    pub duration: Option<i64>,
    /// 是否互动视频（`rights.is_stein_gate`）。
    ///
    /// 基线把整个 `rights` 对象收窄成了这一个字段，此处保持一致：
    /// 其余权限位当前没有消费方，提前引入等于凭空承诺兼容性。
    pub is_stein_gate: Option<i64>,
    pub owner: Option<UserBrief>,
    pub stat: Option<VideoStat>,
    /// 争议提示文案（`argue_info.argue_msg`）。
    pub argue_msg: Option<String>,
    pub cid: Option<i64>,
    pub dimension: Option<Dimension>,
    pub season_id: Option<i64>,
    pub is_upower_exclusive: Option<bool>,
    pub pages: Option<Vec<VideoPart>>,
    pub ugc_season: Option<UgcSeason>,
    pub staff: Option<Vec<Staff>>,
    /// 番剧跳转地址。空串已在映射时归一为 `None`——基线对空串与缺失
    /// 一视同仁，若此处保留空串，调用方会据此跳转到一个空地址。
    pub redirect_url: Option<String>,
}

/// 简介的结构化片段（含 @ 用户、BV 号等可点击元素）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DescSegment {
    pub raw_text: Option<String>,
    pub r#type: Option<i64>,
    pub biz_id: Option<i64>,
}

/// UP 主 / 作者的最小信息。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserBrief {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
}

/// 视频计数。
///
/// `favorite` / `like` / `coin` 非空：基线对这三个字段用了 `?? 0`，
/// UI 直接参与数值展示与增减，缺失按 0 处理是既定行为。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoStat {
    pub view: Option<i64>,
    pub danmaku: Option<i64>,
    pub reply: Option<i64>,
    pub favorite: i64,
    pub coin: i64,
    pub share: Option<i64>,
    pub like: i64,
}

/// 画面尺寸。**已消解 `rotate`**：竖屏视频服务端返回横向尺寸加
/// `rotate=1`，映射时已交换，调用方拿到的恒为实际显示尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Dimension {
    pub width: Option<i64>,
    pub height: Option<i64>,
}

impl Dimension {
    /// 是否竖屏。尺寸不全时按横屏处理（与基线一致）。
    #[must_use]
    pub const fn is_vertical(&self) -> bool {
        match (self.width, self.height) {
            (Some(w), Some(h)) => h > w,
            _ => false,
        }
    }
}

/// 分 P。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoPart {
    pub cid: Option<i64>,
    /// 分 P 序号，从 1 开始。
    pub page: Option<i64>,
    pub from: Option<String>,
    /// 分 P 标题。
    pub part: Option<String>,
    pub duration: Option<i64>,
    pub vid: Option<String>,
    pub dimension: Option<Dimension>,
    pub ctime: Option<i64>,
    pub first_frame: Option<String>,
    pub badge: Option<String>,
}

/// 合集。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UgcSeason {
    pub id: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub mid: Option<i64>,
    pub sections: Option<Vec<SeasonSection>>,
}

/// 合集中的一个小节。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeasonSection {
    pub season_id: Option<i64>,
    pub id: Option<i64>,
    pub title: Option<String>,
    pub r#type: Option<i64>,
    pub episodes: Option<Vec<SeasonEpisode>>,
}

/// 合集中的一集。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeasonEpisode {
    pub season_id: Option<i64>,
    pub section_id: Option<i64>,
    pub id: Option<i64>,
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub cid: Option<i64>,
    pub title: Option<String>,
    pub attribute: Option<i64>,
    pub badge: Option<String>,
    pub arc: Option<EpisodeArc>,
    pub page: Option<VideoPart>,
    pub pages: Option<Vec<VideoPart>>,
}

/// 一集对应稿件的概要。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EpisodeArc {
    pub aid: Option<i64>,
    pub pic: Option<String>,
    pub title: Option<String>,
    pub pubdate: Option<i64>,
    pub duration: Option<i64>,
    pub author: Option<UserBrief>,
    pub stat: Option<VideoStat>,
    pub dimension: Option<Dimension>,
}

/// 联合投稿成员。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Staff {
    /// 基线此字段为 `dynamic`（服务端混用过数字与字符串），映射时统一
    /// 收敛成整数：UI 只拿它跳转空间页，保留两种类型只会把判断推给
    /// 每一个调用点。
    pub mid: Option<i64>,
    /// 身份说明，如「导演」「音乐」。
    pub title: Option<String>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub vip_type: Option<i64>,
    pub vip_status: i64,
    pub official_type: Option<i64>,
    pub official_desc: Option<String>,
}

/// 视频形态。决定走哪个取流接口，以及部分请求参数是否存在。
///
/// 对应基线 `lib/models/common/video/video_type.dart`。接口路径与
/// `biz_type` 属于协议细节，留在 `bili-api`；这里只保留「是哪一类视频」
/// 这个领域概念本身。
///
/// 数值与基线的 `type` 字段一致——它会出现在历史记录等接口的参数里，
/// 换一套编号会让两边的持久化数据对不上。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum VideoType {
    /// 用户投稿。
    Ugc,
    /// 番剧 / 影视等 OGV。
    Pgc,
    /// 课程。
    Pugv,
}

impl VideoType {
    /// 基线的数值编号。
    #[must_use]
    pub const fn code(self) -> i64 {
        match self {
            Self::Ugc => 3,
            Self::Pgc => 4,
            Self::Pugv => 10,
        }
    }

    /// 评论区类型编号（基线 `replyType`）。
    #[must_use]
    pub const fn reply_type(self) -> i64 {
        match self {
            Self::Ugc | Self::Pgc => 1,
            Self::Pugv => 33,
        }
    }
}
