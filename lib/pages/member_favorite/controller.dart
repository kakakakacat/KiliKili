import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_data_controller.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/services/rust/rust_user.dart'
    show RustUser, SubItem, SubList;
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

/// 个人空间的「收藏」页（S8 / issue #58）。
///
/// 首屏一次拿两组（收藏夹、订阅的合集），翻页则是两条各自的接口。基线把
/// 后两条直接写成 `Request().get` 塞在控制器里，连同它们解析用的
/// `SpaceFavItemModel` 一起随本片迁走——那个模型是本片要删的，留着它就得
/// 留着这两段裸请求。
///
/// 两组的列表项都是 [SubItem]：基线的 `SpaceFavItemModel` 继承自
/// `SubItemModel`，多出来的三个字段这一页一个都没用到。
class MemberFavoriteCtr
    extends CommonDataController<List<SpaceFavGroup>?, List<SpaceFavGroup>?> {
  MemberFavoriteCtr({
    required this.mid,
  });

  final int mid;

  late int favPage = 2;
  bool _favExpand = true;
  final RxBool favEnd = true.obs;
  final Rx<SpaceFavGroup> favState = const SpaceFavGroup(list: []).obs;

  late int subPage = 2;
  bool _subExpand = true;
  final RxBool subEnd = true.obs;
  final Rx<SpaceFavGroup> subState = const SpaceFavGroup(list: []).obs;

  bool isExpand(bool isFav) {
    return isFav ? _favExpand : _subExpand;
  }

  void setExpand(bool isFav) {
    if (isFav) {
      flag = _favExpand;
      _favExpand = !_favExpand;
    } else {
      _subExpand = !_subExpand;
    }
  }

  bool flag = false;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<void> onRefresh() {
    favPage = 2;
    subPage = 2;
    return super.onRefresh();
  }

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<List<SpaceFavGroup>?> response,
  ) {
    try {
      final res = response.response!;
      favState.value = res.first;
      subState.value = res[1];

      favEnd.value = (res.first.count ?? -1) <= res.first.list.length;
      subEnd.value = (res[1].count ?? -1) <= res[1].list.length;
    } catch (e) {
      if (kDebugMode) debugPrint(e.toString());
    }
    loadingState.value = response;
    return true;
  }

  Future<void> userFavFolder() => _loadMore(
    state: favState,
    end: favEnd,
    fetch: () => RustFav.spaceFavFolderPage(mid: mid, pn: favPage, ps: 20),
    onPage: () => favPage++,
  );

  Future<void> userSubFolder() => _loadMore(
    state: subState,
    end: subEnd,
    fetch: () => RustUser.userSubFolder(mid: mid, pn: subPage, ps: 20),
    onPage: () => subPage++,
  );

  /// 两组翻页共用：结果形状相同，只有接口与页码不同。
  ///
  /// FRB 的结构是不可变的，所以这里是**整组换一个新值**而不是往原列表里
  /// `addAll`——后者在基线能写，是因为那边的模型字段可改。
  Future<void> _loadMore({
    required Rx<SpaceFavGroup> state,
    required RxBool end,
    required Future<LoadingState<SubList>> Function() fetch,
    required VoidCallback onPage,
  }) async {
    final res = await fetch();
    if (res case Success(:final response)) {
      onPage();
      if (response.list.isEmpty) {
        end.value = true;
        return;
      }
      end.value = response.hasMore == false;
      final current = state.value;
      state.value = SpaceFavGroup(
        id: current.id,
        name: current.name,
        count: current.count,
        list: [...current.list, ...response.list],
      );
    } else {
      end.value = true;
      res.toast();
    }
  }

  /// 从某一组里摘掉一项（收藏夹详情页里把它删了）。
  void removeFav(SubItem item) {
    final current = favState.value;
    favState.value = SpaceFavGroup(
      id: current.id,
      name: current.name,
      count: current.count,
      list: current.list.where((e) => e.id != item.id).toList(),
    );
  }

  @override
  Future<LoadingState<List<SpaceFavGroup>?>> customGetData() =>
      RustFav.spaceFav(mid);
}
