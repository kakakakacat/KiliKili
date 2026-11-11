//! 评论 REST 只读路径的领域实体（S1 / issue #51）。
//!
//! ## 这些实体为什么比基线模型瘦
//!
//! 基线 `lib/models_new/reply/` 有 20 个文件，把服务端下发的每一棵装饰
//! 子树都照抄了一遍（勋章、头像挂件、大会员标签、等级、认证……）。但
//! 评论区 UI 走的是 gRPC（S2 / issue #52），消费的是 protobuf 类型；REST
//! 这条路径在迁移前**只有一个消费者**：`lib/utils/reply_utils.dart` 的
//! 「评论是否被 shadow ban」自检，它只读 `replies[].rpid`。
//!
//! 所以这里带的是**能认出一条评论**所需的东西：身份（rpid/oid/mid/root/
//! parent）、计数与状态、正文、发布者的 mid/昵称/头像，以及游标分页。
//! 装饰字段一概不映射——把没有消费者的结构换种语言再写一遍，只是把要删
//! 的代码搬了个家。真要显示 REST 评论时再按需加字段，解析层加字段是
//! 向后兼容的。
//!
//! ## 两条请求形态的响应形状不同
//!
//! 未登录走 `/x/v2/reply/main`，分页在 `cursor.pagination_reply.next_offset`；
//! 登录走 `/x/v2/reply`，分页在 `page.num/size/count`，**没有 `cursor`**。
//! 基线用同一个模型装两者（另一半字段为 null），这里保留这个事实：
//! [`ReplyList`] 的 `cursor` 与 `page` 都是 `Option`，各自只在对应形态下有值。

/// 一条评论。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct ReplyItem {
    pub rpid: Option<i64>,
    pub oid: Option<i64>,
    /// 评论区类型（1 = 视频稿件，17 = 动态……），与 `oid` 配对才有意义。
    pub kind: Option<i64>,
    pub mid: Option<i64>,
    /// 根评论的 rpid；0 表示它自己就是根评论。
    pub root: Option<i64>,
    pub parent: Option<i64>,
    pub dialog: Option<i64>,
    /// 楼中楼总数（含折叠）。
    pub count: Option<i64>,
    /// 楼中楼可见条数。
    pub rcount: Option<i64>,
    pub state: Option<i64>,
    pub fansgrade: Option<i64>,
    pub attr: Option<i64>,
    /// 发布时间，Unix 秒。
    pub ctime: Option<i64>,
    pub like: Option<i64>,
    /// 当前账号对这条评论的态度：0 无、1 点赞、2 点踩。
    pub action: Option<i64>,
    /// 仅自己可见。shadow ban 自检关心的就是它与「匿名能否翻到」的组合。
    pub invisible: Option<bool>,
    pub message: Option<String>,
    pub member: Option<ReplyMember>,
}

/// 评论的发布者。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct ReplyMember {
    /// 服务端在这里给的是**字符串**形式的 mid（`"2"`），这里解析成整数。
    /// 字符串形式的存在是为了绕开 JS 的 53 位精度，Rust/Dart 的 i64 没有
    /// 这个问题；解析失败时为 `None`，不会把整条评论作废。
    pub mid: Option<i64>,
    pub uname: Option<String>,
    pub avatar: Option<String>,
}

/// 游标式分页（未登录形态）。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct ReplyCursor {
    pub is_begin: Option<bool>,
    pub is_end: Option<bool>,
    pub prev: Option<i64>,
    pub next: Option<i64>,
    /// 下一页的 offset，原样回填进 `pagination_str`。
    pub next_offset: Option<String>,
    /// 排序模式：2 按时间，3 按热度。
    pub mode: Option<i64>,
    pub mode_text: Option<String>,
    /// 评论总数。
    pub all_count: Option<i64>,
    pub name: Option<String>,
}

/// 页码式分页（登录形态与楼中楼）。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct ReplyPage {
    pub num: Option<i64>,
    pub size: Option<i64>,
    pub count: Option<i64>,
}

/// 一页根评论。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct ReplyList {
    /// 未登录形态有值。
    pub cursor: Option<ReplyCursor>,
    /// 登录形态有值。
    pub page: Option<ReplyPage>,
    pub replies: Vec<ReplyItem>,
    /// 置顶评论。服务端给在 `top_replies` 或 `top.upper`，两处都可能为空。
    pub top_replies: Vec<ReplyItem>,
    pub top_upper: Option<ReplyItem>,
    /// 稿件 UP 主的 mid。服务端的 `upper` 在列表接口里只有这一个字段有值。
    pub upper_mid: Option<i64>,
}

/// 一页楼中楼。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct ReplyReplyList {
    pub page: Option<ReplyPage>,
    /// 这一楼的根评论。
    pub root: Option<ReplyItem>,
    pub upper_mid: Option<i64>,
    pub replies: Vec<ReplyItem>,
}

/// UP 主对自己稿件评论区的两个开关的状态。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReplyInteract {
    /// 精选评论。
    pub up_reply_selection: InteractStatus,
    /// 评论区本身（关闭/开启）。
    pub up_reply: InteractStatus,
}

/// 一个开关：当前状态，以及当前账号能否改它。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct InteractStatus {
    pub can_modify: bool,
    pub status: i64,
}

/// 表情包面板里的一组表情。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct EmotePackage {
    /// 组图标。
    pub url: Option<String>,
    /// 4 表示这一组是**文字**表情（如「[doge]」以文字形态插入）。
    pub kind: Option<i64>,
    pub emote: Vec<Emote>,
}

/// 一个表情。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct Emote {
    /// 插入到正文里的占位文本，如 `[doge]`。
    pub text: Option<String>,
    pub url: Option<String>,
    pub meta: Option<EmoteMeta>,
}

/// 表情的尺寸与别名。`size == 1` 是小尺寸，面板据此决定每行放几个。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct EmoteMeta {
    pub size: Option<i64>,
    pub alias: Option<String>,
}
