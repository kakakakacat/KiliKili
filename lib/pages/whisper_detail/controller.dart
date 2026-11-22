import 'dart:async';
import 'dart:convert';

import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:kilikili/services/rust/rust_msg.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:kilikili/utils/feed_back.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class WhisperDetailController
    extends CommonListController<ImMessagePage, ImMessage> {
  late final account = Accounts.main;

  late final int talkerId;
  late final String name;
  late final String? face;
  late final int? mid;
  late final bool isLive;

  int? msgSeqno;

  /// 表情转图片的规则。每页都可能带新的，累加。
  List<ImEmote>? eInfos;

  @override
  void onInit() {
    super.onInit();
    final args = Get.arguments;
    talkerId = args['talkerId'];
    name = args['name'];
    face = args['face'];
    mid = args['mid'];
    isLive = args['isLive'] ?? false;
    queryData();
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<ImMessagePage> response) {
    final msgs = response.response.messages;
    if (msgs.isNotEmpty) {
      msgSeqno = msgs.last.msgSeqno;
      // 「对方主动回复或关注你前，最多发送 1 条消息」那条提示单独成页时
      // 不要标已读——标了之后这条提示就再也不显示了。基线的判据是
      // 「整页只有一条，且类型与来源都是 18」，照搬。
      final isLonelyTip =
          msgs.length == 1 &&
          msgs.last.kind == ImMsgKind.tipMessage &&
          msgs.last.msgSource == 18;
      if (!isLonelyTip) {
        ackSessionMsg(msgs.last.msgSeqno);
      }
      msgs.removeWhere((e) => e.kind == ImMsgKind.drawBack);
      (eInfos ??= <ImEmote>[]).addAll(response.response.emotes);
    }
    return false;
  }

  /// 消息标记已读。位点来自这一页最老那条。
  Future<void> ackSessionMsg(int ackSeqno) async {
    final res = await RustMsg.ackSessionMsg(
      talkerId: talkerId,
      ackSeqno: ackSeqno,
    );
    if (!res.isSuccess) {
      res.toast();
    }
  }

  late bool _isSending = false;

  /// 发一条私信。
  ///
  /// **这条一次都不会自动重发**（Rust 侧定级 `Unsafe`）。`devId` 由
  /// `RustIm.sendMessage` 在这一次调用内生成一次——服务端拿它去重，
  /// 每次重新生成等于告诉服务端「这是新的一条」。
  Future<void> sendMsg({
    String? message,
    Map? picMsg,
    required VoidCallback onClearText,
    ImMsgKind? kind,
    int? index,
  }) async {
    assert((message != null) ^ (picMsg != null));
    if (_isSending) return;
    _isSending = true;
    feedBack();
    SmartDialog.dismiss();
    if (!account.isLogin) {
      SmartDialog.showToast('请先登录');
      _isSending = false;
      return;
    }
    final isDrawBack = kind == ImMsgKind.drawBack;
    final msgKind =
        kind ?? (picMsg != null ? ImMsgKind.picture : ImMsgKind.text);
    final msgType = RustIm.msgTypeOf(msgKind);
    if (msgType == null) {
      SmartDialog.showToast('不支持发送这种消息');
      _isSending = false;
      return;
    }
    final res = await RustIm.sendMessage(
      senderUid: account.mid,
      receiverId: mid!,
      msgType: msgType,
      content: isDrawBack
          ? message!
          : jsonEncode(picMsg ?? {"content": message!}),
    );
    SmartDialog.dismiss();
    if (res.isSuccess) {
      if (isDrawBack) {
        // 撤回成功：换掉列表里那一条（FRB 导出的类型不可变）。
        final list = loadingState.value.data!;
        final item = list[index!];
        list[index] = ImMessage(
          senderUid: item.senderUid,
          receiverId: item.receiverId,
          msgType: item.msgType,
          kind: item.kind,
          msgSource: item.msgSource,
          msgStatus: 1,
          msgKey: item.msgKey,
          msgSeqno: item.msgSeqno,
          timestamp: item.timestamp,
          content: item.content,
        );
        loadingState.refresh();
        SmartDialog.showToast('撤回成功');
      } else {
        onRefresh();
        onClearText();
        SmartDialog.showToast('发送成功');
      }
    } else {
      res.toast();
    }
    _isSending = false;
  }

  @override
  List<ImMessage>? getDataList(ImMessagePage response) {
    if (!response.hasMore) {
      isEnd = true;
    }
    return response.messages;
  }

  @override
  Future<void> onRefresh() {
    msgSeqno = null;
    eInfos = null;
    scrollController.jumpToTop();
    return super.onRefresh();
  }

  @override
  Future<LoadingState<ImMessagePage>> customGetData() =>
      RustIm.messages(talkerId: talkerId, endSeqno: msgSeqno);

  Future<LoadingState> onReport(
    ImMessage item,
    int reasonType,
    String reasonDesc,
  ) {
    return RustMsg.imMsgReport(
      accusedUid: item.senderUid,
      reasonType: reasonType,
      reasonDesc: reasonDesc,
      comment: {'group_id': 0, 'msg_key': '${item.msgKey}'},
      extra: {"msg_keys": []},
    );
  }
}
