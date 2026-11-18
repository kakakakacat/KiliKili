import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/services/rust/rust_video_write.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

/// 追番 / 追剧列表（S8 / issue #58）。
///
/// 选中态改用 [IdMultiSelectMixin]，理由同笔记页：列表项来自 Rust，
/// 不可就地改。按 `seasonId` 记——同一个列表里它唯一。
///
/// 追番状态的那三条写操作（加 / 删 / 改）属于 S4，已经在
/// [RustVideoWrite]，这里只是调用方。
class FavPgcController extends CommonListController<FavPgcList, FavPgcItem>
    with IdMultiSelectMixin<FavPgcList, FavPgcItem, int> {
  final int type;
  final int followStatus;

  FavPgcController(this.type, this.followStatus);

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  final RxBool allSelected = false.obs;

  @override
  final RxBool enableMultiSelect = false.obs;

  @override
  final RxInt rxCount = 0.obs;

  /// 没有 `seasonId` 的项给一个不会与真 id 相撞的替身，免得多项共用 0
  /// 被一起选中。
  @override
  int idOf(FavPgcItem item) => item.seasonId ?? -identityHashCode(item);

  @override
  void handleSelect({bool checked = false, bool disableSelect = true}) {
    allSelected.value = checked;
    super.handleSelect(checked: checked, disableSelect: disableSelect);
  }

  @override
  void onSelect(FavPgcItem item) {
    super.onSelect(item);
    allSelected.value =
        rxCount.value == (loadingState.value.dataOrNull?.length ?? 0);
  }

  @override
  List<FavPgcItem>? getDataList(FavPgcList response) => response.list;

  @override
  Future<LoadingState<FavPgcList>> customGetData() => RustFav.favPgc(
    type: type,
    followStatus: followStatus,
    pn: page,
  );

  void onDisable() {
    if (checkedCount != 0) {
      handleSelect();
    }
    enableMultiSelect.value = false;
  }

  // 取消追番
  Future<void> pgcDel(int index, int? seasonId) async {
    final result = await RustVideoWrite.pgcDel(seasonId: seasonId);
    if (result case Success(:final response)) {
      loadingState
        ..value.data!.removeAt(index)
        ..refresh();
      SmartDialog.showToast(response);
    } else {
      result.toast();
    }
  }

  @override
  void onRemove() {
    assert(false, 'call onUpdateList');
  }

  Future<void> onUpdateList(int followStatus) async {
    final removeList = allChecked.toList();
    if (removeList.isEmpty) return;
    final res = await RustVideoWrite.pgcUpdate(
      seasonIds: removeList.map((item) => item.seasonId).join(','),
      status: followStatus,
    );
    if (res case Success(:final response)) {
      _moveTo(followStatus, removeList);
      afterDeleteIds(removeList.map(idOf).toSet());
      allSelected.value = false;
      SmartDialog.showToast(response);
    } else {
      res.toast();
    }
  }

  Future<void> onUpdate(int index, int followStatus, int? seasonId) async {
    final res = await RustVideoWrite.pgcUpdate(
      seasonIds: seasonId.toString(),
      status: followStatus,
    );
    if (res case Success(:final response)) {
      final list = loadingState.value.data!;
      final item = list.removeAt(index);
      loadingState.refresh();
      _moveTo(followStatus, [item]);
      SmartDialog.showToast(response);
    } else {
      res.toast();
    }
  }

  /// 把改完状态的几项挪到对应状态的那个 tab 的列表头部。
  ///
  /// 那个 tab 可能根本没被打开过（`Get.find` 抛），这不是错误——下次打开
  /// 时它自己会拉一遍。
  void _moveTo(int followStatus, List<FavPgcItem> items) {
    try {
      final ctr = Get.find<FavPgcController>(tag: '$type$followStatus');
      if (ctr.loadingState.value case Success(:final response)) {
        response?.insertAll(0, items);
        ctr
          ..loadingState.refresh()
          ..selectedIds.clear()
          ..rxCount.value = 0
          ..allSelected.value = false;
      }
    } catch (e) {
      if (kDebugMode) debugPrint('fav pgc onUpdate: $e');
    }
  }
}
