import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';

/// 笔记列表（S8 / issue #58）。
///
/// 选中态改用 [IdMultiSelectMixin]：列表项来自 Rust，字段是 final 的，
/// 没法再往项上写 `checked`（见 `multi_select/id_based.dart` 的说明）。
///
/// id 取哪一个跟着 [isPublish] 走：发布过的笔记删除时用 `cvid`，没发布的
/// 用 `noteId`，两条接口的参数名也不同。取错的表现是删不掉而不是报错。
class FavNoteController
    extends CommonListController<List<FavNote>?, FavNote>
    with IdMultiSelectMixin<List<FavNote>?, FavNote, String> {
  FavNoteController(this.isPublish);

  final bool isPublish;

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

  @override
  String idOf(FavNote item) =>
      (isPublish ? item.cvid : item.noteId) ?? item.webUrl ?? '';

  @override
  void handleSelect({bool checked = false, bool disableSelect = true}) {
    allSelected.value = checked;
    super.handleSelect(checked: checked, disableSelect: disableSelect);
  }

  @override
  void onSelect(FavNote item) {
    super.onSelect(item);
    allSelected.value =
        rxCount.value == (loadingState.value.dataOrNull?.length ?? 0);
  }

  @override
  Future<LoadingState<List<FavNote>?>> customGetData() =>
      RustFav.noteList(page: page, published: isPublish);

  @override
  Future<void> onRemove() async {
    final removed = allChecked.map(idOf).toSet();
    if (removed.isEmpty) return;
    final res = await RustFav.delNote(
      ids: removed.join(','),
      published: isPublish,
    );
    if (res.isSuccess) {
      afterDeleteIds(removed);
      allSelected.value = false;
      SmartDialog.showToast('删除成功');
    } else {
      res.toast();
    }
  }

  void onDisable() {
    if (checkedCount != 0) {
      handleSelect();
    }
    enableMultiSelect.value = false;
  }
}
