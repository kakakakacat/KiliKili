import 'package:kilikili/models/horizontal_video_model.dart';
import 'package:kilikili/models/model_video.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;

/// 网页端空间视频列表的**界面侧视图模型**（S6 #56）。
///
/// ## 为什么它存在
///
/// 投稿搜索与合集/列表的取数已经整条迁进 Rust，这个类不再有 `fromJson`。
/// 留着它，是因为它要套进 [HorizontalVideoModel] 这个**界面多态层**：
/// `VideoCardH` 与 `pushVideoH` 对好几种列表模型通用，而那个基类还有两个
/// **尚未迁移**的生产者——`HotVideoItemModel`（热门/推荐）与
/// `SearchVideoItemModel`（搜索页，走 `search_compat.dart`）。
///
/// 按 ADR-011 决策六，共享的运行时模型要等它的**所有**生产者迁完才能删；
/// 在那之前，这里做一次 Rust 结构 → 界面模型的单向适配。两个生产者都迁完
/// 之后，本文件与 `search_compat.dart` 应当一起删除。
///
/// ## 角标文案留在这里
///
/// Rust 只给 [rust.SpaceWebBadge] 这个类别（ADR-011 决策二）：
/// 「课堂」「充电专属」「合作」是界面措辞，换一个宿主框架也得重写。
class SpaceWebVideoModel extends HorizontalVideoModel {
  SpaceWebVideoModel.fromRust(rust.SpaceWebVideo raw) {
    aid = raw.aid?.toInt();
    bvid = raw.bvid;
    cover = raw.cover;
    title = raw.title ?? '';
    desc = raw.desc;
    pubdate = raw.pubdate?.toInt();
    duration = raw.duration?.toInt() ?? 0;
    seasonId = raw.seasonId?.toInt();
    redirectUrl = raw.redirectUrl;
    progress = raw.progress?.toInt();
    isPugv = raw.badge == rust.SpaceWebBadge.lesson;
    badge = switch (raw.badge) {
      rust.SpaceWebBadge.lesson => '课堂',
      rust.SpaceWebBadge.charging => '充电专属',
      rust.SpaceWebBadge.union => '合作',
      null => null,
    };
    // 合集与列表不给作者，两个字段都会是空的；基线在那里写死空串。
    owner = _SpaceWebOwner(raw.ownerMid?.toInt(), raw.ownerName ?? '');
    stat = _SpaceWebStat(raw.view?.toInt(), raw.danmaku?.toInt());
  }
}

class _SpaceWebOwner extends BaseOwner {
  _SpaceWebOwner(int? mid, String name) {
    this.mid = mid;
    this.name = name;
  }
}

class _SpaceWebStat extends BaseStat {
  _SpaceWebStat(int? view, int? danmu) {
    this.view = view;
    this.danmu = danmu;
  }
}
