//! 音乐页（BGM 详情与相关推荐）的领域实体（S15 / issue #66）。
//!
//! 对应基线 `lib/models_new/music/`。这里的字段是**全量**映射：音乐页
//! 把详情里的每一项都显示出来了（标题、原唱、专辑、艺人、热度曲线、
//! 成就标签、评论数、发行日期），没有「存下来没人读」的部分。

use serde::Serialize;

/// 一首 BGM 的详情。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct MusicDetail {
    pub music_title: Option<String>,
    pub origin_artist: Option<String>,
    pub origin_artist_list: Option<String>,
    pub mv_aid: Option<i64>,
    /// 基线缺失时填 0（`json["mv_cid"] ?? 0`），这里保留该行为：调用点
    /// 直接拿它去跳播放页，`null` 与 0 在那里是同一回事。
    pub mv_cid: i64,
    pub mv_bvid: Option<String>,
    pub mv_cover: Option<String>,
    pub wish_listen: Option<bool>,
    pub wish_count: Option<i64>,
    pub music_source: Option<String>,
    pub album: Option<String>,
    pub artists_list: Vec<Artist>,
    pub listen_pv: Option<i64>,
    /// 成就标签。基线把 `achievement` 数组与两个单独的榜单字段
    /// （`music_rank`、`recreation_rank`）拼成一个列表，顺序即此，
    /// 界面按顺序排标签——顺序是展示约定，必须保持。
    pub achievement: Vec<String>,
    pub hot_song_heat: Option<HotSongHeat>,
    pub music_comment: Option<MusicComment>,
    pub music_relation: Option<i64>,
    pub music_publish: Option<String>,
}

/// 参与这首歌的一位艺人。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Artist {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    /// 身份（作词、作曲、演唱……），界面显示成 `身份: 名字`。
    pub identity: Option<String>,
}

/// 热度曲线。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct HotSongHeat {
    pub last_heat: Option<i64>,
    /// **已反转**成时间升序。服务端给的是降序，基线在模型里就 reverse 了，
    /// 曲线图按列表顺序画，顺序错了图就是反的。
    pub song_heat: Vec<SongHeat>,
}

/// 曲线上的一个点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SongHeat {
    /// 形如 20240131 的日期数字，不是时间戳。
    pub date: i64,
    pub heat: i64,
}

/// 这首歌的评论区入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct MusicComment {
    pub nums: Option<i64>,
    pub oid: Option<i64>,
    pub page_type: Option<i64>,
}

/// 相关推荐里的一条视频。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct BgmRecommend {
    pub bvid: Option<String>,
    pub cid: Option<i64>,
    pub cover: Option<String>,
    pub title: Option<String>,
    pub up_nick_name: Option<String>,
    pub play: Option<i64>,
    pub danmu: Option<i64>,
    /// 秒。
    pub duration: Option<i64>,
    /// 标签名。基线是 `[{name}]`，这里摊平成字符串——除了 `name`
    /// 服务端没给别的，保留一层对象只会让调用点多一次解包。
    pub labels: Vec<String>,
}
