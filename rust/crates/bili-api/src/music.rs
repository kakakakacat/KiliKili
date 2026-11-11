//! 音乐页只读接口的请求构造与解析（S15 / issue #66）。
//!
//! 对应基线 `MusicHttp` 的 `bgmDetail`、`bgmRecommend`（`lib/http/music.dart`）。
//! 点「想听」(`wishUpdate`) 是写操作，等 ADR-013（issue #50）。
//!
//! 两处把基线行为原样搬过来的地方，都在解析里：
//! 1. **成就标签要拼三处**：`achievement` 数组加上 `music_rank`、
//!    `recreation_rank` 两个单独字段，顺序即界面排标签的顺序；
//! 2. **热度曲线要反转**：服务端给的是时间降序，基线在模型里 reverse 过，
//!    曲线图按列表顺序画。

use bili_domain::error::DomainError;
use bili_domain::music::{Artist, BgmRecommend, HotSongHeat, MusicComment, MusicDetail, SongHeat};
use bili_protocol::wbi;
use serde::Deserialize;

use crate::http::{HttpTransport, RequestContext};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const BGM_DETAIL: &str = "/x/copyright-music-publicity/bgm/detail";
    pub const BGM_RECOMMEND: &str = "/x/copyright-music-publicity/bgm/recommend_list";
}

/// BGM 详情的请求。WBI 签名。
///
/// `relation_from=bgm_page` 是基线写死的固定值（告诉服务端这次是从音乐页
/// 来的，影响它给不给 `music_relation`）。
#[must_use]
pub fn bgm_detail_request(music_id: &str, mixin_key: &str, wts: i64) -> PreparedRequest {
    let mut query = vec![
        ("music_id".to_owned(), music_id.to_owned()),
        ("relation_from".to_owned(), "bgm_page".to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest {
        path: path::BGM_DETAIL,
        query,
    }
}

/// 相关推荐的请求。不签名。
#[must_use]
pub fn bgm_recommend_request(music_id: &str) -> PreparedRequest {
    PreparedRequest {
        path: path::BGM_RECOMMEND,
        query: vec![("music_id".to_owned(), music_id.to_owned())],
    }
}

/// 取 BGM 详情。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_bgm_detail<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    music_id: &str,
    mixin_key: &str,
    wts: i64,
) -> Result<MusicDetail, DomainError> {
    let req = bgm_detail_request(music_id, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_bgm_detail(&body)
}

/// 取相关推荐。
///
/// # Errors
/// 见 [`fetch_bgm_detail`]。
pub async fn fetch_bgm_recommend<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    music_id: &str,
) -> Result<Vec<BgmRecommend>, DomainError> {
    let req = bgm_recommend_request(music_id);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_bgm_recommend(&body)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MusicDetailDto {
    music_title: Option<String>,
    origin_artist: Option<String>,
    origin_artist_list: Option<String>,
    mv_aid: Option<i64>,
    mv_cid: Option<i64>,
    mv_bvid: Option<String>,
    mv_cover: Option<String>,
    wish_listen: Option<bool>,
    wish_count: Option<i64>,
    music_source: Option<String>,
    album: Option<String>,
    artists_list: Option<Vec<ArtistDto>>,
    listen_pv: Option<i64>,
    achievement: Option<Vec<String>>,
    music_rank: Option<String>,
    recreation_rank: Option<String>,
    hot_song_heat: Option<HotSongHeatDto>,
    music_comment: Option<MusicCommentDto>,
    music_relation: Option<i64>,
    music_publish: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArtistDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
    identity: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HotSongHeatDto {
    last_heat: Option<i64>,
    song_heat: Option<Vec<SongHeatDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SongHeatDto {
    date: Option<i64>,
    heat: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MusicCommentDto {
    nums: Option<i64>,
    oid: Option<i64>,
    page_type: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BgmRecommendListDto {
    list: Option<Vec<BgmRecommendDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BgmRecommendDto {
    bvid: Option<String>,
    cid: Option<i64>,
    cover: Option<String>,
    title: Option<String>,
    up_nick_name: Option<String>,
    play: Option<i64>,
    danmu: Option<i64>,
    duration: Option<i64>,
    label_list: Option<Vec<LabelDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LabelDto {
    name: Option<String>,
}

/// 解析 BGM 详情。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_bgm_detail(body: &str) -> Result<MusicDetail, DomainError> {
    let dto: MusicDetailDto = crate::envelope::unwrap_envelope(body)?;

    // 成就标签：数组在前，两个单独榜单字段按基线的顺序追加在后。
    let mut achievement = dto.achievement.unwrap_or_default();
    achievement.extend(dto.music_rank);
    achievement.extend(dto.recreation_rank);

    Ok(MusicDetail {
        music_title: dto.music_title,
        origin_artist: dto.origin_artist,
        origin_artist_list: dto.origin_artist_list,
        mv_aid: dto.mv_aid,
        mv_cid: dto.mv_cid.unwrap_or(0),
        mv_bvid: dto.mv_bvid,
        mv_cover: dto.mv_cover,
        wish_listen: dto.wish_listen,
        wish_count: dto.wish_count,
        music_source: dto.music_source,
        album: dto.album,
        artists_list: dto
            .artists_list
            .unwrap_or_default()
            .into_iter()
            .map(|a| Artist {
                mid: a.mid,
                name: a.name,
                face: a.face,
                identity: a.identity,
            })
            .collect(),
        listen_pv: dto.listen_pv,
        achievement,
        hot_song_heat: dto.hot_song_heat.map(|h| HotSongHeat {
            last_heat: h.last_heat,
            song_heat: h
                .song_heat
                .unwrap_or_default()
                .into_iter()
                // 服务端给的是时间降序，反转成升序供曲线图按顺序画。
                .rev()
                // 缺 date 或 heat 的点画不出来，丢掉这一个点而不是整条曲线。
                .filter_map(|p| {
                    Some(SongHeat {
                        date: p.date?,
                        heat: p.heat?,
                    })
                })
                .collect(),
        }),
        music_comment: dto.music_comment.map(|c| MusicComment {
            nums: c.nums,
            oid: c.oid,
            page_type: c.page_type,
        }),
        music_relation: dto.music_relation,
        music_publish: dto.music_publish,
    })
}

/// 解析相关推荐。
///
/// # Errors
/// 见 [`parse_bgm_detail`]。`list` 缺失按空列表处理，与基线一致。
pub fn parse_bgm_recommend(body: &str) -> Result<Vec<BgmRecommend>, DomainError> {
    let dto: BgmRecommendListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto
        .list
        .unwrap_or_default()
        .into_iter()
        .map(|i| BgmRecommend {
            bvid: i.bvid,
            cid: i.cid,
            cover: i.cover,
            title: i.title,
            up_nick_name: i.up_nick_name,
            play: i.play,
            danmu: i.danmu,
            duration: i.duration,
            labels: i
                .label_list
                .unwrap_or_default()
                .into_iter()
                .filter_map(|l| l.name)
                .collect(),
        })
        .collect())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const MIXIN_KEY: &str = "ea1db124af3c7062474693fa704f4ff8";

    #[test]
    fn detail_request_is_signed_and_carries_the_fixed_relation_from() {
        let req = bgm_detail_request("MA12345", MIXIN_KEY, 1_700_000_000);
        let keys: Vec<&str> = req.query.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["music_id", "relation_from", "wts", "w_rid"]);
    }

    #[test]
    fn achievement_concatenates_the_array_then_the_two_rank_fields() {
        let body = r#"{"code":0,"message":"0","data":{
            "achievement":["热歌榜第 1"],
            "music_rank":"音乐榜第 2",
            "recreation_rank":"二创榜第 3"
        }}"#;
        let d = parse_bgm_detail(body).expect("应解析成功");
        assert_eq!(
            d.achievement,
            ["热歌榜第 1", "音乐榜第 2", "二创榜第 3"],
            "顺序就是界面排标签的顺序"
        );
    }

    #[test]
    fn missing_rank_fields_do_not_leave_holes() {
        let body = r#"{"code":0,"message":"0","data":{"achievement":["只有这一个"]}}"#;
        let d = parse_bgm_detail(body).expect("应解析成功");
        assert_eq!(d.achievement, ["只有这一个"]);
    }

    #[test]
    fn song_heat_is_reversed_into_ascending_time() {
        let body = r#"{"code":0,"message":"0","data":{"hot_song_heat":{"last_heat":9,
            "song_heat":[{"date":20240103,"heat":3},{"date":20240102,"heat":2},{"date":20240101,"heat":1}]}}}"#;
        let d = parse_bgm_detail(body).expect("应解析成功");
        let heat = d.hot_song_heat.expect("有曲线");
        assert_eq!(
            heat.song_heat.iter().map(|p| p.date).collect::<Vec<_>>(),
            [20240101, 20240102, 20240103]
        );
    }

    #[test]
    fn missing_mv_cid_becomes_zero_like_the_baseline() {
        let d = parse_bgm_detail(r#"{"code":0,"message":"0","data":{}}"#).expect("应解析成功");
        assert_eq!(d.mv_cid, 0);
    }

    #[test]
    fn recommend_labels_are_flattened_to_names() {
        let body = r#"{"code":0,"message":"0","data":{"list":[
            {"bvid":"BV1","label_list":[{"name":"鬼畜"},{"other":"忽略"},{"name":"音MAD"}]}
        ]}}"#;
        let list = parse_bgm_recommend(body).expect("应解析成功");
        assert_eq!(
            list[0].labels,
            ["鬼畜", "音MAD"],
            "没有 name 的标签丢掉，不是空串"
        );
    }

    #[test]
    fn missing_recommend_list_is_empty_not_an_error() {
        let list = parse_bgm_recommend(r#"{"code":0,"message":"0","data":{}}"#).expect("不是错误");
        assert!(list.is_empty());
    }
}
