//! 评论过滤规则的 Dart 导出（S2 / issue #52）。
//!
//! 规则在 `bili_domain::reply_filter`，本模块只做类型转换与批处理
//! （RFC-0001 §3：桥接层不承载业务逻辑）。
//!
//! ## 为什么是「一次一页」而不是「一次一条」
//!
//! 基线是 `replies.removeWhere(needRemoveGrpc)`，每条评论调一次。照搬成
//! 每条一次跨 FFI，一页就是几十到上百次调用，正是 RFC §2.2 要避免的
//! 形状。这里改成一次递一页、返回等长的 `Vec<bool>`，跨 FFI 次数从
//! 「每条一次」降到「每个响应一次」。
//!
//! 顺带，正则也只编译一次：基线把 `RegExp` 存在静态字段里复用，而跨 FFI
//! 只能递模式串，逐条调用就意味着逐条重编。
//!
//! ## 为什么是 `#[frb(sync)]`
//!
//! 基线的 `removeWhere` 是同步的，调用点在 `ReplyGrpc.mainList` 等函数
//! 拿到响应之后、返回给控制器之前。改成 Future 会让四个调用点都多一次
//! 异步跳转，而评论列表正处在用户已经在等的路径上。一页几十条字符串
//! 匹配是微秒到毫秒级，同步是合适的。

use bili_domain::reply_filter;
use flutter_rust_bridge::frb;

/// 过滤规则的取值，来自宿主的 `Pref` 与 `GlobalData().blackMids`。
///
/// 真相源留在 Dart，按请求传入，Rust 不缓存——与 Cookie 同理。
#[derive(Debug, Clone)]
pub struct ReplyFilterSettings {
    pub blocked_mids: Vec<i64>,
    pub anti_goods: bool,
    pub block_at_only: bool,
    /// `None` = 不启用，对应基线 `Pref.blockReplyAtCount` 的 null。
    pub block_at_count: Option<i64>,
    /// 关键词正则源串。空串=不启用。
    pub ban_word: String,
}

/// 一条 URL 的带货标记；宿主只递 `ReplyInfo.content.urls` 里**带 `extra`**
/// 的那些条目。
#[derive(Debug, Clone)]
pub struct ReplyUrlExtra {
    pub goods_cm_control: i64,
    pub has_goods_item_id: bool,
    pub has_goods_prefetched_cache: bool,
}

/// 规则要读的那几个字段，由宿主从 `ReplyInfo` 投影而来。
#[derive(Debug, Clone)]
pub struct ReplyFilterItem {
    pub mid: i64,
    pub message: String,
    /// `content.atNameToMid` 的键。
    pub at_names: Vec<String>,
    pub has_emotes: bool,
    pub has_pictures: bool,
    pub url_extras: Vec<ReplyUrlExtra>,
    pub root: i64,
    pub parent: i64,
}

/// 判定结果。`data` 与入参 `items` **等长且同序**，`true` = 该条移除。
///
/// `data` 为空即失败（ADR-011：失败是返回值，不是异常）。唯一的失败原因
/// 是关键词正则不被支持，此时 `error_kind` 是 `Unsupported`。
#[derive(Debug, Clone)]
pub struct ReplyFilterMaskResult {
    pub data: Option<Vec<bool>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

impl ReplyFilterSettings {
    fn into_domain(self) -> reply_filter::ReplyFilterSettings {
        reply_filter::ReplyFilterSettings {
            blocked_mids: self.blocked_mids,
            anti_goods: self.anti_goods,
            block_at_only: self.block_at_only,
            block_at_count: self.block_at_count,
            ban_word: self.ban_word,
        }
    }
}

impl ReplyFilterItem {
    fn into_domain(self) -> reply_filter::ReplyFilterItem {
        reply_filter::ReplyFilterItem {
            mid: self.mid,
            message: self.message,
            at_names: self.at_names,
            has_emotes: self.has_emotes,
            has_pictures: self.has_pictures,
            url_extras: self
                .url_extras
                .into_iter()
                .map(|e| reply_filter::ReplyUrlExtra {
                    goods_cm_control: e.goods_cm_control,
                    has_goods_item_id: e.has_goods_item_id,
                    has_goods_prefetched_cache: e.has_goods_prefetched_cache,
                })
                .collect(),
            root: self.root,
            parent: self.parent,
        }
    }
}

/// 一页评论的移除判定。返回与 `items` 等长同序的 `Vec<bool>`。
#[frb(sync)]
#[must_use]
pub fn reply_filter_mask(
    settings: ReplyFilterSettings,
    items: Vec<ReplyFilterItem>,
) -> ReplyFilterMaskResult {
    let filter = match reply_filter::ReplyFilter::compile(&settings.into_domain()) {
        Ok(filter) => filter,
        Err(e) => {
            return ReplyFilterMaskResult {
                data: None,
                error_kind: Some(e.kind().to_string()),
                error_detail: Some(e.detail().to_owned()),
                error_code: e.code(),
            };
        }
    };

    ReplyFilterMaskResult {
        data: Some(
            items
                .into_iter()
                .map(|item| filter.removes(&item.into_domain()))
                .collect(),
        ),
        error_kind: None,
        error_detail: None,
        error_code: None,
    }
}
