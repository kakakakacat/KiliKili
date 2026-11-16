import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/pages/live_room/superchat/super_chat_state.dart';

void main() {
  test('卡片销毁会取消延迟移除且晚到的帧回调不能重新排入', () {
    fakeAsync((async) {
      final removal = LiveSuperChatRemoval();
      var calls = 0;
      removal.schedule(() => calls++);
      async.elapse(const Duration(milliseconds: 500));
      removal.dispose();
      removal.schedule(() => calls++);
      async.elapse(const Duration(seconds: 2));
      expect(calls, 0);
      expect(async.nonPeriodicTimerCount, 0);
    });
  });

  test('仍存活的卡片在展示延迟后只移除一次', () {
    fakeAsync((async) {
      final removal = LiveSuperChatRemoval();
      var calls = 0;
      removal.schedule(() => calls++);
      async.elapse(const Duration(seconds: 1));
      expect(calls, 1);
      async.elapse(const Duration(seconds: 2));
      expect(calls, 1);
      removal.dispose();
    });
  });

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
