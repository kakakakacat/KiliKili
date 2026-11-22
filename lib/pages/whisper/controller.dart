import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_whisper_controller.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:flutter/foundation.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

typedef MsgFeedTopItem = ({
  bool enabled,
  IconData icon,
  String name,
  String route,
});

class WhisperController extends CommonWhisperController<ImSessionPage> {
  /// 主列表就是「首页」那一页。页面类型取 Rust 映射表里的 HOME（1）。
  @override
  int sessionPageType = RustIm.sessionPageTypeHome;

  late final List<MsgFeedTopItem> msgFeedTopItems;
  final disableLikeMsg = Pref.disableLikeMsg;
  late final RxList<int> unreadCounts;

  List<ImSessionOffset>? offsets;

  Rx<List<ImThreeDotItem>?> threeDotItems = Rx<List<ImThreeDotItem>?>(null);
  Rx<List<ImThreeDotItem>?> outsideItem = Rx<List<ImThreeDotItem>?>(null);

  @override
  void onInit() {
    super.onInit();
    msgFeedTopItems = [
      const (
        name: "回复我的",
        icon: Icons.message_outlined,
        route: "/replyMe",
        enabled: true,
      ),
      const (
        name: "@我",
        icon: Icons.alternate_email_outlined,
        route: "/atMe",
        enabled: true,
      ),
      (
        name: "收到的赞",
        icon: Icons.favorite_border_outlined,
        route: "/likeMe",
        enabled: !disableLikeMsg,
      ),
      const (
        name: "系统通知",
        icon: Icons.notifications_none_outlined,
        route: "/sysMsg",
        enabled: true,
      ),
    ];
    unreadCounts = List.filled(msgFeedTopItems.length, 0).obs;
    queryMsgFeedUnread();
    queryData();
  }

  Future<void> queryMsgFeedUnread() async {
    final res = await RustIm.totalUnread();
    if (res case Success(:final response)) {
      final unreadCounts = [
        response.reply,
        response.at,
        disableLikeMsg ? 0 : response.like,
        response.sysMsg,
      ];
      if (!listEquals(this.unreadCounts, unreadCounts)) {
        this.unreadCounts.value = unreadCounts;
      }
    } else {
      res.toast();
    }
  }

  @override
  List<ImSession>? getDataList(ImSessionPage response) {
    offsets = response.offsets;
    isEnd = !response.hasMore;
    return response.sessions;
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<ImSessionPage> response) {
    if (isRefresh) {
      threeDotItems.value = response.response.threeDotItems;
      outsideItem.value = response.response.outsideItems;
    }
    return false;
  }

  @override
  Future<LoadingState<ImSessionPage>> customGetData() =>
      RustIm.sessionMain(offsets: offsets);

  @override
  Future<void> onRefresh() {
    offsets = null;
    queryMsgFeedUnread();
    return super.onRefresh();
  }
}
