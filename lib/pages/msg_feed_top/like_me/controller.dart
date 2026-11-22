import 'package:kilikili/common/widgets/pair.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_data_controller.dart';
import 'package:kilikili/services/rust/rust_msg.dart';
import 'package:kilikili/utils/extension/iterable_ext.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

class LikeMeController
    extends
        CommonDataController<
          LikeFeed,
          Pair<List<LikeFeedItem>, List<LikeFeedItem>>
        > {
  int? cursor;
  int? cursorTime;

  bool isEnd = false;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<void> queryData([bool isRefresh = true]) {
    if (!isRefresh && isEnd) {
      return Future.syncValue(null);
    }
    return super.queryData(isRefresh);
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<LikeFeed> response) {
    LikeFeed data = response.response;
    if (data.total?.cursor?.isEnd == true ||
        data.total?.items.isNullOrEmpty == true) {
      isEnd = true;
    }
    cursor = data.total?.cursor?.id;
    cursorTime = data.total?.cursor?.time;
    // `[...?x]` 一步做两件事：`latest`/`total` 整段可能缺（基线写的是
    // `?? <MsgLikeItem>[]`），以及**不要就地改 FRB 刚解出来的那份列表**
    // ——下面要往里 insertAll，删除时还要 removeAt。
    List<LikeFeedItem> latest = [...?data.latest?.items];
    List<LikeFeedItem> total = [...?data.total?.items];
    if (!isRefresh) {
      if (loadingState.value case Success(:final response)) {
        latest.insertAll(0, response.first);
        total.insertAll(0, response.second);
      }
    }
    loadingState.value = Success(Pair(first: latest, second: total));
    return true;
  }

  @override
  Future<void> onRefresh() {
    cursor = null;
    cursorTime = null;
    return super.onRefresh();
  }

  @override
  Future<LoadingState<LikeFeed>> customGetData() =>
      RustMsg.msgFeedLikeMe(cursor: cursor, cursorTime: cursorTime);

  Future<void> onRemove(Object? id, int index, bool isLatest) async {
    try {
      // tp=0 是「收到的赞」。
      final res = await RustMsg.delMsgfeed(0, '$id');
      if (res.isSuccess) {
        Pair<List<LikeFeedItem>, List<LikeFeedItem>> pair =
            loadingState.value.data;
        if (isLatest) {
          pair.first.removeAt(index);
        } else {
          pair.second.removeAt(index);
        }
        loadingState.refresh();
        SmartDialog.showToast('删除成功');
      } else {
        res.toast();
      }
    } catch (_) {}
  }

  /// 开关某条赞的通知。
  ///
  /// 基线在这里直接改 `item.noticeState`。FRB 导出的类型是不可变的
  /// （字段全是 final），所以改成**按位置换一条新的**——这也更诚实：
  /// 列表里的那条本来就该是服务端状态的副本，而不是一个可以被就地改写
  /// 的可变对象。因此调用点要多给 [index] 与 [isLatest]。
  Future<void> onSetNotice(
    LikeFeedItem item,
    int index,
    bool isLatest,
    bool isNotice,
  ) async {
    final int noticeState = isNotice ? 1 : 0;
    final res = await RustMsg.msgSetNotice(
      id: '${item.id}',
      noticeState: noticeState,
    );
    if (res.isSuccess) {
      final updated = LikeFeedItem(
        id: item.id,
        users: item.users,
        item: item.item,
        counts: item.counts,
        likeTime: item.likeTime,
        noticeState: noticeState,
      );
      final pair = loadingState.value.data;
      (isLatest ? pair.first : pair.second)[index] = updated;
      loadingState.refresh();
      SmartDialog.showToast('设置成功');
    } else {
      res.toast();
    }
  }
}
