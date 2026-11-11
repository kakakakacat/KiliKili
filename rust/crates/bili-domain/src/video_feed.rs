//! 视频流条目：热门、排行、相关推荐、每周必看、入站必刷。
//!
//! 这五条接口在基线里共用一个 `HotVideoItemModel`
//! （`lib/models/model_hot_video_item.dart`），响应形状确实同构，所以
//! 这里也只有一个 [`FeedVideoItem`]。
//!
//! ## 过滤规则为什么在这一层
//!
//! 基线把三套过滤散在三处：`VideoHttp._filterHotAndRank`（热门/排行）、
//! `RecommendFilter`（时长、播放量、点赞率、标题词）、`whereNotBlocked`
//! （拉黑的 UP 主，`GlobalData().blackMids`）。它们是**业务规则**，不是
//! 协议解析，也不是界面逻辑——换一套 UI 仍然成立，所以放领域层。
//!
//! 规则的**取值**来自用户设置，按请求传入（[`FeedFilterSettings`]），
//! Rust 不存：与 Cookie 同理，真相源留在 Dart。
//!
//! ## 哪几条接口过滤、哪几条不过滤
//!
//! 严格照基线，不「顺手统一」：
//!
//! | 接口 | 拉黑 UP | 标题词 | 分区词 | 点赞率/播放量 | 时长 |
//! |---|---|---|---|---|---|
//! | 热门 / 排行 | ✓ | ✓ | ✓ | ✓ | — |
//! | 相关推荐 | ✓ | 仅当开了「对相关视频生效」 | — | 同左 | 同左 |
//! | 每周必看 / 入站必刷 | — | — | — | — | — |
//!
//! 每周必看与入站必刷在基线里一条都不过滤。把它们一起过滤是行为变更，
//! 不属于本次迁移。

use std::collections::HashSet;

use serde::Serialize;

use crate::error::DomainError;
use crate::regex_compat::compile_user_pattern;

/// 一条视频流条目。字段与基线 `HotVideoItemModel` 一一对应。
///
/// 全是 `Option`：服务端在不同接口里给的字段并不一致（排行榜没有
/// `pub_location`，相关推荐没有 `now_rank`），缺字段不该让整条列表作废。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeedVideoItem {
    pub aid: Option<i64>,
    pub cid: Option<i64>,
    pub bvid: Option<String>,
    pub videos: Option<i64>,
    pub tid: Option<i64>,
    pub tname: Option<String>,
    pub copyright: Option<i64>,
    /// 封面（响应里的 `pic`）。
    pub cover: Option<String>,
    pub title: Option<String>,
    pub pubdate: Option<i64>,
    pub ctime: Option<i64>,
    pub desc: Option<String>,
    pub state: Option<i64>,
    /// 秒。基线缺字段时填 -1（`BaseSimpleVideoItemModel.duration`），
    /// 这里保持 `None`，由过滤规则按「未知时长不过滤」处理。
    pub duration: Option<i64>,
    pub owner: FeedOwner,
    pub stat: FeedStat,
    /// 已按 `rotate` 归正的宽高，见 [`FeedDimension`]。
    pub dimension: Option<FeedDimension>,
    pub first_frame: Option<String>,
    pub pub_location: Option<String>,
    pub redirect_url: Option<String>,
    pub progress: Option<f64>,
    /// 角标的**来源**，不是角标文案：`charging`（充电专属）/
    /// `cooperation`（合作）。文案属界面，留在 Dart（ADR-011 决策二）。
    pub badge_kind: Option<String>,
    /// 服务端直接给的角标文案（`pgc_label`）。仅在 `badge_kind` 为空时
    /// 有意义——基线的三级优先级是 充电 > 合作 > pgc_label。
    pub pgc_label: Option<String>,
}

/// UP 主。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeedOwner {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
}

/// 计数。`coin` 在响应里偶尔是浮点，基线用 `num` 收，这里用 `f64`。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeedStat {
    pub view: Option<i64>,
    pub like: Option<i64>,
    pub danmaku: Option<i64>,
    pub reply: Option<i64>,
    pub favorite: Option<i64>,
    pub coin: Option<f64>,
    pub share: Option<i64>,
    pub now_rank: Option<i64>,
    pub his_rank: Option<i64>,
    pub dislike: Option<i64>,
    pub vt: Option<i64>,
    pub vv: Option<i64>,
}

/// 画面宽高。
///
/// **已经按 `rotate` 归正**：基线 `Dimension.fromJson` 在 `rotate == 1`
/// 时交换宽高，竖屏判定依赖这一步。归正是对协议的理解，放这里；UI 只管
/// 比大小。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeedDimension {
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// 每周必看的一期。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PopularSeriesOne {
    pub config: Option<PopularSeriesConfig>,
    pub reminder: Option<String>,
    pub list: Option<Vec<FeedVideoItem>>,
}

/// 每周必看一期的标题信息。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PopularSeriesConfig {
    pub name: Option<String>,
    pub label: Option<String>,
    pub media_id: Option<i64>,
}

/// 每周必看的期号列表项。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PopularSeriesListItem {
    pub number: Option<i64>,
    pub subject: Option<String>,
    pub status: Option<i64>,
    pub name: Option<String>,
}

/// 入站必刷。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PopularPrecious {
    pub media_id: Option<i64>,
    pub list: Option<Vec<FeedVideoItem>>,
}

/// 过滤规则的取值，来自用户设置（基线 `Pref` + `GlobalData().blackMids`）。
#[derive(Debug, Clone, Default)]
pub struct FeedFilterSettings {
    /// 拉黑的 UP 主 mid。
    pub blocked_mids: Vec<i64>,
    /// 推荐屏蔽词正则源串（基线 `Pref.banWordForRecommend`）。空串=不启用。
    pub ban_word_rcmd: String,
    /// 分区屏蔽词正则源串（基线 `Pref.banWordForZone`）。空串=不启用。
    pub ban_word_zone: String,
    /// 低于此时长（秒）的过滤掉。
    pub min_duration: i64,
    /// 低于此播放量的过滤掉。
    pub min_play: i64,
    /// 点赞率（百分比）低于此值的过滤掉。
    pub min_like_ratio: i64,
    /// 相关推荐是否也套用上面这些阈值（基线 `applyFilterToRelatedVideos`）。
    pub apply_to_related: bool,
}

/// 编译好的过滤规则。
#[derive(Debug)]
pub struct FeedFilter {
    blocked_mids: HashSet<i64>,
    ban_word_rcmd: Option<regex::Regex>,
    ban_word_zone: Option<regex::Regex>,
    min_duration: i64,
    min_play: i64,
    min_like_ratio: i64,
    apply_to_related: bool,
}

impl FeedFilter {
    /// 按用户设置编译。
    ///
    /// # Errors
    /// 屏蔽词正则在两端语义不同、或 Rust 引擎编不过时返回
    /// [`crate::error::ErrorKind::Unsupported`]。
    ///
    /// **为什么是报错而不是忽略这条规则**：typed 直连没有降级路径
    /// （ADR-011 决策四）。忽略规则意味着用户明确屏蔽过的内容照样出现，
    /// 而且没有任何提示；报错至少让界面能说清「屏蔽词规则不被支持，请
    /// 修改设置」。少看一屏列表可以重试，看到拉黑的内容不能撤回。
    pub fn compile(settings: &FeedFilterSettings) -> Result<Self, DomainError> {
        let compile_one =
            |pattern: &str, label: &str| -> Result<Option<regex::Regex>, DomainError> {
                if pattern.is_empty() {
                    // 基线用 `pattern.isNotEmpty` 当开关，空串即关闭。
                    Ok(None)
                } else {
                    compile_user_pattern(pattern, label).map(Some)
                }
            };

        Ok(Self {
            blocked_mids: settings.blocked_mids.iter().copied().collect(),
            ban_word_rcmd: compile_one(&settings.ban_word_rcmd, "推荐屏蔽词")?,
            ban_word_zone: compile_one(&settings.ban_word_zone, "分区屏蔽词")?,
            min_duration: settings.min_duration,
            min_play: settings.min_play,
            min_like_ratio: settings.min_like_ratio,
            apply_to_related: settings.apply_to_related,
        })
    }

    /// 热门 / 排行的保留判定。对应基线 `VideoHttp._filterHotAndRank`。
    ///
    /// 注意这里**不看时长**——基线的热门与排行就没有时长过滤，只有相关
    /// 推荐与首页推荐才走 `filterAll`。
    #[must_use]
    pub fn keeps_hot_or_rank(&self, item: &FeedVideoItem) -> bool {
        !self.is_blocked(item)
            && !self.matches_title(item)
            && !self.matches_like_ratio(item)
            && !self.matches_zone(item)
    }

    /// 相关推荐的保留判定。对应基线
    /// `whereNotBlocked(...)` + 可选的 `RecommendFilter.filterAll`。
    #[must_use]
    pub fn keeps_related(&self, item: &FeedVideoItem) -> bool {
        if self.is_blocked(item) {
            return false;
        }
        if !self.apply_to_related {
            return true;
        }
        !(self.matches_duration(item) || self.matches_like_ratio(item) || self.matches_title(item))
    }

    fn is_blocked(&self, item: &FeedVideoItem) -> bool {
        // mid 缺失时基线是 `contains(null)`，恒为 false（blackMids 里不会
        // 有 null），即不过滤。
        item.owner
            .mid
            .is_some_and(|mid| self.blocked_mids.contains(&mid))
    }

    fn matches_title(&self, item: &FeedVideoItem) -> bool {
        match (&self.ban_word_rcmd, &item.title) {
            (Some(re), Some(title)) => re.is_match(title),
            _ => false,
        }
    }

    fn matches_zone(&self, item: &FeedVideoItem) -> bool {
        // 基线明确要求 tname 非空才判（`i['tname'] != null && ...`）。
        match (&self.ban_word_zone, &item.tname) {
            (Some(re), Some(tname)) => re.is_match(tname),
            _ => false,
        }
    }

    /// 基线 `RecommendFilter.filterDuration`：`duration > 0 && < 阈值`。
    /// 时长未知（基线是 -1）时不过滤。
    fn matches_duration(&self, item: &FeedVideoItem) -> bool {
        item.duration
            .is_some_and(|d| d > 0 && d < self.min_duration)
    }

    /// 基线 `RecommendFilter.filterLikeRatio`。
    ///
    /// 两个 `> -1` 的判断照抄：服务端用 -1 表示「不公开」，那种条目基线
    /// 是放行的，不是当成 0。
    fn matches_like_ratio(&self, item: &FeedVideoItem) -> bool {
        let Some(view) = item.stat.view else {
            // 基线：view 为 null 时整条规则不生效。
            return false;
        };
        if view > -1 && view < self.min_play {
            return true;
        }
        item.stat
            .like
            .is_some_and(|like| like > -1 && like * 100 < self.min_like_ratio * view)
    }
}

#[cfg(test)]
// 测试里 expect 是对的：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        FeedDimension, FeedFilter, FeedFilterSettings, FeedOwner, FeedStat, FeedVideoItem,
    };
    use crate::error::ErrorKind;

    fn item() -> FeedVideoItem {
        FeedVideoItem {
            aid: Some(1),
            cid: Some(2),
            bvid: Some("BV1Fixture01".to_owned()),
            videos: Some(1),
            tid: Some(22),
            tname: Some("鬼畜调教".to_owned()),
            copyright: Some(1),
            cover: None,
            title: Some("一个普通标题".to_owned()),
            pubdate: Some(0),
            ctime: Some(0),
            desc: None,
            state: Some(0),
            duration: Some(600),
            owner: FeedOwner {
                mid: Some(20001),
                name: Some("UP主".to_owned()),
                face: None,
            },
            stat: FeedStat {
                view: Some(100_000),
                like: Some(10_000),
                danmaku: Some(1),
                reply: None,
                favorite: None,
                coin: None,
                share: None,
                now_rank: None,
                his_rank: None,
                dislike: None,
                vt: None,
                vv: None,
            },
            dimension: Some(FeedDimension {
                width: Some(1920),
                height: Some(1080),
            }),
            first_frame: None,
            pub_location: None,
            redirect_url: None,
            progress: None,
            badge_kind: None,
            pgc_label: None,
        }
    }

    fn settings() -> FeedFilterSettings {
        FeedFilterSettings {
            min_duration: 0,
            min_play: 0,
            min_like_ratio: 0,
            ..FeedFilterSettings::default()
        }
    }

    #[test]
    fn default_settings_keep_everything() {
        let filter = FeedFilter::compile(&settings()).expect("空设置可编译");
        assert!(filter.keeps_hot_or_rank(&item()));
        assert!(filter.keeps_related(&item()));
    }

    #[test]
    fn blocked_up_is_dropped_everywhere() {
        let filter = FeedFilter::compile(&FeedFilterSettings {
            blocked_mids: vec![20001],
            ..settings()
        })
        .expect("可编译");
        assert!(!filter.keeps_hot_or_rank(&item()));
        // 相关推荐即使没开「对相关视频生效」，拉黑仍然生效——基线的
        // whereNotBlocked 在那个开关之外。
        assert!(!filter.keeps_related(&item()));
    }

    #[test]
    fn missing_mid_is_never_blocked() {
        let mut it = item();
        it.owner.mid = None;
        let filter = FeedFilter::compile(&FeedFilterSettings {
            blocked_mids: vec![20001],
            ..settings()
        })
        .expect("可编译");
        assert!(filter.keeps_hot_or_rank(&it));
    }

    #[test]
    fn title_ban_word_applies_to_hot_and_rank() {
        let filter = FeedFilter::compile(&FeedFilterSettings {
            ban_word_rcmd: "普通".to_owned(),
            ..settings()
        })
        .expect("可编译");
        assert!(!filter.keeps_hot_or_rank(&item()));
    }

    #[test]
    fn title_ban_word_is_case_insensitive_like_the_baseline() {
        let mut it = item();
        it.title = Some("Watch This AD Now".to_owned());
        let filter = FeedFilter::compile(&FeedFilterSettings {
            ban_word_rcmd: "ad".to_owned(),
            ..settings()
        })
        .expect("可编译");
        assert!(!filter.keeps_hot_or_rank(&it));
    }

    #[test]
    fn zone_ban_word_only_applies_to_hot_and_rank() {
        let filter = FeedFilter::compile(&FeedFilterSettings {
            ban_word_zone: "鬼畜".to_owned(),
            apply_to_related: true,
            ..settings()
        })
        .expect("可编译");
        assert!(!filter.keeps_hot_or_rank(&item()));
        // 基线的相关推荐从不看分区词，只有 _filterHotAndRank 看。
        assert!(filter.keeps_related(&item()));
    }

    #[test]
    fn related_ignores_thresholds_unless_the_switch_is_on() {
        let strict = FeedFilterSettings {
            min_duration: 1200,
            min_play: 1_000_000,
            min_like_ratio: 50,
            ban_word_rcmd: "普通".to_owned(),
            ..settings()
        };

        let off = FeedFilter::compile(&FeedFilterSettings {
            apply_to_related: false,
            ..strict.clone()
        })
        .expect("可编译");
        assert!(off.keeps_related(&item()), "开关关着时相关推荐不套阈值");

        let on = FeedFilter::compile(&FeedFilterSettings {
            apply_to_related: true,
            ..strict
        })
        .expect("可编译");
        assert!(!on.keeps_related(&item()));
    }

    #[test]
    fn duration_filter_never_applies_to_hot_or_rank() {
        let mut it = item();
        it.duration = Some(30);
        let filter = FeedFilter::compile(&FeedFilterSettings {
            min_duration: 300,
            ..settings()
        })
        .expect("可编译");
        // 基线 _filterHotAndRank 里压根没有时长判断，加上就是行为变更。
        assert!(filter.keeps_hot_or_rank(&it));
    }

    #[test]
    fn unknown_duration_is_not_filtered() {
        let mut it = item();
        it.duration = None;
        let filter = FeedFilter::compile(&FeedFilterSettings {
            min_duration: 300,
            apply_to_related: true,
            ..settings()
        })
        .expect("可编译");
        assert!(filter.keeps_related(&it));
    }

    #[test]
    fn hidden_counts_are_passed_through_not_treated_as_zero() {
        // 服务端用 -1 表示「不公开」。基线的两个 `> -1` 判断让这类条目
        // 原样放行，当成 0 会把它们全部过滤掉。
        let mut it = item();
        it.stat.view = Some(-1);
        it.stat.like = Some(-1);
        let filter = FeedFilter::compile(&FeedFilterSettings {
            min_play: 1_000_000,
            min_like_ratio: 90,
            ..settings()
        })
        .expect("可编译");
        assert!(filter.keeps_hot_or_rank(&it));
    }

    #[test]
    fn missing_view_disables_the_ratio_rule() {
        let mut it = item();
        it.stat.view = None;
        let filter = FeedFilter::compile(&FeedFilterSettings {
            min_play: 1_000_000,
            min_like_ratio: 90,
            ..settings()
        })
        .expect("可编译");
        assert!(filter.keeps_hot_or_rank(&it));
    }

    #[test]
    fn low_play_count_is_filtered() {
        let mut it = item();
        it.stat.view = Some(100);
        let filter = FeedFilter::compile(&FeedFilterSettings {
            min_play: 1_000,
            ..settings()
        })
        .expect("可编译");
        assert!(!filter.keeps_hot_or_rank(&it));
    }

    #[test]
    fn low_like_ratio_is_filtered() {
        let mut it = item();
        it.stat.view = Some(100_000);
        it.stat.like = Some(100); // 0.1% < 5%
        let filter = FeedFilter::compile(&FeedFilterSettings {
            min_like_ratio: 5,
            ..settings()
        })
        .expect("可编译");
        assert!(!filter.keeps_hot_or_rank(&it));
    }

    #[test]
    fn divergent_regex_is_rejected_rather_than_silently_ignored() {
        // 忽略这条规则 = 用户屏蔽过的内容照样出现且没有提示。typed 直连
        // 没有降级路径，只能报错（ADR-011 决策四）。
        let err = FeedFilter::compile(&FeedFilterSettings {
            ban_word_rcmd: r"\d+".to_owned(),
            ..settings()
        })
        .expect_err("两端语义不同，必须拒绝");
        assert_eq!(err.kind(), ErrorKind::Unsupported);
        assert!(
            err.to_string().contains("推荐屏蔽词"),
            "错误要指明是哪一项设置"
        );
    }

    #[test]
    fn unsupported_zone_regex_names_its_own_setting() {
        let err = FeedFilter::compile(&FeedFilterSettings {
            ban_word_zone: "foo(?=bar)".to_owned(),
            ..settings()
        })
        .expect_err("Rust 不支持 lookahead");
        assert_eq!(err.kind(), ErrorKind::Unsupported);
        assert!(err.to_string().contains("分区屏蔽词"));
    }
}
