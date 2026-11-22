import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:kilikili/services/rust/rust_msg.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

/// 私信会话列表的公共控制器。
///
/// [R] 是主列表与二级列表各自的响应类型；它们在 Rust 侧已经投影成同一个
/// [ImSessionPage]，所以这里其实只剩一个类型参数的壳。留着是因为两个子类
/// 的 `customGetData` 调的是不同的接口。
abstract class CommonWhisperController<R>
    extends CommonListController<R, ImSession> {
  /// 这一页对应的会话页类型。
  ///
  /// 用的是 Rust 映射好的页面类型（`ImSessionTarget.foldPageType`）而不是
  /// 会话类型——两个枚举的数字几乎每行都不同，传错不会报错，只会操作到
  /// 另一个列表。
  int get sessionPageType;

  /// 删掉列表里第 [index] 条会话。
  ///
  /// 对方的 mid 从那一条自己身上取，不再由 widget 传进来——基线传的是
  /// `item.id.privateId.talkerUid`，而那正是这一条的属性。
  Future<void> onRemoveAt(int index) async {
    final list = loadingState.value.data;
    final talkerUid = list?.elementAtOrNull(index)?.target.talkerUid;
    if (list == null || talkerUid == null) return;
    final res = await RustMsg.removeMsg(talkerUid);
    if (res.isSuccess) {
      list.removeAt(index);
      loadingState.refresh();
      SmartDialog.showToast('删除成功');
    } else {
      res.toast();
    }
  }

  /// 把第 [index] 条的未读清掉（本地）。
  ///
  /// 「标为已读」成功后、以及点进会话时调用。只改本地那一条——远端已经
  /// 由调用方标过了，或者（点进会话时）由详情页的已读位点负责。
  void onAcked(int index) {
    final list = loadingState.value.data;
    final item = list?.elementAtOrNull(index);
    if (list == null || item == null) return;
    list[index] = _copyWith(item, clearUnread: true);
    loadingState.refresh();
  }

  /// 置顶 / 取消置顶。
  ///
  /// 成功后换掉列表里那一条而不是就地改字段：FRB 导出的 [ImSession] 是
  /// 不可变的（字段全是 final）。取消置顶时基线也不调整顺序，照旧。
  Future<void> onSetTop(ImSession item, int index, bool isTop) async {
    final talkerUid = item.target.talkerUid;
    if (talkerUid == null) return;
    final res = await RustIm.setPinned(talkerUid: talkerUid, pinned: !isTop);
    if (res.isSuccess) {
      final list = loadingState.value.data!;
      list[index] = _copyWith(item, isPinned: !isTop);
      if (!isTop) {
        list.insert(0, list.removeAt(index));
      }
      loadingState.refresh();
      SmartDialog.showToast('${isTop ? '移除' : ''}置顶成功');
    } else {
      res.toast();
    }
  }

  Future<void> onSetMute(ImSession item, int index, bool isMuted) async {
    final talkerUid = item.target.talkerUid;
    if (talkerUid == null) return;
    final res = await RustMsg.setMsgDnd(
      uid: Accounts.main.mid,
      setting: isMuted ? 0 : 1,
      dndUid: talkerUid,
    );
    if (res.isSuccess) {
      loadingState.value.data![index] = _copyWith(item, isMuted: !isMuted);
      loadingState.refresh();
      SmartDialog.showToast('设置成功');
    } else {
      res.toast();
    }
  }

  Future<void> onClearUnread() async {
    final res = await RustIm.clearUnread(sessionPageType);
    if (res.isSuccess) {
      if (loadingState.value case Success(:final response)) {
        if (response != null && response.isNotEmpty) {
          for (var i = 0; i < response.length; i++) {
            if (response[i].unreadStyle != ImUnreadStyle.none) {
              response[i] = _copyWith(response[i], clearUnread: true);
            }
          }
          loadingState.refresh();
        }
      }
      SmartDialog.showToast('已标记为已读');
    } else {
      res.toast();
    }
  }

  Future<void> onDeleteList() async {
    final res = await RustIm.deleteSessionList(sessionPageType);
    if (res.isSuccess) {
      loadingState.value = const Success(null);
    } else {
      res.toast();
    }
  }

  /// 就地换掉一条会话。
  ///
  /// FRB 的导出类型没有 `copyWith`，而这三处都只改一两个字段。写成一个
  /// 私有辅助而不是在三处各抄一遍十九个字段——抄漏一个的表现是那条会话
  /// 的头像或未读数莫名其妙回到了上一次的值。
  static ImSession _copyWith(
    ImSession item, {
    bool? isPinned,
    bool? isMuted,
    bool clearUnread = false,
  }) => ImSession(
    target: item.target,
    name: item.name,
    avatarUrl: item.avatarUrl,
    pendantUrl: item.pendantUrl,
    officialType: item.officialType,
    vipStatus: item.vipStatus,
    vipType: item.vipType,
    labelText: item.labelText,
    isLive: item.isLive,
    isPinned: isPinned ?? item.isPinned,
    isMuted: isMuted ?? item.isMuted,
    timestampUs: item.timestampUs,
    summary: item.summary,
    unreadStyle: clearUnread ? ImUnreadStyle.none : item.unreadStyle,
    unreadNumber: clearUnread ? 0 : item.unreadNumber,
    mid: item.mid,
  );
}
