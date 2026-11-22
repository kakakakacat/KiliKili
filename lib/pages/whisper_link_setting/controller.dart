import 'dart:async';

import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/dialog/report_member.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:kilikili/services/rust/rust_msg.dart';
import 'package:kilikili/services/rust/rust_video_write.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:flutter/widgets.dart' show Text;
import 'package:get/get.dart';

class WhisperLinkSettingController extends GetxController {
  WhisperLinkSettingController({
    required this.talkerUid,
  });

  final int talkerUid;
  RxBool isPinned = false.obs;

  @override
  void onInit() {
    super.onInit();
    getUserInfo();
    getSessionSs();
    getMsgDnd();
    getIsPinned();
  }

  final Rx<LoadingState<List<ImUserInfo>>> userState =
      LoadingState<List<ImUserInfo>>.loading().obs;
  final Rx<LoadingState<SessionSs>> sessionSs =
      LoadingState<SessionSs>.loading().obs;
  final Rx<LoadingState<List<MsgDndSetting>>> msgDnd =
      LoadingState<List<MsgDndSetting>>.loading().obs;

  Future<void> getUserInfo() async {
    userState.value = await RustMsg.imUserInfos(uids: talkerUid.toString());
  }

  Future<void> getSessionSs() async {
    sessionSs.value = await RustMsg.getSessionSs(talkerUid: talkerUid);
  }

  Future<void> getMsgDnd() async {
    msgDnd.value = await RustMsg.getMsgDnd(uidsStr: '$talkerUid');
  }

  Future<void> getIsPinned() async {
    final res = await RustIm.session(talkerUid);
    if (res case Success(:final response)) {
      // 查不到这个会话时 response 为 null——那是「没有」，不是失败，
      // 保持默认的未置顶。
      isPinned.value = response?.isPinned ?? false;
    }
  }

  void setPush(bool isPush) {
    if (isPush) {
      showConfirmDialog(
        context: Get.context!,
        title: const Text('确认关闭内容推送吗？'),
        content: const Text('若关闭此开关，你将不再收到该账号的图文消息与稿件推送，但通知类消息不受影响'),
        onConfirm: () => _setPush(isPush),
      );
      return;
    }
    _setPush(isPush);
  }

  Future<void> _setPush(bool isPush) async {
    int setting = isPush ? 1 : 0;
    final res = await RustMsg.setPushSs(
      setting: setting,
      talkerUid: talkerUid,
    );
    if (res.isSuccess) {
      _patchSessionSs(pushSetting: setting);
    } else {
      res.toast();
    }
  }

  Future<void> setPin() async {
    final res = await RustIm.setPinned(
      talkerUid: talkerUid,
      pinned: !isPinned.value,
    );
    if (res.isSuccess) {
      isPinned.toggle();
    } else {
      res.toast();
    }
  }

  Future<void> setMute(bool isMuted) async {
    int setting = isMuted ? 0 : 1;
    final res = await RustMsg.setMsgDnd(
      uid: Accounts.main.mid,
      setting: setting,
      dndUid: talkerUid,
    );
    if (res.isSuccess) {
      // FRB 导出的类型不可变（字段全是 final），所以换一条新的而不是就地
      // 改 setting。列表里那条本来就该是服务端状态的副本。
      if (msgDnd.value case Success(:final response)) {
        if (response.isNotEmpty) {
          response[0] = MsgDndSetting(setting: setting);
          msgDnd.refresh();
        }
      }
    } else {
      res.toast();
    }
  }

  Future<void> setBlock(bool isBlocked) async {
    if (isBlocked) {
      final res = await RustVideoWrite.relationMod(
        mid: talkerUid,
        act: 6,
        reSrc: 11,
      );
      if (res.isSuccess) {
        _patchSessionSs(clearFollowStatus: true);
      } else {
        res.toast();
      }
    } else {
      showConfirmDialog(
        context: Get.context!,
        title: const Text('确认拉黑该用户'),
        content: const Text('加入黑名单后，将自动解除关注关系和对该用户的合集订阅关系，禁止该用户与我互动或查看我的空间'),
        onConfirm: () async {
          final res = await RustVideoWrite.relationMod(
            mid: talkerUid,
            act: 5,
            reSrc: 11,
          );
          if (res.isSuccess) {
            // 128 是「已拉黑」。
            _patchSessionSs(followStatus: 128);
          } else {
            res.toast();
          }
        },
      );
    }
  }

  /// 就地更新会话设置的一项。
  ///
  /// FRB 导出的 [SessionSs] 是不可变的，所以这里重建一份而不是改字段。
  /// [clearFollowStatus] 单独给一个参数：`followStatus: null` 没法和「这次
  /// 不改它」区分开，而「取消关注后清掉关系」正好要置空。
  void _patchSessionSs({
    int? pushSetting,
    int? followStatus,
    bool clearFollowStatus = false,
  }) {
    if (sessionSs.value case Success(:final response)) {
      sessionSs
        ..value = Success(
          SessionSs(
            followStatus: clearFollowStatus
                ? null
                : (followStatus ?? response.followStatus),
            pushSetting: pushSetting ?? response.pushSetting,
            showPushSetting: response.showPushSetting,
          ),
        )
        ..refresh();
    }
  }

  void report() => showMemberReportDialog(
    Get.context!,
    name: userState.value.dataOrNull?.firstOrNull?.name,
    mid: talkerUid,
  );
}
