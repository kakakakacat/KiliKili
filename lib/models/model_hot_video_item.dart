import 'package:kilikili/models/horizontal_video_model.dart';
import 'package:kilikili/models/model_owner.dart';
import 'package:kilikili/models/model_video.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/pages/common/multi_select/base.dart';
import 'package:kilikili/rust/api/video_feed.dart' show FeedStat, FeedVideoItem;

/// 热门、排行、相关推荐、每周必看、入站必刷共用的列表条目。
///
/// **这个类已经不解析协议**：S3（issue #53）把那五条接口的请求、解析与
/// 过滤都搬到了 Rust，`fromJson` 随之删除，只剩 [fromRust] 一个入口。
///
/// 它仍然留在 Dart，是因为迁移的边界是**网络边界**（ADR-011 决策六）：
/// 列表控件依赖 `HorizontalVideoModel` 这套 UI 模型与 [MultiSelectData]
/// 的多选状态，而那套基类还有搜索、动态等未迁移的生产者。等它们都迁完，
/// 这个类连同基类一起删。
class HotVideoItemModel extends HorizontalVideoModel with MultiSelectData {
  int? videos;
  int? tid;
  String? tname;
  int? copyright;
  int? ctime;
  int? state;
  String? firstFrame;
  String? pubLocation;

  /// 由 Rust 的 typed 结构构造。
  ///
  /// 这里只做两件 UI 的事：把「没有时长」还原成基线的 -1，以及把角标的
  /// **来源**翻成文案。协议层面的归一（竖屏宽高互换、播放量不公开、
  /// mid 的字符串形态）都已在 Rust 侧完成。
  HotVideoItemModel.fromRust(FeedVideoItem i) {
    aid = i.aid?.toInt();
    cid = i.cid?.toInt();
    bvid = i.bvid;
    videos = i.videos?.toInt();
    tid = i.tid?.toInt();
    tname = i.tname;
    copyright = i.copyright?.toInt();
    cover = i.cover;
    title = i.title ?? '';
    pubdate = i.pubdate?.toInt();
    ctime = i.ctime?.toInt();
    desc = i.desc;
    state = i.state?.toInt();
    // 基线用 -1 表示「没有时长」，列表控件据此不显示时长角标。
    duration = i.duration?.toInt() ?? -1;
    owner = Owner(
      mid: i.ownerMid?.toInt(),
      name: i.ownerName,
      face: i.ownerFace,
    );
    stat = HotStat.fromRust(i.stat);
    dimension = i.width == null && i.height == null
        ? null
        : Dimension(width: i.width?.toInt(), height: i.height?.toInt());
    firstFrame = i.firstFrame;
    pubLocation = i.pubLocation;
    redirectUrl = i.redirectUrl;
    progress = i.progress;
    // 角标文案属界面（ADR-011 决策二）：Rust 只说这条是哪一类。
    badge = switch (i.badgeKind) {
      'charging' => '充电专属',
      'cooperation' => '合作',
      _ => i.pgcLabel,
    };
  }
}

class HotStat extends Stat {
  int? reply;
  int? favorite;
  num? coin;
  int? share;
  int? nowRank;
  int? hisRank;
  int? dislike;
  int? vt;
  int? vv;

  HotStat.fromRust(FeedStat s) : super.empty() {
    view = s.view?.toInt();
    like = s.like?.toInt();
    danmu = s.danmaku?.toInt();
    reply = s.reply?.toInt();
    favorite = s.favorite?.toInt();
    coin = s.coin;
    share = s.share?.toInt();
    nowRank = s.nowRank?.toInt();
    hisRank = s.hisRank?.toInt();
    dislike = s.dislike?.toInt();
    vt = s.vt?.toInt();
    vv = s.vv?.toInt();
  }
}
