import 'package:kilikili/models/horizontal_video_model.dart';
import 'package:kilikili/models/model_owner.dart';
import 'package:kilikili/models/model_video.dart';
import 'package:kilikili/services/rust/rust_misc.dart' show MatchContest;
import 'package:kilikili/services/rust/rust_search.dart'
    show
        SearchActivityItem,
        SearchArticleItem,
        SearchLiveItem,
        SearchPgcItem,
        SearchUserCard,
        SearchUserItem;

/// 搜索结果的**列表容器**。
///
/// ## 这里还剩什么，为什么
///
/// S9（issue #59）把搜索接口迁到了 Rust 的 typed 直连，`fromJson` 那一层
/// 协议模型随之删除——用户、直播间、番剧、专栏、活动的条目现在直接是
/// `lib/rust/api/search.dart` 里的 FRB 类型。
///
/// 留下来的只有两类：
///
/// 1. **UI 容器**（本文件的 `Search*Data`）。它们不解析任何东西，只是
///    `CommonListController<R, T>` 这套分页框架要求的形状（`numResults`
///    + `list`）。那是界面基础设施，不是协议模型。
/// 2. **[SearchVideoItemModel]**。它继承 `HorizontalVideoModel`，而那个
///    共享运行时模型还有三个未迁的生产者（`HotVideoItemModel`、
///    `VListItemModel`、`SeasonArchive`）。按 ADR-011 决策六，共享模型要
///    等它的**所有**生产者迁完才能删，所以这里保留一个单向适配
///    （`lib/services/rust/search_compat.dart`）。
abstract class SearchNumData<T> {
  SearchNumData({
    this.numResults,
    this.list,
  });

  int? numResults;
  List<T>? list;
}

/// 视频搜索结果，以及综合搜索的其余分区。
///
/// 综合搜索把视频、用户、番剧、活动、赛事装在同一次响应里，基线因此让
/// 视频容器兼做综合容器。保持这个形状：拆开的话 `SearchPanelController`
/// 的泛型参数要跟着分裂，而那是纯粹的界面代价。
class SearchVideoData extends SearchNumData<SearchVideoItemModel> {
  SearchVideoData({
    super.numResults,
    super.list,
    this.searchUser,
    this.searchMedia,
    this.searchActivity,
    this.searchEsports,
  });

  /// 以下四项只有综合搜索才有值。
  List<SearchUserCard>? searchUser;
  List<SearchPgcItem>? searchMedia;
  List<SearchActivityItem>? searchActivity;

  /// 赛事卡片。S15（#66）把赛事详情也迁了之后，这里直接是 typed 结构，
  /// 不再经过 `SearchEsports` 那层 JSON 绕路。
  List<MatchContest>? searchEsports;
}

class SearchUserData extends SearchNumData<SearchUserItem> {
  SearchUserData({super.numResults, super.list});
}

class SearchLiveData extends SearchNumData<SearchLiveItem> {
  SearchLiveData({super.numResults, super.list});
}

class SearchPgcData extends SearchNumData<SearchPgcItem> {
  SearchPgcData({super.numResults, super.list});
}

class SearchArticleData extends SearchNumData<SearchArticleItem> {
  SearchArticleData({super.numResults, super.list});
}

/// 搜索到的视频条目。
///
/// **这个类是共享运行时模型的适配点，不是协议模型。** 它继承
/// `HorizontalVideoModel`，才能进那些按基类接收的通用列表组件
/// （`lib/pages/member_video_web/base/view.dart` 等）。解析已经在 Rust，
/// 构造见 `lib/services/rust/search_compat.dart` 的
/// `RustSearchVideoCompat.toItemModel()`。
///
/// `HorizontalVideoModel` 的另外三个生产者迁完之后，这个类连同基类一起
/// 重新评估（ADR-011 决策六）。
class SearchVideoItemModel extends HorizontalVideoModel {
  SearchVideoItemModel();

  int? id;
  String? arcurl;
  String? tag;
  int? ctime;

  @override
  int? get seasonId => aid;
}

/// 条目统计。字段取自搜索接口的扁平形状，与 `Stat` 的嵌套形状不同。
class SearchStat extends BaseStat {
  SearchStat();

  /// 收藏数。
  int? favorite;

  /// 评论数。
  int? reply;
}

/// UP 主。搜索接口把它扁平在条目上（`mid` / `author` / `upic`），
/// 不是嵌套的 `owner` 对象。
class SearchOwner extends Owner {
  SearchOwner();
}
