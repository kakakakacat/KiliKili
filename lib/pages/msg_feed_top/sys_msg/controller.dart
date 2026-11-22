import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_msg.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

class SysMsgController extends CommonListController<List<SysMsgItem>, SysMsgItem> {
  int? cursor;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  void handleListResponse(List<SysMsgItem> dataList) {
    if (cursor == null) {
      msgSysUpdateCursor(dataList.first.cursor);
    }
    cursor = dataList.last.cursor;
  }

  void msgSysUpdateCursor(int? cursor) {
    if (cursor != null) {
      RustMsg.msgSysUpdateCursor(cursor);
    }
  }

  @override
  Future<void> onRefresh() {
    cursor = null;
    return super.onRefresh();
  }

  Future<void> onRemove(Object? id, int index) async {
    try {
      final res = await RustMsg.delSysMsg('$id');
      if (res.isSuccess) {
        loadingState
          ..value.data!.removeAt(index)
          ..refresh();
        SmartDialog.showToast('删除成功');
      } else {
        res.toast();
      }
    } catch (_) {}
  }

  @override
  Future<LoadingState<List<SysMsgItem>>> customGetData() =>
      RustMsg.msgFeedNotify(cursor: cursor);
}
