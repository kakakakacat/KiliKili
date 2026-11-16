import 'dart:math';

import 'package:kilikili/rust/api/live.dart';
import 'package:kilikili/rust/api/live_danmaku.dart';

/// 仅保存展示寿命；网络字段始终直接使用 Rust typed 数据。
class LiveSuperChatState {
  LiveSuperChatState(this.data, {int? displayEndTime})
    : displayEndTime = displayEndTime ?? data.endTime;

  final LiveSuperChat data;
  final int displayEndTime;
  bool expired = false;
  bool deleted = false;

  /// 仅用于设置页和调试预览，不参与网络响应解析。
  static LiveSuperChatState get random {
    final random = Random();
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    return LiveSuperChatState(
      LiveSuperChat(
        id: random.nextInt(2147483647),
        uid: 0,
        price: 66,
        backgroundColor: '#EDF5FF',
        backgroundBottomColor: '#2A60B2',
        backgroundPriceColor: '#7497CD',
        messageFontColor: '#FFFFFF',
        startTime: now,
        endTime: now + 5,
        message: '这是一条用于预览的醒目留言。',
        token: '',
        ts: now,
        userInfo: LiveSuperChatUser(
          face: '',
          faceFrame: null,
          uname: 'UNAME',
          nameColor: '#000000',
        ),
        medal: LiveMedal(
          name: 'Medal',
          level: random.nextInt(40),
          id: 123,
          ruid: 456,
          colorStart: '#4C7DFF99',
          colorText: '#FFFFFF',
        ),
        roomId: random.nextInt(2147483647),
      ),
    );
  }
}
