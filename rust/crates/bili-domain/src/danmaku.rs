//! 弹幕实体。
//!
//! 对应 `bilibili.community.service.dm.v1.DmSegMobileReply`：一个分段
//! （6 分钟）内的全部弹幕。与视频详情不同，这里的实体**贴着协议的形状**，
//! 原因是协议本身就是弹幕的领域模型——`mode`、`pool`、`attr` 这些字段
//! 没有「更业务」的表达方式，基线（`lib/pages/danmaku/view.dart`）也是
//! 直接按数值分支的。强行翻译只会多一层需要同步的映射表。
//!
//! ## 字段取舍
//!
//! 解码**服务端会下发的全部标量字段**（protobuf 字段号 1–27），而不只是
//! 当前 UI 用到的那几个。理由是 shadow mode 比对的覆盖面取决于实体的
//! 宽度：没解的字段，协议改了也不会有人发现；而多解几个标量字段的代价
//! 只是几行映射。
//!
//! 字段号 100（`count`）与 101（`is_self`）**不在**此处：它们是上游手工
//! 加进生成代码的客户端字段（`lib/grpc/bilibili/community/service/dm/v1.pb.dart`，
//! 上游提交 `47e94b33b`），服务端从不下发，由客户端在合并同内容弹幕、
//! 标记自己发的弹幕时写入。那是弹幕引擎（P1-05 的下一步）的产物，不是
//! 解码的产物。

use serde::Serialize;

/// 一个弹幕分段的解码结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DanmakuSegment {
    pub elems: Vec<DanmakuElem>,
    /// 弹幕关闭状态。基线在 `state == 1` 时提示「该视频弹幕已关闭」。
    pub state: i32,
}

/// 一条弹幕。
///
/// `Default` 即 proto3 的零值：未出现的标量字段取零值，解码器从这里起步，
/// 逐个覆盖出现过的字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct DanmakuElem {
    /// 弹幕 id。
    pub id: i64,
    /// 出现时间，视频内毫秒偏移。
    pub progress: i32,
    /// 1 滚动 / 4 底部 / 5 顶部 / 7 高级（内容为 JSON）/ 9 BAS。
    pub mode: i32,
    pub fontsize: i32,
    /// 十进制 RGB，不含 alpha。
    pub color: u32,
    /// 发送者 mid 的哈希。**不是** mid：服务端不下发真实 mid，
    /// 客户端只能用它和自己的哈希比对来认出自己发的弹幕。
    pub mid_hash: String,
    pub content: String,
    /// 发送时间（Unix 秒）。
    pub ctime: i64,
    /// 屏蔽等级，0–10。基线按用户设置的阈值过滤。
    pub weight: i32,
    pub action: String,
    /// 0 普通池 / 1 字幕池 / 2 特殊池。
    pub pool: i32,
    /// 字符串形式的 id。服务端对大 id 同时下发两种形式。
    pub id_str: String,
    /// 位标记（保护、直播、高赞等）。
    pub attr: i32,
    pub like_count: i64,
    pub animation: String,
    /// 附加信息，服务端下发的 JSON 字符串。基线不解析。
    pub extra: String,
    pub colorful: DanmakuColorful,
    /// protobuf 里的 `type` 字段。
    pub dm_type: i32,
    /// 所属 cid。
    pub oid: i64,
    pub dm_from: DanmakuFrom,
}

/// 彩色弹幕类型（protobuf 枚举 `DmColorfulType`）。
///
/// 这是**封闭**枚举：未知取值归为 [`DanmakuColorful::None`]，而不是保留
/// 原始数值。这一点必须与基线一致——protobuf.dart 对识别不了的枚举值
/// 会把它丢进 `unknownFields`，getter 返回默认值 0。若此处保留原值，
/// 同一条弹幕在两种实现下就会得出不同的渲染结果（基线按默认值渲染成
/// 普通弹幕，Rust 却认为它是某种彩色弹幕），shadow mode 也会报出一堆
/// 并非解码错误的差异。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(into = "i32")]
pub enum DanmakuColorful {
    #[default]
    None,
    /// 大会员渐变彩色。
    VipGradualColor,
}

impl DanmakuColorful {
    /// 由线上数值构造；未知值退回 [`Self::None`]（见类型文档）。
    #[must_use]
    pub const fn from_wire(value: i32) -> Self {
        match value {
            60001 => Self::VipGradualColor,
            _ => Self::None,
        }
    }
}

impl From<DanmakuColorful> for i32 {
    fn from(value: DanmakuColorful) -> Self {
        match value {
            DanmakuColorful::None => 0,
            DanmakuColorful::VipGradualColor => 60001,
        }
    }
}

/// 弹幕来源（protobuf 枚举 `DmFromType`）。未知取值的处理同
/// [`DanmakuColorful`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(into = "i32")]
pub enum DanmakuFrom {
    #[default]
    Unknown,
    Normal,
    Cmd,
    Live,
}

impl DanmakuFrom {
    /// 由线上数值构造；未知值退回 [`Self::Unknown`]。
    #[must_use]
    pub const fn from_wire(value: i32) -> Self {
        match value {
            1 => Self::Normal,
            2 => Self::Cmd,
            3 => Self::Live,
            _ => Self::Unknown,
        }
    }
}

impl From<DanmakuFrom> for i32 {
    fn from(value: DanmakuFrom) -> Self {
        match value {
            DanmakuFrom::Unknown => 0,
            DanmakuFrom::Normal => 1,
            DanmakuFrom::Cmd => 2,
            DanmakuFrom::Live => 3,
        }
    }
}

/// 分段长度：6 分钟，单位毫秒。服务端按此切分弹幕。
pub const SEGMENT_LENGTH_MS: i64 = 6 * 60 * 1000;

/// 某个播放进度（毫秒）落在第几个分段，从 0 开始。
///
/// 与基线 `DmUtils.calcSegment` 一致；请求时需要 `+1`，服务端的
/// `segment_index` 从 1 开始。
#[must_use]
pub const fn segment_index(progress_ms: i64) -> i64 {
    progress_ms / SEGMENT_LENGTH_MS
}
