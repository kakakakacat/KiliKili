//! 评论区的过滤规则（S2 / issue #52）。
//!
//! 对应基线 `lib/grpc/reply.dart` 里 `ReplyGrpc` 的四条规则：
//! `_needRemoveGoodGrpc`（带货）、`_blockAtOnlyReply`（纯 @ 评论）、
//! `_blockReplyAtCount`（@ 数量）与 `banWordForReply` 正则，外加
//! `GlobalData().blackMids` 的拉黑判定。
//!
//! ## 为什么在领域层
//!
//! 它们是「协议没规定、但两端必须一致」的决定：换一套 UI 之后这些判定
//! 仍然成立，而把它们留在 Dart 就意味着换宿主时要重写一遍（AGENTS.md
//! §2 的判据）。规则的**取值**来自用户设置，按请求传入
//! （[`ReplyFilterSettings`]），Rust 不存——与 Cookie 同理，真相源留在
//! Dart。形状照 [`crate::video_feed::FeedFilter`]，那是同一类规则的先例。
//!
//! ## 输入为什么不是 protobuf 消息
//!
//! [`ReplyFilterItem`] 是一个**投影**，只带规则真正要读的那几个字段。
//! #52 落地时它由宿主从 `ReplyInfo` 里挑出来递过来；取数迁过来之后
//! （#176）投影在 Rust 自己做（`bili_api::grpc_reply::filter_input`），
//! 这个结构一个字段都没动——规则读什么不取决于谁解的码。
//!
//! ## UTF-16 与 UTF-8：为什么逐字节扫描与基线等价
//!
//! 基线的 `_blockAtOnlyReply` 用 `codeUnitAt` 逐 **UTF-16 码元**走，这里
//! 逐 **UTF-8 字节**走。两者在本算法上结果相同，理由是扫描只在四个
//! ASCII 字符上做判断（空格、换行、`@`、`:`）：UTF-8 的多字节序列里不会
//! 出现 ASCII 字节，UTF-16 的代理对里也不会出现 ASCII 码元，所以**切分
//! 边界完全一致**，被切出来的名字也就逐字符相同。
//!
//! 这一条必须写下来：`{n,m}`、`\d` 这类按长度计数的构造在两端确实不同
//! （见 [`crate::regex_compat`]），容易让人以为这里也有同样的坑。区别在于
//! 本算法不数长度，只找分隔符。

use std::collections::HashSet;

use crate::error::DomainError;
use crate::regex_compat::compile_user_pattern;

/// 带货评论的链接前缀。基线 `Constants.goodsUrlPrefix`。
///
/// 它是**判定规则的一部分**（「长这样的链接算带货」），不是界面文案，
/// 所以跟着规则走到 Rust，宿主侧不再保留第二份。
pub const GOODS_URL_PREFIX: &str = "https://gaoneng.bilibili.com/tetris";

/// 过滤规则的取值，来自用户设置（基线 `Pref` + `GlobalData().blackMids`）。
#[derive(Debug, Clone, Default)]
pub struct ReplyFilterSettings {
    /// 拉黑的 UP 主 mid（基线 `GlobalData().blackMids`）。
    pub blocked_mids: Vec<i64>,
    /// 是否屏蔽带货评论（基线 `Pref.antiGoodsReply`）。
    pub anti_goods: bool,
    /// 是否屏蔽「只包含 @」的评论（基线 `Pref.blockAtOnlyReply`）。
    pub block_at_only: bool,
    /// @ 数量达到此值即屏蔽（基线 `Pref.blockReplyAtCount`）。
    /// `None` = 不启用，与基线的 `null` 同义。
    pub block_at_count: Option<i64>,
    /// 评论关键词正则源串（基线 `Pref.banWordForReply`）。空串=不启用。
    pub ban_word: String,
    /// 稿件简介。与之逐字相同（去掉首尾空白后）的根评论会被移除，对应基线
    /// `ReplyController._removeIntro` + `Pref.filterIntro`。
    ///
    /// `None` / 空串 = 不启用，与 [`Self::ban_word`] 同一个约定。开关与取值
    /// 在基线里是两个东西（`_filterIntro` 与 `intro`），合成一个的理由是
    /// 两者只有「一起有效」一种组合，分开表达会多出一个无意义的状态。
    pub intro: Option<String>,
}

/// 一条 URL 的带货标记，对应基线 `ReplyInfo.content.urls` 里**带 `extra`
/// 的**那些条目。
///
/// 没有 `extra` 的条目基线直接判 false（`url.hasExtra() && ...`），所以
/// 宿主只递带 `extra` 的那些，这里不必再表达「存在与否」。
#[derive(Debug, Clone, Default)]
pub struct ReplyUrlExtra {
    /// 基线判的是 `== Int64.ONE`，不是「非零」。照抄。
    pub goods_cm_control: i64,
    /// 基线 `extra.hasGoodsItemId()`：字段**存在**即算，不看值。
    pub has_goods_item_id: bool,
    /// 基线 `extra.hasGoodsPrefetchedCache()`，同上。
    pub has_goods_prefetched_cache: bool,
}

/// 规则真正要读的那几个字段，从 `ReplyInfo` 投影而来。
#[derive(Debug, Clone, Default)]
pub struct ReplyFilterItem {
    pub mid: i64,
    /// `content.message`。
    pub message: String,
    /// `content.atNameToMid` 的**键**。规则只用到「有哪些名字」与「有
    /// 几个」，对应的 mid 用不上，不必跨过来。
    pub at_names: Vec<String>,
    /// `content.emotes` 非空。
    pub has_emotes: bool,
    /// `content.pictures` 非空。
    pub has_pictures: bool,
    /// `content.urls` 里带 `extra` 的那些条目的 `extra`。
    pub url_extras: Vec<ReplyUrlExtra>,
    pub root: i64,
    pub parent: i64,
}

/// 编译好的过滤规则。
#[derive(Debug)]
pub struct ReplyFilter {
    blocked_mids: HashSet<i64>,
    anti_goods: bool,
    block_at_only: bool,
    block_at_count: Option<i64>,
    ban_word: Option<regex::Regex>,
    intro: Option<String>,
}

impl ReplyFilter {
    /// 按用户设置编译。
    ///
    /// # Errors
    /// 关键词正则在两端语义不同、或 Rust 引擎编不过时返回
    /// [`crate::error::ErrorKind::Unsupported`]。处理方式与
    /// [`crate::video_feed::FeedFilter::compile`] 一致：报错而不是忽略
    /// 这条规则——用户明确屏蔽过的评论照常显示，界面上没有任何提示，
    /// 比少看一页更糟。
    ///
    /// ## 大小写敏感性在这里被统一成「不敏感」
    ///
    /// 基线有三个构造点，而且**彼此不一致**：启动时
    /// （`RegExp(Pref.banWordForReply, caseSensitive: false)`）与设置页
    /// （`getBanWordModel` 同样是 `false`）不敏感，而长按菜单里「加入
    /// 过滤词」那条构造的是 `caseSensitive: true`
    /// （`reply_menu_helper.dart`）。
    ///
    /// 跨到 Rust 的只有模式串，必须二选一。选不敏感，因为那是**持久化
    /// 后的行为**：菜单加的词一旦重启就会被按不敏感重新编译，所以敏感
    /// 那一档本来就只活到下次重启。统一之后「当场」与「重启后」一致，
    /// 这是修掉一个基线的不一致，不是引入一个新的。
    pub fn compile(settings: &ReplyFilterSettings) -> Result<Self, DomainError> {
        Ok(Self {
            blocked_mids: settings.blocked_mids.iter().copied().collect(),
            anti_goods: settings.anti_goods,
            block_at_only: settings.block_at_only,
            block_at_count: settings.block_at_count,
            ban_word: if settings.ban_word.is_empty() {
                // 基线用 `pattern.isNotEmpty` 当开关（`enableFilter`）。
                None
            } else {
                Some(compile_user_pattern(&settings.ban_word, "评论关键词过滤")?)
            },
            intro: settings
                .intro
                .as_ref()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
        })
    }

    /// 这条评论是否要被移除。对应基线 `ReplyGrpc.needRemoveGrpc`。
    ///
    /// 五条判断的**顺序照抄基线**。短路求值下顺序只影响开销，不影响结果，
    /// 但照抄能让两边逐行对得上，省得下一个人怀疑是不是漏了一条。
    #[must_use]
    pub fn removes(&self, item: &ReplyFilterItem) -> bool {
        self.blocked_mids.contains(&item.mid)
            || self.blocks_by_at_count(item)
            || self.blocks_at_only(item)
            || (self.anti_goods && self.has_goods(item))
            || self
                .ban_word
                .as_ref()
                .is_some_and(|re| re.is_match(&item.message))
    }

    /// 这条评论是不是把稿件简介原样复读了一遍。
    ///
    /// **刻意不并进 [`Self::removes`]**：基线只把它用在根评论列表上
    /// （`ReplyController._removeIntro` 作用于 `MainListReply.replies`），
    /// 楼中楼、对话、折叠列表都不过。并进去会多屏蔽掉一批基线显示的评论，
    /// 那不是迁移，是改行为。
    #[must_use]
    pub fn removes_as_intro(&self, message: &str) -> bool {
        self.intro.as_ref().is_some_and(|i| message.trim() == i)
    }

    /// 基线 `_blockReplyAtCount`：`atNameToMid.length >= 阈值`。
    fn blocks_by_at_count(&self, item: &ReplyFilterItem) -> bool {
        self.block_at_count
            .is_some_and(|n| i64::try_from(item.at_names.len()).unwrap_or(i64::MAX) >= n)
    }

    /// 基线 `_blockAtOnlyReply`：整条评论除了 @ 某人之外没有别的内容。
    ///
    /// 「别的内容」的判定不只看文本：带表情或图片的直接放行（那条评论
    /// 显然不是纯 @）。
    fn blocks_at_only(&self, item: &ReplyFilterItem) -> bool {
        if !self.block_at_only {
            return false;
        }
        if item.at_names.is_empty() {
            return false;
        }
        if item.has_emotes || item.has_pictures {
            return false;
        }

        let message = item.message.as_str();
        let bytes = message.as_bytes();
        let len = bytes.len();
        let mut i = 0usize;

        // 楼中楼的回复在 message 前面带「回复 @某人:」这一段抬头，是服务端
        // 拼的，不算用户写的内容，所以从第一个 `:` 之后开始数。
        //
        // `root == parent` 时是直接挂在根评论下的一层，没有抬头，从 0 开始。
        //
        // 找不到 `:` 时基线是 `indexOf` 返回 -1、`i = 0`、`i <= 0` 成立而
        // 放行。照抄这个放行：抬头缺失说明这条评论的形状不是我们认得的
        // 那种，宁可不屏蔽。
        if item.root != item.parent {
            match message.find(':') {
                Some(p) => i = p + 1,
                None => return false,
            }
        }

        let mut has_at = false;
        while i < len {
            let c = bytes[i];
            // 空格与换行是 @ 之间的间隔，跳过。
            if c == b' ' || c == b'\n' {
                i += 1;
                continue;
            }
            // 出现任何不是 @ 的内容 → 这条评论有正文，放行。
            if c != b'@' {
                return false;
            }

            i += 1;
            let name_start = i;
            while i < len {
                let c = bytes[i];
                if c == b' ' || c == b'@' || c == b'\n' {
                    break;
                }
                i += 1;
            }

            // 光一个 `@` 后面没有名字。
            if i == name_start {
                return false;
            }
            // 切片边界只会落在 ASCII 字节或串尾上（见模块文档），不会
            // 切开多字节字符。
            let name = &message[name_start..i];
            // @ 的不是本条评论认得的人（服务端没给出对应 mid）→ 当成正文。
            if !item.at_names.iter().any(|n| n == name) {
                return false;
            }

            has_at = true;
        }

        has_at
    }

    /// 基线 `_needRemoveGoodGrpc`。
    fn has_goods(&self, item: &ReplyFilterItem) -> bool {
        item.url_extras
            .iter()
            .any(|e| e.goods_cm_control == 1 || e.has_goods_item_id || e.has_goods_prefetched_cache)
            || item.message.contains(GOODS_URL_PREFIX)
    }
}

#[cfg(test)]
// 测试里 expect 是对的：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        GOODS_URL_PREFIX, ReplyFilter, ReplyFilterItem, ReplyFilterSettings, ReplyUrlExtra,
    };

    fn filter(settings: ReplyFilterSettings) -> ReplyFilter {
        ReplyFilter::compile(&settings).expect("设置可编译")
    }

    /// 一条普通的根评论：有正文，没 @，没图，不带货。
    fn plain() -> ReplyFilterItem {
        ReplyFilterItem {
            mid: 1001,
            message: "这个讲得真清楚".to_owned(),
            ..ReplyFilterItem::default()
        }
    }

    /// 纯 @ 的根评论（`root == parent == 0`，没有抬头）。
    fn at_only() -> ReplyFilterItem {
        ReplyFilterItem {
            mid: 1001,
            message: "@阿强 @阿明".to_owned(),
            at_names: vec!["阿强".to_owned(), "阿明".to_owned()],
            ..ReplyFilterItem::default()
        }
    }

    #[test]
    fn empty_settings_keep_everything() {
        let f = filter(ReplyFilterSettings::default());
        assert!(!f.removes(&plain()));
        assert!(!f.removes(&at_only()), "开关没开时纯 @ 评论照样显示");
    }

    #[test]
    fn blocked_mid_is_removed() {
        let f = filter(ReplyFilterSettings {
            blocked_mids: vec![1001],
            ..ReplyFilterSettings::default()
        });
        assert!(f.removes(&plain()));
    }

    #[test]
    fn at_count_threshold_is_inclusive() {
        // 基线是 `>=`，不是 `>`：阈值 2 时两个 @ 就该被屏蔽。
        let f = filter(ReplyFilterSettings {
            block_at_count: Some(2),
            ..ReplyFilterSettings::default()
        });
        assert!(f.removes(&at_only()));

        let f = filter(ReplyFilterSettings {
            block_at_count: Some(3),
            ..ReplyFilterSettings::default()
        });
        assert!(!f.removes(&at_only()));
    }

    #[test]
    fn at_only_needs_every_token_to_be_a_known_at() {
        let f = filter(ReplyFilterSettings {
            block_at_only: true,
            ..ReplyFilterSettings::default()
        });
        assert!(f.removes(&at_only()));

        // 多一个字的正文就不是「纯 @」了。
        let mut with_text = at_only();
        with_text.message = "@阿强 说得对".to_owned();
        assert!(!f.removes(&with_text));

        // @ 了一个服务端没给出 mid 的名字：当成正文，放行。
        let mut unknown = at_only();
        unknown.message = "@阿强 @路人".to_owned();
        assert!(!f.removes(&unknown));
    }

    #[test]
    fn at_only_lets_emotes_and_pictures_through() {
        let f = filter(ReplyFilterSettings {
            block_at_only: true,
            ..ReplyFilterSettings::default()
        });

        let mut with_emote = at_only();
        with_emote.has_emotes = true;
        assert!(!f.removes(&with_emote), "带表情的不算纯 @");

        let mut with_picture = at_only();
        with_picture.has_pictures = true;
        assert!(!f.removes(&with_picture), "带图的不算纯 @");
    }

    #[test]
    fn at_only_skips_the_server_prefix_on_nested_replies() {
        let f = filter(ReplyFilterSettings {
            block_at_only: true,
            ..ReplyFilterSettings::default()
        });

        // root != parent：抬头「回复 @阿明:」要跳过，只看冒号之后。
        let nested = ReplyFilterItem {
            mid: 1001,
            message: "回复 @阿明:@阿强".to_owned(),
            at_names: vec!["阿明".to_owned(), "阿强".to_owned()],
            root: 7,
            parent: 9,
            ..ReplyFilterItem::default()
        };
        assert!(f.removes(&nested));

        // 同一串，但 root == parent（没有抬头）：「回复」两个字就是正文。
        let flat = ReplyFilterItem {
            root: 7,
            parent: 7,
            ..nested.clone()
        };
        assert!(!f.removes(&flat));
    }

    #[test]
    fn at_only_keeps_nested_reply_without_a_colon() {
        // 抬头缺失（没有冒号）时基线放行，不屏蔽。
        let f = filter(ReplyFilterSettings {
            block_at_only: true,
            ..ReplyFilterSettings::default()
        });
        let weird = ReplyFilterItem {
            mid: 1001,
            message: "@阿强".to_owned(),
            at_names: vec!["阿强".to_owned()],
            root: 7,
            parent: 9,
            ..ReplyFilterItem::default()
        };
        assert!(!f.removes(&weird));
    }

    #[test]
    fn at_only_rejects_a_bare_at_sign() {
        let f = filter(ReplyFilterSettings {
            block_at_only: true,
            ..ReplyFilterSettings::default()
        });
        let bare = ReplyFilterItem {
            mid: 1001,
            message: "@ ".to_owned(),
            at_names: vec!["阿强".to_owned()],
            ..ReplyFilterItem::default()
        };
        assert!(!f.removes(&bare), "`@` 后面没名字，不算纯 @");
    }

    #[test]
    fn at_only_handles_non_bmp_names() {
        // emoji 是 UTF-16 的代理对、UTF-8 的四字节序列。两端的切分边界
        // 都落在 ASCII 的空格上，名字应当被原样切出来。
        let f = filter(ReplyFilterSettings {
            block_at_only: true,
            ..ReplyFilterSettings::default()
        });
        let item = ReplyFilterItem {
            mid: 1001,
            message: "@阿强🎉 @阿明".to_owned(),
            at_names: vec!["阿强🎉".to_owned(), "阿明".to_owned()],
            ..ReplyFilterItem::default()
        };
        assert!(f.removes(&item));
    }

    #[test]
    fn goods_is_detected_by_extra_or_by_url_prefix() {
        let f = filter(ReplyFilterSettings {
            anti_goods: true,
            ..ReplyFilterSettings::default()
        });

        let mut by_cm_control = plain();
        by_cm_control.url_extras = vec![ReplyUrlExtra {
            goods_cm_control: 1,
            ..ReplyUrlExtra::default()
        }];
        assert!(f.removes(&by_cm_control));

        let mut by_item_id = plain();
        by_item_id.url_extras = vec![ReplyUrlExtra {
            has_goods_item_id: true,
            ..ReplyUrlExtra::default()
        }];
        assert!(f.removes(&by_item_id));

        let mut by_prefix = plain();
        by_prefix.message = format!("快来看 {GOODS_URL_PREFIX}/x");
        assert!(f.removes(&by_prefix));
    }

    #[test]
    fn goods_cm_control_matches_one_not_merely_nonzero() {
        // 基线判的是 `== Int64.ONE`。2 不算。
        let f = filter(ReplyFilterSettings {
            anti_goods: true,
            ..ReplyFilterSettings::default()
        });
        let mut item = plain();
        item.url_extras = vec![ReplyUrlExtra {
            goods_cm_control: 2,
            ..ReplyUrlExtra::default()
        }];
        assert!(!f.removes(&item));
    }

    #[test]
    fn goods_rules_only_apply_when_enabled() {
        let f = filter(ReplyFilterSettings::default());
        let mut item = plain();
        item.message = format!("快来看 {GOODS_URL_PREFIX}/x");
        assert!(!f.removes(&item), "开关没开时带货评论照样显示");
    }

    #[test]
    fn intro_is_matched_after_trimming_both_sides() {
        let f = filter(ReplyFilterSettings {
            intro: Some("  本期视频的简介  ".to_owned()),
            ..ReplyFilterSettings::default()
        });
        assert!(f.removes_as_intro(" 本期视频的简介\n"));
        assert!(!f.removes_as_intro("本期视频的简介！"));
        // 根评论列表之外的规则不受影响。
        let mut item = plain();
        item.message = "本期视频的简介".to_owned();
        assert!(!f.removes(&item), "简介判定刻意不并进 removes");
    }

    #[test]
    fn empty_intro_disables_the_rule() {
        let f = filter(ReplyFilterSettings {
            intro: Some("   ".to_owned()),
            ..ReplyFilterSettings::default()
        });
        assert!(!f.removes_as_intro("   "), "空简介不该把空评论也删掉");
    }

    #[test]
    fn ban_word_is_case_insensitive() {
        let f = filter(ReplyFilterSettings {
            ban_word: "SPAM".to_owned(),
            ..ReplyFilterSettings::default()
        });
        let mut item = plain();
        item.message = "这是 spam".to_owned();
        assert!(f.removes(&item));
    }

    #[test]
    fn empty_ban_word_disables_the_rule() {
        let f = filter(ReplyFilterSettings::default());
        assert!(!f.removes(&plain()));
    }

    #[test]
    fn divergent_pattern_is_rejected_rather_than_ignored() {
        // `\d` 在两端语义不同（ADR-010 决策二）：必须报错，不能当没这条规则。
        let err = ReplyFilter::compile(&ReplyFilterSettings {
            ban_word: r"\d{3}".to_owned(),
            ..ReplyFilterSettings::default()
        })
        .expect_err("两端语义不同的构造必须被拒");
        assert_eq!(err.kind(), crate::error::ErrorKind::Unsupported);
    }
}
