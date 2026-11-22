import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_msg.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

class ReplyMeController extends CommonListController<ReplyFeed, ReplyFeedItem> {
  int? cursor;
  int? cursorTime;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<ReplyFeedItem>? getDataList(ReplyFeed response) {
    if (response.cursor?.isEnd == true) {
      isEnd = true;
    }
    cursor = response.cursor?.id;
    cursorTime = response.cursor?.time;
    return response.items;
  }

  @override
  Future<void> onRefresh() {
    cursor = null;
    cursorTime = null;
    return super.onRefresh();
  }

  @override
  Future<LoadingState<ReplyFeed>> customGetData() =>
      RustMsg.msgFeedReplyMe(cursor: cursor, cursorTime: cursorTime);

  Future<void> onRemove(Object? id, int index) async {
    try {
      // tp=1 是「回复我的」。id 按字符串过去：服务端对它既可能回整数也
      // 可能回字符串（同类问题见 #166）。
      final res = await RustMsg.delMsgfeed(1, '$id');
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
}
