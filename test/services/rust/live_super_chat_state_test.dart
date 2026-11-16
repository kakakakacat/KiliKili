import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/pages/live_room/superchat/super_chat_state.dart';

void main() {
  test('全屏展示到期和移除状态不改写 typed 网络数据', () {
    final source = LiveSuperChatState.random.data;
    final panel = LiveSuperChatState(source);
    final overlay = LiveSuperChatState(
      source,
      displayEndTime: source.endTime - 2,
    );
    overlay.expired = true;
    overlay.deleted = true;

    expect(identical(panel.data, source), isTrue);
    expect(identical(overlay.data, source), isTrue);
    expect(panel.displayEndTime, source.endTime);
    expect(overlay.displayEndTime, source.endTime - 2);
    expect(panel.expired, isFalse);
    expect(panel.deleted, isFalse);
  });
}
