import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/live_danmaku.dart';
import 'package:kilikili/services/rust/live_message_session.dart';

const tokenData = LiveDanmakuToken(
  token: 'fixture',
  servers: ['wss://example.invalid/sub'],
);
const tokenResult = LiveDanmakuTokenResult(
  data: tokenData,
  errorKind: null,
  errorDetail: null,
  errorCode: null,
);
const actionResult = LiveSocketActionResult(
  errorKind: null,
  errorDetail: null,
  errorCode: null,
);

void main() {
  test(
    'closing while token is pending never opens a late connection',
    () async {
      final pending = Completer<LiveDanmakuTokenResult>();
      var opened = 0;
      final session = LiveMessageSession(
        token: () => pending.future,
        open: (data, generation) {
          opened++;
          return LiveSocketOpenResult(
            data: LiveSocketHandle(sessionId: 1, generation: generation),
            errorKind: null,
            errorDetail: null,
            errorCode: null,
          );
        },
        poll: (_) => throw StateError('must not poll'),
        closeHandle: (_) => actionResult,
        onBatch: (_) => fail('must not deliver'),
        onError: (_, _) => fail('must not report cancellation'),
      );
      final start = session.start();
      session.close();
      pending.complete(tokenResult);
      await start;
      expect(opened, 0);
      expect(session.active, false);
    },
  );

  test('repeated start opens once and close releases handle once', () async {
    var opened = 0;
    var closed = 0;
    final session = LiveMessageSession(
      token: () async => tokenResult,
      open: (data, generation) {
        opened++;
        return LiveSocketOpenResult(
          data: LiveSocketHandle(sessionId: 3, generation: generation),
          errorKind: null,
          errorDetail: null,
          errorCode: null,
        );
      },
      poll: (_) => throw StateError('timer must be cancelled'),
      closeHandle: (_) {
        closed++;
        return actionResult;
      },
      onBatch: (_) => fail('must not deliver'),
      onError: (_, _) => fail('must not error'),
    );
    await Future.wait([session.start(), session.start()]);
    expect(opened, 1);
    session
      ..close()
      ..close();
    expect(closed, 1);
  });
}
