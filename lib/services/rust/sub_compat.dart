import 'package:kilikili/models/model_owner.dart';
import 'package:kilikili/models_new/fav/fav_detail/cnt_info.dart';
import 'package:kilikili/models_new/sub/sub/list.dart';
import 'package:kilikili/services/rust/rust_user.dart' show SubItem;

/// typed 直连的过渡适配：Rust 的 [SubItem] → 基线的 `SubItemModel`（S7 #57）。
///
/// ## 为什么还需要它
///
/// `SubItemModel` 有**两个**生产者，分属两条切片：
///
/// - `UserHttp.userSubFolder`（「我的订阅」列表）—— 本切片，已迁进 Rust；
/// - `SubDetailData.fromJson`（`lib/models_new/sub/sub_detail/data.dart`，
///   由 `FavHttp.favSeasonList` 喂数据）—— **收藏夹那条切片**，还在 Dart。
///
/// 两个生产者的结果喂进同一批界面：订阅列表页点进去是合集详情页，详情页
/// 的头部既可能来自列表页传过来的那一项，也可能来自详情接口自己回的
/// `info`（`SubDetailController.subInfo` 两处都赋值）。只迁其中一个就会
/// 出现「同一个头部要同时吃 typed 结构和 Dart 模型」。
///
/// 按 S3 定下的「按共享模型的生产者分批，最后迁完的那一片才能删模型」，
/// **收藏夹那片迁完 `favSeasonList` 的那个 PR 应当连本文件与
/// `lib/models_new/sub/sub/list.dart` 一起删**。
///
/// 反方向不提供：Dart 侧永远不该去造一个 Rust 结构来冒充取数结果。
extension RustSubItemCompat on SubItem {
  SubItemModel toItemModel() => SubItemModel(
    id: id,
    fid: fid,
    mid: mid,
    attr: attr,
    title: title,
    cover: cover,
    // `Owner` 只有 fromJson 一个构造函数（上游的写法），这里没有「顺手给它
    // 加个普通构造函数」——那会改到上游既有文件，每次同步都要再解一次冲突，
    // 而本文件本来就是要删的。
    upper: Owner.fromJson({
      'mid': upperMid,
      'name': upperName,
      'face': upperFace,
    }),
    coverType: coverType,
    intro: intro,
    ctime: ctime,
    mtime: mtime,
    state: state,
    favState: favState,
    mediaCount: mediaCount,
    viewCount: viewCount,
    type: kind,
    cntInfo: CntInfo(play: playCount, danmaku: collectCount),
  );
}
