//! 电竞赛事的领域实体（S15 / issue #66）。
//!
//! 对应基线 `lib/models_new/match/match_info/`。
//!
//! ## 一个实体，两套字段名
//!
//! 同一场比赛有两个来源，键名风格不同：赛事详情接口
//! （`/x/esports/matchs/seasons`）是 `snake_case`，综合搜索里的赛事卡片
//! 是 `camelCase`（`gameStage`、`homeTeam`、`ID`）。基线为此在
//! `MatchContest` 上挂了两个工厂方法。两边的解析都在 `bili-api`
//! （`match_info::parse_contest` 与 `parse_search_contests`），领域层只有
//! 这一个实体——**调用点不该知道这场比赛是从哪个接口来的**。

use serde::Serialize;

/// 一场比赛。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct MatchContest {
    /// 只有搜索卡片给（`ID`）。详情接口不给，因为调用方本来就是拿 id 去查的。
    pub id: Option<i64>,
    /// 赛段，如「决赛」。**空串已归一成 `None`**：基线的
    /// `nonNullOrEmptyString` 做的就是这件事，界面据此决定要不要留出那一行。
    pub game_stage: Option<String>,
    /// 开赛时间，Unix 秒。
    pub stime: Option<i64>,
    pub home_score: Option<i64>,
    pub away_score: Option<i64>,
    /// 直播间号。0 表示没有直播。
    pub live_room: Option<i64>,
    pub season: Option<MatchSeason>,
    pub home_team: Option<MatchTeam>,
    pub away_team: Option<MatchTeam>,
    /// 1 未开始、2 进行中、3 已结束（界面据此显示比分或倒计时）。
    pub contest_status: Option<i64>,
    /// 回放地址。
    pub playback: Option<String>,
}

/// 赛事（季）。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct MatchSeason {
    pub title: Option<String>,
    pub logo: Option<String>,
}

/// 参赛队伍。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct MatchTeam {
    pub title: Option<String>,
    pub logo: Option<String>,
    /// 完整版队徽。注意服务端这个键在两套风格里都是 `logoFull`。
    pub logo_full: Option<String>,
}
