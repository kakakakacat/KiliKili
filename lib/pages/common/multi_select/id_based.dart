import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/common/multi_select/base.dart';
import 'package:get/get.dart';

/// 以 **id 集合**记录选中态的多选，供列表项是**不可变**结构的页面使用
/// （S7 #57 起：观看历史、稍后再看）。
///
/// ## 为什么要有第二套
///
/// `lib/pages/common/multi_select/base.dart` 那一套把选中态写在列表项上
/// （`MultiSelectData.checked`），要求每个模型都能被就地改。typed 直连之后
/// 列表项来自 Rust，字段是 final 的——而选中态本来也**不是协议的一部分**，
/// 它是界面状态，按 AGENTS.md §2 就该留在控制器里。
///
/// 没有直接改那一套，是因为 `MultiSelectData` 还有六个实现者分属别的切片
/// （收藏、下载、动态 @、热门视频…）。改它等于把那几片一起拖进本次迁移。
/// **它们迁到 Rust 时应当改用本文件，`base.dart` 随最后一个实现者删掉。**
/// `base.dart` 末尾那个被注释掉的 `SetMultiSelectController` 草稿想做的就是
/// 这件事，这里把它做完了。
///
/// ## [enableMultiSelect] 与 [rxCount] 为什么是抽象的
///
/// 历史页与稍后再看页都是**多个 tab 共用一个底部多选条**，开关与计数挂在
/// 各自的 base controller 上；而别的页面可能自己持有。所以由实现类给，
/// 不在这里造。选中的 id 集合则必然是每个列表自己的。
mixin IdMultiSelectMixin<R, T, I> on CommonListController<R, T>
    implements MultiSelectBar {
  /// 选中的 id。用 id 而不是对象：列表刷新后对象是新的实例，按对象记的
  /// 选中态会在刷新的那一刻凭空消失；FRB 生成的类也没有值相等语义。
  final Set<I> selectedIds = <I>{};

  /// 多选模式开关。可能与同页的别的 tab 共用。
  @override
  RxBool get enableMultiSelect;

  /// 选中计数。可能与同页的别的 tab 共用，所以不是从 [selectedIds] 直接读。
  RxInt get rxCount;

  /// 这一项的 id。必须在**同一个列表里唯一**，否则会连带选中另一项。
  I idOf(T item);

  @override
  int get checkedCount => rxCount.value;

  bool isChecked(T item) => selectedIds.contains(idOf(item));

  Iterable<T> get allChecked =>
      (loadingState.value.dataOrNull ?? const []).where(isChecked);

  void onSelect(T item) {
    final id = idOf(item);
    if (!selectedIds.remove(id)) {
      selectedIds.add(id);
    }
    rxCount.value = selectedIds.length;
    loadingState.refresh();
    if (selectedIds.isEmpty) {
      enableMultiSelect.value = false;
    }
  }

  /// 全选/全不选。[disableSelect] 为真时取消全选顺带退出多选模式。
  @override
  void handleSelect({bool checked = false, bool disableSelect = true}) {
    final list = loadingState.value.dataOrNull;
    selectedIds.clear();
    if (checked && list != null) {
      selectedIds.addAll(list.map(idOf));
    }
    rxCount.value = selectedIds.length;
    loadingState.refresh();
    if (disableSelect && !checked) {
      enableMultiSelect.value = false;
    }
  }

  /// 删除之后把它们从列表里摘掉。
  ///
  /// 与 `DeleteItemMixin` 的差别只在「按 id 比」：列表项不可变，`remove`
  /// 依赖 `==`，而 FRB 生成的类没有值相等语义。
  void afterDeleteIds(Set<I> removed) {
    final list = loadingState.value.dataOrNull;
    if (list == null) return;
    list.removeWhere((e) => removed.contains(idOf(e)));
    selectedIds.removeAll(removed);
    rxCount.value = selectedIds.length;
    if (list.isNotEmpty || isEnd) {
      loadingState.refresh();
    } else {
      onReload();
    }
    if (enableMultiSelect.value && selectedIds.isEmpty) {
      enableMultiSelect.value = false;
    }
  }
}
