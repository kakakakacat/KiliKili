import 'package:kilikili/models_new/pgc/pgc_info_model/episode.dart';
import 'package:kilikili/models_new/pgc/pgc_info_model/stat.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/rust/api/pgc.dart';

// 番剧详情的**单向适配**（ADR-011 决策六）。
//
// S10 第二批把 `pgcInfo` / `pugvInfo` 的解析迁到了 Rust，但有两个东西
// 没法跟着走，各有各的理由：
//
// ## 1. 剧集：共享基类还有四个未迁的生产者
//
// `pgc.EpisodeItem` 继承 `BaseEpisodeItem`，剧集面板
// （`lib/pages/episode_panel/view.dart`）按基类存 `List<BaseEpisodeItem>`
// 并用 `case pgc.EpisodeItem item:` / `case ugc.EpisodeItem item:` 分流。
// 那个基类还有四个生产者没迁：UGC 合集剧集、分 P（`Part`）、媒体列表、
// 互动视频选项。按决策六，共享模型要等**所有**生产者迁完才能删。
//
// **随哪个切片删除**：`BaseEpisodeItem` 的最后一个生产者迁完时——即
// UGC 视频详情（`VideoDetailData`，仍是 JSON 往返路径，EPIC #67 留到
// Phase 2 统一评估）落地之后。届时 [toDartEpisode] 连同
// `pgc_info_model/episode.dart` 一起删。
//
// ## 2. 统计：点完赞的计数是客户端状态
//
// `PgcIntroController` 在拿到详情之后会**就地改** `stat`：
// `stat.like += 1`、`stat.favorite = max(1, ...)`。FRB 的结构不可变，
// 所以这里产出一个可变的 `PgcStat`。
//
// 这不是权宜之计掩盖的设计问题——它本来就是两种性质的数据混在同一个
// 字段里：服务端返回的计数，和用户本次操作后的本地计数。`StatDetail`
// 是 UGC 与 PGC 的共享契约（`CommonIntroController.getStat()`），拆开
// 要连 UGC 一起动，不在本切片范围内。
//
// ## 适配是单向的
//
// 只有 Rust → Dart，没有反向。Dart 这边改过的 `stat` 不会写回 FRB 结构，
// 也不该写回：FRB 结构代表的是**服务端那一刻说了什么**。
/// 把 Rust 的剧集转成剧集面板要的共享基类子类。
///
/// `width` / `height` 已在 Rust 侧按 `rotate` 校正过，所以这里用
/// `Dimension` 的普通构造而不是 `fromJson`——再走一次 `fromJson` 会把
/// 已经校正过的值再换一次。
EpisodeItem toDartEpisode(PgcEpisode e) => EpisodeItem(
  aid: e.aid,
  badge: e.badge,
  bvid: e.bvid,
  cid: e.cid,
  cover: e.cover,
  dimension: e.width == null && e.height == null
      ? null
      : Dimension(width: e.width, height: e.height),
  duration: e.duration,
  epId: e.epId,
  from: e.from,
  id: e.id,
  link: e.link,
  longTitle: e.longTitle,
  pubTime: e.pubTime,
  shareCopy: e.shareCopy,
  shareUrl: e.shareUrl,
  showTitle: e.showTitle,
  title: e.title,
  play: e.play,
);

/// 把 Rust 的统计转成可变的 `PgcStat`。
///
/// 详情里没有 `stat` 时返回 null，与基线一致——界面据此整块隐藏。
PgcStat? toDartStat(PgcSeasonStat? s) => s == null
    ? null
    : (PgcStat.fromJson(const <String, dynamic>{})
        ..coin = s.coin
        ..favorite = s.favorite
        ..like = s.like
        ..danmaku = s.danmaku
        ..reply = s.reply
        ..share = s.share
        ..view = s.view);

/// 基线 `PgcInfoModel` 上那几个方法。
///
/// 它们是对已经取回来的数据做的纯遍历，放在 Dart 这边而不是再跨一次
/// FFI：RFC §2.2 要求只暴露粗粒度 API，为一次列表查找跨边界是反过来的。
extension PgcSeasonDetailX on PgcSeasonDetail {
  bool get hasSeasons => seasons.length > 1;

  bool get hasEpisodes => episodes.isNotEmpty;

  bool get hasEpisodesOrSeasons => hasEpisodes || hasSeasons;

  bool get hasSections => sections.isNotEmpty;

  /// 定位要播的那一集，返回 `(剧集, 花絮组下标)`。
  ///
  /// 花絮组下标为 null 表示这一集在正片里。逐行照搬基线
  /// `PgcInfoModel.findEpisode`，包括它的回退顺序：
  ///
  /// 1. 不给 epId 时用上次看到的那一集（`user_status.progress.last_ep_id`）；
  /// 2. 仍然没有，就取正片第一集，正片为空再取第一组花絮的第一集；
  /// 3. 给了 epId 但正片与花絮里都找不到，**同样**回退到第 2 步——
  ///    这不是遗漏：epId 可能来自一条已经下架的分享链接，此时打开第一集
  ///    比停在空白页合理。
  (PgcEpisode? episode, int? sectionIndex) findEpisode([int? epId]) {
    epId ??= userStatus?.lastEpId;

    if (epId == null) {
      return _firstAvailable();
    }

    for (final e in episodes) {
      if (e.epId == epId) return (e, null);
    }

    for (final (index, section) in sections.indexed) {
      for (final e in section.episodes) {
        if (e.epId == epId) return (e, index);
      }
    }

    return _firstAvailable();
  }

  (PgcEpisode?, int?) _firstAvailable() {
    if (episodes.isNotEmpty) return (episodes.first, null);
    final first = sections.firstOrNull?.episodes.firstOrNull;
    return first == null ? const (null, null) : (first, 0);
  }
}
