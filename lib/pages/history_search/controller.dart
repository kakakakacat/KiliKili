import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/pages/common/search/common_search_controller.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:flutter/widgets.dart' show Text;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';

class HistorySearchController
    extends CommonSearchController<HistoryPage, HistoryItem>
    with IdMultiSelectMixin<HistoryPage, HistoryItem, String> {
  @override
  Future<LoadingState<HistoryPage>> customGetData() =>
      RustHistory.searchHistory(
        pn: page,
        keyword: editController.value.text,
        account: account,
      );

  @override
  List<HistoryItem>? getDataList(HistoryPage response) {
    return response.list;
  }

  /// 删除接口认的就是这个串，也在同一个列表里唯一，直接拿它当选中态的 id。
  @override
  String idOf(HistoryItem item) => '${item.history.business}_${item.kid}';

  /// 搜索页的多选条是自己的，不与别的 tab 共用。
  @override
  final RxBool enableMultiSelect = false.obs;

  @override
  final RxInt rxCount = 0.obs;

  final account = Accounts.history;

  Future<void> onDelHistory(int index, int? kid, String? business) async {
    final res = await RustHistory.delHistory(
      '${business}_$kid',
      account: account,
    );
    if (res.isSuccess) {
      loadingState
        ..value.data!.removeAt(index)
        ..refresh();
      SmartDialog.showToast('已删除');
    } else {
      res.toast();
    }
  }

  @override
  void onRemove() {
    showConfirmDialog(
      context: Get.context!,
      title: const Text('提示'),
      content: const Text('确认删除所选历史记录吗？'),
      onConfirm: () async {
        SmartDialog.showLoading(msg: '请求中');
        final removed = allChecked.map(idOf).toSet();
        final response = await RustHistory.delHistory(
          removed.join(','),
          account: account,
        );
        if (response.isSuccess) {
          afterDeleteIds(removed);
          SmartDialog.showToast('已删除');
        } else {
          response.toast();
        }
        SmartDialog.dismiss();
      },
    );
  }
}
