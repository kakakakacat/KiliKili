import 'package:kilikili/models/model_owner.dart';
import 'package:kilikili/models_new/fav/fav_detail/cnt_info.dart';
import 'package:kilikili/models_new/media_list/media_list.dart';
import 'package:kilikili/models_new/media_list/page.dart';
import 'package:kilikili/services/rust/rust_history.dart' show MediaListItem;

/// typed 直连的过渡适配：Rust 的 [MediaListItem] → 基线的
/// `MediaListItemModel`（S7 #57）。
///
/// ## 为什么还需要它
///
/// `MediaListItemModel extends BaseEpisodeItem`，而 `BaseEpisodeItem` 是
/// **播放器分集列表的共用基类**：UGC 分 P（`video_detail/page.dart`）、
/// PGC 剧集（`pgc_info_model/episode.dart`）、互动视频的选项
/// （`video_stein_edgeinfo/choice.dart`）都是它的子类，播放页的续播、
/// 下载面板、分集面板全按它写。
///
/// 把 `medialist` 这一条单独换成 Rust 结构，就要让那套机器同时吃两种类型。
/// 按 S3 定下的「按共享模型的生产者分批」，这里留一层单向适配，**等分集
/// 那几片（UGC 分 P / PGC 剧集）也迁完、`BaseEpisodeItem` 整个消失的那个
/// PR 连同本文件与 `lib/models_new/media_list/` 一起删**。
///
/// 反方向不提供：Dart 侧永远不该去造一个 Rust 结构来冒充取数结果。
extension RustMediaListItemCompat on MediaListItem {
  MediaListItemModel toItemModel() => MediaListItemModel(
    aid: aid,
    intro: intro,
    cntInfo: CntInfo(play: play, danmaku: danmaku),
    cover: cover,
    duration: duration,
    // `MediaListItemModel.cid` 是个取 `pages.first.id` 的 getter（覆盖了
    // 基类的字段），所以这里必须把 cid 放回 `pages` 里——直接传 `cid:` 会
    // 被那个 getter 盖掉，表现是播放器拿不到 cid 而重新去查一次。
    pages: firstPageCid == null ? null : [Page(id: firstPageCid)],
    title: title,
    type: kind,
    // `Owner` 只有 fromJson 一个构造函数（上游的写法），这里没有「顺手给它
    // 加个普通构造函数」——那会改到上游既有文件，每次同步都要再解一次冲突，
    // 而本文件本来就是要删的。
    upper: Owner.fromJson({
      'mid': upperMid,
      'name': upperName,
      'face': upperFace,
    }),
    bvid: bvid,
    badge: badge,
  );
}
