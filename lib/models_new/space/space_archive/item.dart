import 'package:kilikili/models/model_owner.dart';
import 'package:kilikili/models/model_video.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;

/// 投稿列表里的一条，**界面侧的视图模型**（S6 #56）。
///
/// ## 为什么它没跟着切片一起删
///
/// 取数与解析已经整条迁进 Rust（`bili_domain::space::SpaceArchiveItem`），
/// 这个类不再有 `fromJson`，只是把 Rust 的结构套进
/// [`BaseSimpleVideoItemModel`] 这个**界面多态层**——视频卡片的长按菜单
/// （`lib/common/widgets/video_popup_menu.dart`）对十来种列表模型通用，
/// 靠的就是这个基类。同 S3 留下 `HotVideoItemModel` 的理由（ADR-011
/// 决策六）。
///
/// **随基类一起消失**：换掉 Flutter 宿主时 `BaseSimpleVideoItemModel` 这
/// 一层本身就不存在了，那时直接消费 Rust 结构。
///
/// ## 作者 mid 是 null，这是真的
///
/// 服务端在这一族只给 `author` 昵称。基线在这里造了
/// `Owner(mid: 0, name: author)` 这个假 owner，并在长按菜单里用
/// `videoItem is! SpaceArchiveItem` 把「访问/拉黑作者」特判掉——因为那个
/// 0 是假的。现在两处一起去掉了：这里如实给 `mid: null`，菜单改判
/// `owner.mid == null`，对任何缺作者 id 的列表都成立。
class SpaceArchiveItem extends BaseSimpleVideoItemModel {
  SpaceArchiveItem.fromRust(this.raw) {
    title = raw.title ?? '';
    cover = raw.cover;
    bvid = raw.bvid;
    cid = raw.cid?.toInt();
    duration = raw.duration.toInt();
    owner = Owner(name: raw.author);
    stat = _SpaceArchiveStat(raw);
  }

  /// Rust 给的那条原始结构。卡片要的 `goto` / `param` / 角标都从它读，
  /// 不再往这个类上抄一遍。
  final rust.SpaceArchiveItem raw;

  String? get uri => raw.uri;
  String? get param => raw.param;
  String? get goto => raw.goto;
  List<String> get badges => raw.badges;
  bool get isSteins => raw.isSteins == true;
  bool get isCooperation => raw.isCooperation == true;
  bool get isPgc => raw.isPgc == true;
  bool get isPugv => raw.isPugv == true;
  String? get publishTimeText => raw.publishTimeText;
  int? get seasonMtime => raw.seasonMtime?.toInt();
  int? get historyProgress => raw.history?.progress?.toInt();
  int? get historyDuration => raw.history?.duration?.toInt();
  String? get styles => raw.styles;
  String? get label => raw.label;
}

class _SpaceArchiveStat extends BaseStat {
  _SpaceArchiveStat(rust.SpaceArchiveItem raw) {
    view = raw.play?.toInt();
    danmu = raw.danmaku?.toInt();
  }
}
