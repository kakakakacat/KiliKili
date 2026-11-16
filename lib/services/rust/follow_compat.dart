import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/dynamics/up.dart';
import 'package:kilikili/services/rust/rust_misc.dart' show FollowList;

/// 关注列表 → 动态页「全部 up 主」容器的转换（#66 起，S6 #56 / S7 #57 收窄）。
///
/// ## 本文件还剩什么
///
/// `FollowData` / `FollowItemModel` 的六个生产者现在**全部**在 Rust，两个
/// 模型连同它们的 `fromJson` 已经删除，列表页直接消费 [FollowList]。剩下
/// 这一条转换是给**动态页**的：它的「全部 up 主」栏用 `FollowUpModel`
/// 装 [UpItem]，而 `UpItem` 还有另一个生产者在
/// `lib/models/dynamics/up.dart`（动态自己的 `up_list`），那一片没迁完
/// 之前删不掉（issue #57 的说明）。
///
/// 所以这里**不再**是「typed 直连的过渡适配」，而是一次普通的容器转换：
/// 动态那几片迁完、`UpItem` 随之消失的那个 PR 把本文件一起删掉。
///
/// 反方向不提供：Dart 侧永远不该去造一个 Rust 结构来冒充取数结果。
extension RustFollowListCompat on FollowList {
  /// 动态页的「全部 up 主」用的是关注列表接口，只是换个容器。
  FollowUpModel toFollowUpModel() => FollowUpModel.fromUpItems(
    list
        .map((e) => UpItem(mid: e.mid, uname: e.uname, face: e.face))
        .toList(),
  );
}

/// 结果状态的映射。失败与加载态不带载荷，原样透出即可
/// （`Error` / `Loading` 都是 `LoadingState<Never>`，对任何 `T` 都是子类型）。
extension RustFollowStateCompat on LoadingState<FollowList> {
  LoadingState<FollowUpModel> toFollowUpState() => switch (this) {
    Success(:final response) => Success(response.toFollowUpModel()),
    final Error e => e,
    final Loading l => l,
  };
}
