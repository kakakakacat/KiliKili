import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:get/get.dart';

mixin MultiSelectData {
  bool checked = false;
}

/// 顶部多选条（`MultiSelectAppBarWidget`）需要的那几样，**与列表项类型
/// 无关**。抽出来是为了让两套多选实现都能喂同一个 widget：
/// 本文件这套按 `MultiSelectData.checked` 记选中态，
/// `id_based.dart` 那套按 id 记（S7 #57 起，列表项来自 Rust、不可变）。
abstract interface class MultiSelectBar {
  RxBool get enableMultiSelect;

  int get checkedCount;

  void handleSelect({bool checked = false, bool disableSelect = true});
  void onRemove();
}

abstract interface class MultiSelectBase<T extends MultiSelectData>
    implements MultiSelectBar {
  @override
  RxBool get enableMultiSelect;

  @override
  int get checkedCount;

  void onSelect(T item);
  @override
  void handleSelect({bool checked = false, bool disableSelect = true});
  @override
  void onRemove();
}

mixin BaseMultiSelectMixin<T extends MultiSelectData>
    implements MultiSelectBase<T> {
  late final RxInt rxCount = 0.obs;
  @override
  int get checkedCount => rxCount.value;

  @override
  final RxBool enableMultiSelect = false.obs;

  RxObjectMixin get state;
  List<T> get list;

  Iterable<T> get allChecked => list.where((v) => v.checked);

  @override
  void handleSelect({bool checked = false, bool disableSelect = true}) {
    for (final item in list) {
      item.checked = checked;
    }
    state.refresh();
    rxCount.value = checked ? list.length : 0;
    if (disableSelect && !checked) {
      enableMultiSelect.value = false;
    }
  }

  @override
  void onSelect(T item) {
    item.checked = !item.checked;
    if (item.checked) {
      rxCount.value++;
    } else {
      rxCount.value--;
    }
    state.refresh();
    if (checkedCount == 0) {
      enableMultiSelect.value = false;
    }
  }
}

mixin CommonMultiSelectMixin<T extends MultiSelectData>
    implements MultiSelectBase<T> {
  @override
  late final RxBool enableMultiSelect = false.obs;
  RxBool? get allSelected => null;

  Rx<LoadingState<List<T>?>> get loadingState;
  late final RxInt rxCount = 0.obs;

  @override
  int get checkedCount => rxCount.value;

  Iterable<T> get allChecked =>
      loadingState.value.data!.where((v) => v.checked);

  @override
  void onSelect(T item) {
    List<T> list = loadingState.value.data!;
    item.checked = !item.checked;
    if (item.checked) {
      rxCount.value++;
    } else {
      rxCount.value--;
    }
    loadingState.refresh();
    if (checkedCount == 0) {
      enableMultiSelect.value = false;
    } else {
      allSelected?.value = checkedCount == list.length;
    }
  }

  @override
  void handleSelect({bool checked = false, bool disableSelect = true}) {
    if (loadingState.value case Success(:final response)) {
      if (response != null && response.isNotEmpty) {
        for (final item in response) {
          item.checked = checked;
        }
        loadingState.refresh();
        rxCount.value = checked ? response.length : 0;
      }
    }
    if (disableSelect && !checked) {
      enableMultiSelect.value = false;
    }
  }
}

mixin DeleteItemMixin<R, T extends MultiSelectData>
    on CommonListController<R, T>, CommonMultiSelectMixin<T> {
  Future<void> afterDelete(Set<T> removeList) async {
    final list = loadingState.value.data!;
    if (removeList.length == list.length) {
      list.clear();
    } else if (removeList.length == 1) {
      list.remove(removeList.first);
    } else {
      list.removeWhere(removeList.contains);
    }
    if (list.isNotEmpty || isEnd) {
      loadingState.refresh();
    } else {
      onReload();
    }
    if (enableMultiSelect.value) {
      rxCount.value = 0;
      enableMultiSelect.value = false;
    }
  }
}

// abstract class SetMultiSelectController<R, T, I>
//     extends CommonListController<R, T>
//     with MultiSelectMixin<T>, SetCommonMultiSelectMixin<T, I> {}

// mixin SetCommonMultiSelectMixin<T, R> on MultiSelectMixin<T> {
//   Rx<LoadingState<List<T>?>> get loadingState;
//   RxSet<R> get selected;

//   @override
//   int get checkedCount => selected.length;

//   R getId(T item);

//   @override
//   void onSelect(T item, [bool disableSelect = true]) {
//     final id = getId(item);
//     if (selected.contains(id)) {
//       selected.remove(id);
//     } else {
//       selected.add(id);
//     }
//     loadingState.refresh();
//     if (disableSelect) {
//       if (checkedCount == 0) {
//         enableMultiSelect.value = false;
//       }
//     } else {
//       allSelected.value = checkedCount == loadingState.value.data!.length;
//     }
//   }

//   @override
//   void handleSelect([bool checked = false, bool disableSelect = true]) {
//     if (loadingState.value.isSuccess) {
//       final list = loadingState.value.data;
//       if (list != null && list.isNotEmpty) {
//         if (checked) {
//           selected.addAll(list!.map(getId));
//         } else {
//           selected.clear();
//         }
//         loadingState.refresh();
//       }
//     }
//     if (disableSelect && !checked) {
//       enableMultiSelect.value = false;
//     }
//   }
// }
