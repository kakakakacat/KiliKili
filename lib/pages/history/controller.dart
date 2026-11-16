import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/pages/history/base_controller.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_key.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class HistoryController
    extends CommonListController<HistoryPage, HistoryItem>
    with
        IdMultiSelectMixin<HistoryPage, HistoryItem, String>,
        GetSingleTickerProviderStateMixin {
  HistoryController(this.type);

  late final baseCtr = Get.put(HistoryBaseController());

  Account get account => baseCtr.account;

  final String? type;
  TabController? tabController;
  late RxList<HistoryTab> tabs = <HistoryTab>[].obs;

  int? max;
  int? viewAt;

  @override
  RxInt get rxCount => baseCtr.checkedCount;

  @override
  RxBool get enableMultiSelect => baseCtr.enableMultiSelect;

  @override
  void onInit() {
    super.onInit();
    historyStatus();
    queryData();
  }

  @override
  Future<void> onRefresh() {
    max = null;
    viewAt = null;
    return super.onRefresh();
  }

  @override
  List<HistoryItem>? getDataList(HistoryPage response) {
    return response.list;
  }

  /// 删除接口认的就是这个串（`${business}_${kid}`），正好也在同一个列表里
  /// 唯一，所以直接拿它当选中态的 id，不另造一个。
  @override
  String idOf(HistoryItem item) => '${item.history.business}_${item.kid}';

  @override
  bool customHandleResponse(bool isRefresh, Success<HistoryPage> response) {
    final data = response.response;
    final last = data.list.lastOrNull;
    if (last == null) {
      isEnd = true;
      max = viewAt = null;
    } else {
      isEnd = false;
      max = last.history.oid;
      viewAt = last.viewAt;
    }

    if (isRefresh && type == null) {
      final tab = data.tabs;
      if (tabs.isEmpty && tab.isNotEmpty) {
        tabs.value = tab;
        tabController = TabController(length: tab.length + 1, vsync: this);
      }
    }

    return false;
  }

  // 观看历史暂停状态
  Future<void> historyStatus() async {
    final res = await RustHistory.historyStatus(account: account);
    if (res case Success(:final response)) {
      baseCtr.pauseStatus.value = response;
      GStorage.localCache.put(LocalCacheKey.historyPause, response);
    } else {
      res.toast();
    }
  }

  // 删除某条历史记录
  void delHistory(HistoryItem item) {
    _onDelete({item});
  }

  // 删除已看历史记录
  void onDelViewedHistory() {
    final viewedList = loadingState.value.dataOrNull
        ?.where((e) => e.progress == -1)
        .toSet();
    if (viewedList != null && viewedList.isNotEmpty) {
      _onDelete(viewedList);
    } else {
      SmartDialog.showToast('无已看记录');
    }
  }

  Future<void> _onDelete(Set<HistoryItem> removeList) async {
    SmartDialog.showLoading(msg: '请求中');
    final ids = removeList.map(idOf).toSet();
    final res = await RustHistory.delHistory(
      ids.join(','),
      account: account,
    );
    SmartDialog.dismiss();
    if (res.isSuccess) {
      afterDeleteIds(ids);
      SmartDialog.showToast('已删除');
    } else {
      res.toast();
    }
  }

  // 删除选中的记录
  @override
  void onRemove() {
    showConfirmDialog(
      context: Get.context!,
      title: const Text('提示'),
      content: const Text('确认删除所选历史记录吗？'),
      onConfirm: () => _onDelete(allChecked.toSet()),
    );
  }

  @override
  Future<LoadingState<HistoryPage>> customGetData() => RustHistory.historyList(
    type: type ?? 'all',
    max: max,
    viewAt: viewAt,
    account: account,
  );

  @override
  void onClose() {
    tabController?.dispose();
    super.onClose();
  }

  @override
  Future<void> onReload() {
    scrollController.jumpToTop();
    return super.onReload();
  }
}
