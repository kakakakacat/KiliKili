import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/reply/reply_controller.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

mixin BaseReplyFoldMixin<R> on ReplyController<R> {
  ReplyFoldCard? foldCard;

  Future<void> loadFoldList({bool toast = true}) async {
    if (isLoading) return;
    isLoading = true;
    try {
      final res = await RustReplyGrpc.foldList(
        oid: oid,
        type: replyType,
        offset: foldCard!.paginationOffset,
      );
      if (isClosed) return;
      if (res case Success(:final response)) {
        foldCard = null;
        count.value += response.replies.length;
        loadingState
          ..value.data!.addAll(response.replies)
          ..refresh();
        if (toast) {
          if (response.title case final title?) {
            SmartDialog.showToast(title);
          }
        }
      } else {
        res.toast();
      }
    } catch (e) {
      if (kDebugMode) rethrow;
      SmartDialog.showToast(e.toString());
    } finally {
      isLoading = false;
    }
  }

  @override
  Future<void> onRefresh() {
    foldCard = null;
    return super.onRefresh();
  }
}

mixin ReplyFoldMixin on BaseReplyFoldMixin<ReplyMainPage> {
  @override
  List<ReplyEntity>? getDataList(ReplyMainPage response) {
    // 这一页没提混合卡时**沿用上一页的折叠入口**，别清掉——照抄基线
    // `getFoldCard` 的 `if (mixedCards.isNotEmpty)`。
    if (response.hasMixedCards) foldCard = response.foldCard;
    return response.replies;
  }
}
