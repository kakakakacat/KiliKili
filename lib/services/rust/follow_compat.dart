import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/dynamics/up.dart';
import 'package:kilikili/models/model_avatar.dart';
import 'package:kilikili/models_new/follow/data.dart';
import 'package:kilikili/models_new/follow/list.dart';
import 'package:kilikili/services/rust/rust_misc.dart' show FollowList, FollowUser;

/// typed 直连的过渡适配：Rust 的 [FollowList] → 基线的 `FollowData`（#66）。
///
/// ## 为什么还需要它
///
/// 迁移的边界是**网络边界**（ADR-011 决策六）。`/x/relation/followings` 与
/// `/x/relation/fans` 这次整条进了 Rust，但运行时模型 `FollowData` /
/// `FollowItemModel` 还有四个生产者留在 Dart：
///
/// - `UserHttp.followedUp`、`UserHttp.sameFollowing`（S7 / #57）；
/// - `MemberHttp.followUpGroup`、`MemberHttp.getfollowSearch`（S6 / #56）。
///
/// 而且三个列表页的控制器是同一组泛型基类
/// （`CommonListController<FollowData, FollowItemModel>`），只换其中两个
/// 生产者的类型参数，剩下的那几个就编译不过。所以这一层要留到 S6 与 S7
/// 把另外四条也迁完——**那个 PR 应当连本文件一起删掉**，并把
/// `lib/models_new/follow/` 与 `FollowItemModel extends UpItem` 这条继承
/// 关系一并清理。
///
/// 反方向不提供：Dart 侧永远不该去造一个 Rust 结构来冒充取数结果。
extension RustFollowUserCompat on FollowUser {
  FollowItemModel toItemModel() => FollowItemModel(
    mid: mid,
    attribute: attribute,
    uname: uname,
    face: face,
    sign: sign,
    // `BaseOfficialVerify` 只有 fromJson 一个构造函数（上游的写法），这里
    // 没有「顺手给它加个普通构造函数」——那会改到上游既有文件，每次同步
    // 都要再解一次冲突，而本文件本来就是要删的。
    officialVerify: officialVerify == null
        ? null
        : BaseOfficialVerify.fromJson({
            'type': officialVerify!.kind,
            'desc': officialVerify!.desc,
          }),
  );
}

extension RustFollowListCompat on FollowList {
  FollowData toFollowData() => FollowData(
    list: list.map((e) => e.toItemModel()).toList(),
    total: total,
  );

  /// 动态页的「全部 up 主」用的是同一条接口，但装进 `FollowUpModel`。
  FollowUpModel toFollowUpModel() => FollowUpModel.fromUpItems(
    list.map((e) => e.toItemModel()).toList(),
  );
}

/// 结果状态的映射。失败与加载态不带载荷，原样透出即可
/// （`Error` / `Loading` 都是 `LoadingState<Never>`，对任何 `T` 都是子类型）。
extension RustFollowStateCompat on LoadingState<FollowList> {
  LoadingState<T> _map<T>(T Function(FollowList) f) => switch (this) {
    Success(:final response) => Success(f(response)),
    final Error e => e,
    final Loading l => l,
  };

  LoadingState<FollowData> toFollowDataState() => _map((e) => e.toFollowData());

  LoadingState<FollowUpModel> toFollowUpState() =>
      _map((e) => e.toFollowUpModel());
}
