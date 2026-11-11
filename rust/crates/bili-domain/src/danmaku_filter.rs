//! 弹幕屏蔽规则（云端同步的那一份）的领域实体（S15 / issue #66）。
//!
//! 对应基线 `lib/models/user/danmaku_block.dart`。
//!
//! ## 分类在这一层完成
//!
//! 服务端给的是一个平列表，每条带 `type`：0 关键词、1 正则、2 用户。
//! 基线在模型的构造函数里就按 type 分成三组，并**丢掉未知 type**。那是
//! 协议知识（哪个数字是什么意思），按 ADR-011 决策二留在 Rust；界面只
//! 负责把三组画成三个 Tab。
//!
//! 正则的语义差异（`\d`、`\b`、`{n,m}` 在两端行为不同）由
//! [`crate::regex_compat`] 判定，不在这里重复。

use serde::Serialize;

/// 云端弹幕屏蔽规则的全量快照。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct DanmakuFilter {
    /// 关键词（type 0）。
    pub keywords: Vec<FilterRule>,
    /// 正则（type 1）。
    pub patterns: Vec<FilterRule>,
    /// 用户（type 2），`filter` 是 mid。
    pub users: Vec<FilterRule>,
    /// 服务端给的提示文案。**空串已归一成 `None`**（基线
    /// `json['toast'] == '' ? null : …`），界面据此决定弹不弹。
    pub toast: Option<String>,
    pub valid: Option<i64>,
    /// 规则版本号，增量同步用。
    pub ver: Option<i64>,
}

/// 一条规则。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilterRule {
    pub id: i64,
    /// 0 关键词、1 正则、2 用户。留着是因为写操作（新增/删除，等 #50）
    /// 要按它回填，界面也用它决定条目图标。
    pub kind: i64,
    pub filter: String,
}
