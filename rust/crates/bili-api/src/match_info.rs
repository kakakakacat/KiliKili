//! 电竞赛事详情的请求构造与解析（S15 / issue #66）。
//!
//! 对应基线 `MatchHttp.matchInfo`（`lib/http/match.dart`），外加综合搜索
//! 里赛事卡片的解析——同一个实体两套键名，见
//! [`bili_domain::match_info`]。S9 迁搜索时把赛事块原样当 JSON 串带回
//! Dart，正是因为那时 `MatchContest` 还有本接口这个未迁的生产者；两边
//! 一起迁完，那个绕路（`lib/models/search/search_esports.dart`）才能去掉。

use bili_domain::error::DomainError;
use bili_domain::match_info::{MatchContest, MatchSeason, MatchTeam};
use serde::Deserialize;

use crate::http::{HttpTransport, RequestContext};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const MATCH_INFO: &str = "/x/esports/match/info";
}

/// 赛事详情的请求。
///
/// `platform=2` 是基线写死的（移动端）。
#[must_use]
pub fn match_info_request(contest_id: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::MATCH_INFO,
        query: vec![
            ("cid".to_owned(), contest_id.to_string()),
            ("platform".to_owned(), "2".to_owned()),
        ],
    }
}

/// 取一场比赛的详情。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。响应里没有 `contest`
/// 时返回 `None`——基线在这里给的是 `Success(null)`，界面显示空态而不是
/// 错误。
pub async fn fetch_match_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    contest_id: i64,
) -> Result<Option<MatchContest>, DomainError> {
    let req = match_info_request(contest_id);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_match_info(&body)
}

/// 赛事详情接口的字段名：`snake_case`。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ContestDto {
    game_stage: Option<String>,
    stime: Option<i64>,
    home_score: Option<i64>,
    away_score: Option<i64>,
    live_room: Option<i64>,
    season: Option<SeasonDto>,
    home_team: Option<TeamDto>,
    away_team: Option<TeamDto>,
    contest_status: Option<i64>,
    playback: Option<String>,
}

/// 综合搜索赛事卡片的字段名：`camelCase`，且多一个 `ID`。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SearchContestDto {
    #[serde(rename = "ID")]
    id: Option<i64>,
    #[serde(rename = "gameStage")]
    game_stage: Option<String>,
    stime: Option<i64>,
    #[serde(rename = "homeScore")]
    home_score: Option<i64>,
    #[serde(rename = "awayScore")]
    away_score: Option<i64>,
    #[serde(rename = "liveRoom")]
    live_room: Option<i64>,
    season: Option<SeasonDto>,
    #[serde(rename = "homeTeam")]
    home_team: Option<TeamDto>,
    #[serde(rename = "awayTeam")]
    away_team: Option<TeamDto>,
    #[serde(rename = "contestStatus")]
    contest_status: Option<i64>,
    playback: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonDto {
    title: Option<String>,
    logo: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TeamDto {
    title: Option<String>,
    logo: Option<String>,
    // 两套键名风格里这一个都叫 logoFull。
    #[serde(rename = "logoFull")]
    logo_full: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MatchInfoDto {
    contest: Option<ContestDto>,
}

/// 空串归一成 `None`。基线 `nonNullOrEmptyString` 做的就是这件事：界面用
/// 它决定要不要留出赛段那一行，空串会留出一行空白。
fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.is_empty())
}

fn season(dto: Option<SeasonDto>) -> Option<MatchSeason> {
    dto.map(|s| MatchSeason {
        title: s.title,
        logo: s.logo,
    })
}

fn team(dto: Option<TeamDto>) -> Option<MatchTeam> {
    dto.map(|t| MatchTeam {
        title: t.title,
        logo: t.logo,
        logo_full: t.logo_full,
    })
}

impl SearchContestDto {
    pub(crate) fn into_domain(self) -> MatchContest {
        MatchContest {
            id: self.id,
            game_stage: non_empty(self.game_stage),
            stime: self.stime,
            home_score: self.home_score,
            away_score: self.away_score,
            live_room: self.live_room,
            season: season(self.season),
            home_team: team(self.home_team),
            away_team: team(self.away_team),
            contest_status: self.contest_status,
            playback: self.playback,
        }
    }
}

/// 解析赛事详情。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_match_info(body: &str) -> Result<Option<MatchContest>, DomainError> {
    let dto: MatchInfoDto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto.contest.map(|c| MatchContest {
        // 详情接口不给 id：调用方本来就是拿着 id 去查的。
        id: None,
        game_stage: non_empty(c.game_stage),
        stime: c.stime,
        home_score: c.home_score,
        away_score: c.away_score,
        live_room: c.live_room,
        season: season(c.season),
        home_team: team(c.home_team),
        away_team: team(c.away_team),
        contest_status: c.contest_status,
        playback: c.playback,
    }))
}

/// 解析综合搜索里的赛事卡片（`{"contest":[…]}`，camelCase）。
///
/// 结构不符时返回空列表：赛事卡片解析失败不该让整页综合搜索出不来——
/// 基线在这里也是 `try { … } catch (_) {}` 吞掉。
#[must_use]
pub fn parse_search_contests(value: &serde_json::Value) -> Vec<MatchContest> {
    let Some(list) = value.get("contest").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|item| serde_json::from_value::<SearchContestDto>(item.clone()).ok())
        .map(SearchContestDto::into_domain)
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn request_is_the_baseline_shape() {
        let req = match_info_request(77);
        assert_eq!(req.path, path::MATCH_INFO);
        assert_eq!(req.query_string(), "cid=77&platform=2");
    }

    #[test]
    fn empty_game_stage_becomes_none() {
        let body = r#"{"code":0,"message":"0","data":{"contest":{"game_stage":""}}}"#;
        let contest = parse_match_info(body).expect("应解析成功").expect("有比赛");
        assert_eq!(contest.game_stage, None, "空串与没有是一回事");
    }

    #[test]
    fn missing_contest_is_none_not_an_error() {
        let parsed = parse_match_info(r#"{"code":0,"message":"0","data":{}}"#).expect("不是错误");
        assert!(parsed.is_none());
    }

    #[test]
    fn the_two_key_styles_produce_the_same_entity() {
        let detail = parse_match_info(
            r#"{"code":0,"message":"0","data":{"contest":{
                "game_stage":"决赛","stime":1700000000,"home_score":2,"away_score":1,
                "live_room":123,"contest_status":3,"playback":"https://example.invalid/p",
                "season":{"title":"S14","logo":"https://example.invalid/s.png"},
                "home_team":{"title":"A","logo":"https://example.invalid/a.png","logoFull":"https://example.invalid/af.png"},
                "away_team":{"title":"B","logo":"https://example.invalid/b.png","logoFull":"https://example.invalid/bf.png"}
            }}}"#,
        )
        .expect("应解析成功")
        .expect("有比赛");

        let from_search = parse_search_contests(
            &serde_json::from_str(
                r#"{"contest":[{
                    "ID":77,"gameStage":"决赛","stime":1700000000,"homeScore":2,"awayScore":1,
                    "liveRoom":123,"contestStatus":3,"playback":"https://example.invalid/p",
                    "season":{"title":"S14","logo":"https://example.invalid/s.png"},
                    "homeTeam":{"title":"A","logo":"https://example.invalid/a.png","logoFull":"https://example.invalid/af.png"},
                    "awayTeam":{"title":"B","logo":"https://example.invalid/b.png","logoFull":"https://example.invalid/bf.png"}
                }]}"#,
            )
            .expect("合法 JSON"),
        );

        assert_eq!(from_search.len(), 1);
        // 只有 id 不同：详情接口不给，搜索卡片给。
        assert_eq!(from_search[0].id, Some(77));
        assert_eq!(
            MatchContest {
                id: None,
                ..from_search[0].clone()
            },
            detail
        );
    }

    #[test]
    fn a_malformed_card_does_not_sink_the_others() {
        let contests = parse_search_contests(
            &serde_json::from_str(r#"{"contest":[{"ID":"不是数字"},{"ID":2}]}"#)
                .expect("合法 JSON"),
        );
        assert_eq!(contests.len(), 1, "坏的那条丢掉，好的那条留下");
        assert_eq!(contests[0].id, Some(2));
    }

    #[test]
    fn a_missing_contest_block_is_an_empty_list() {
        assert!(parse_search_contests(&serde_json::json!({})).is_empty());
    }
}
