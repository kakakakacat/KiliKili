import 'dart:async';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/frb_generated.dart';
import 'package:kilikili/rust/api/player_port.dart';
import 'package:kilikili/plugin/pl_player/player_port/flutter_media_kit_adapter.dart';

final enabled = Platform.environment['KILIKILI_RUST_BRIDGE_TESTS'] == '1';
PlayerObservation observation(
  PlayerObservationKind kind, [
  double value = 0,
  BigInt? generation,
]) => PlayerObservation(kind: kind, value: value, generation: generation);
PlayerIntent openIntent(String video) => PlayerIntent(
  kind: PlayerIntentKind.open,
  video: video,
  userAgent: 'fixture',
  referer: 'fixture',
);

class Backend implements PlayerPortBackend {
  final pending = <PlayerObservation>[];
  final commands = <PlayerCommandTicket>[];
  Completer<void>? blocked;
  bool disposed = false;
  @override
  List<PlayerObservation> drain() {
    final copy = [...pending];
    pending.clear();
    return copy;
  }

  @override
  void sample(BigInt generation) {}
  @override
  void detachObserver() {}
  @override
  Future<void> execute(
    PlayerCommandTicket ticket, {
    Duration? start,
    Map<String, String>? extras,
    bool play = false,
  }) async {
    commands.add(ticket);
    await blocked?.future;
  }

  @override
  Future<void> dispose() async {
    disposed = true;
  }
}

void main() {
  setUpAll(() async {
    if (!enabled) return;
    final library = Platform.environment['KILIKILI_PLAYER_BRIDGE_LIBRARY'];
    await RustLib.init(
      externalLibrary: library == null ? null : ExternalLibrary.open(library),
    );
  });

  test(
    'typed facts require backend confirmation and immutable sample generation',
    () {
      final baseline = playerPortSessionCount();
      final session = RustPlayerPortSession();
      final ticket = session.prepare(openIntent('fixture.mp4'));
      expect(
        playerPortBegin(
          sessionId: session.id,
          generation: ticket.generation,
        ).errorKind,
        isNull,
      );
      var batch = session.observe([
        observation(PlayerObservationKind.position, 2, BigInt.zero),
      ]);
      expect(batch.snapshot, isNull);
      batch = session.observe([
        observation(PlayerObservationKind.startFile),
        observation(PlayerObservationKind.fileLoaded),
        observation(PlayerObservationKind.position, 3, BigInt.zero),
      ]);
      expect(batch.snapshot, isNull);
      batch = session.observe([
        observation(PlayerObservationKind.position, 4, ticket.generation),
      ]);
      expect(batch.snapshot?.positionMs, BigInt.from(4000));
      final seek = session.prepare(
        PlayerIntent(
          kind: PlayerIntentKind.seek,
          positionMs: BigInt.from(9000),
        ),
      );
      expect(
        playerPortBegin(
          sessionId: session.id,
          generation: seek.generation,
        ).errorKind,
        isNull,
      );
      expect(
        session.observe([
          observation(PlayerObservationKind.playbackRestart),
        ]).confirmed,
        isEmpty,
      );
      batch = session.observe([
        observation(PlayerObservationKind.seek),
        observation(PlayerObservationKind.playbackRestart),
      ]);
      expect(batch.confirmed, [seek.generation]);
      expect(batch.snapshot, isNull);
      expect(
        session.observe([
          observation(PlayerObservationKind.position, 5, ticket.generation),
        ]).snapshot,
        isNull,
      );
      expect(
        session
            .observe([
              observation(PlayerObservationKind.position, 9, seek.generation),
            ])
            .snapshot
            ?.positionMs,
        BigInt.from(9000),
      );
      session.close();
      expect(playerPortSessionCount(), baseline);
      expect(
        playerPortObserve(sessionId: session.id, observations: []).errorKind,
        'closed',
      );
    },
    skip: !enabled,
  );

  test('logical close invalidates immediately but physical disposal waits for open', () async {
    final backend = Backend()..blocked = Completer<void>();
    final session = RustPlayerPortSession();
    final adapter = FlutterMediaKitAdapter(
      session: session,
      backend: backend,
      automaticPolling: false,
    );
    final ticket = session.prepare(openIntent('fixture.mp4'));
    final opening = adapter.execute(ticket);
    await Future<void>.delayed(Duration.zero);
    expect(backend.commands, hasLength(1));
    final closing = adapter.dispose();
    expect(session.isCurrent(ticket), isFalse);
    expect(backend.disposed, isFalse);
    backend.blocked!.complete();
    expect(await opening, isFalse);
    await closing;
    expect(backend.disposed, isTrue);
  }, skip: !enabled);

  test(
    'timeout poisons backend and late A confirmation cannot complete B',
    () async {
      final backend = Backend();
      final session = RustPlayerPortSession();
      final adapter = FlutterMediaKitAdapter(
        session: session,
        backend: backend,
        automaticPolling: false,
        confirmationTimeout: const Duration(milliseconds: 10),
      );
      final first = session.prepare(openIntent('A.mp4'));
      await expectLater(
        adapter.execute(first),
        throwsA(isA<TimeoutException>()),
      );
      expect(adapter.isUsable, isFalse);
      expect(() => session.prepare(openIntent('B.mp4')), throwsStateError);
      backend.pending.addAll([
        observation(PlayerObservationKind.startFile),
        observation(PlayerObservationKind.fileLoaded),
      ]);
      adapter.poll();
      expect(backend.commands, hasLength(1));
      await adapter.dispose();
    },
    skip: !enabled,
  );

  test('queued stale commands never enter backend', () async {
    final backend = Backend();
    final session = RustPlayerPortSession();
    final adapter = FlutterMediaKitAdapter(
      session: session,
      backend: backend,
      automaticPolling: false,
    );
    final old = session.prepare(openIntent('old.mp4'));
    final fresh = session.prepare(openIntent('fresh.mp4'));
    expect(await adapter.execute(old), isFalse);
    final future = adapter.execute(fresh);
    await Future<void>.delayed(Duration.zero);
    backend.pending.addAll([
      observation(PlayerObservationKind.startFile),
      observation(PlayerObservationKind.fileLoaded),
    ]);
    adapter.poll();
    expect(await future, isTrue);
    expect(backend.commands.map((c) => c.mediaUri), ['fresh.mp4']);
    await adapter.dispose();
  }, skip: !enabled);
  test('network failure permits guarded backend replacement retry', () async {
    final backend = Backend();
    final session = RustPlayerPortSession();
    final adapter = FlutterMediaKitAdapter(
      session: session,
      backend: backend,
      automaticPolling: false,
    );
    final ticket = session.prepare(openIntent('fixture.mp4'));
    final opening = adapter.execute(ticket);
    final failure = expectLater(opening, throwsStateError);
    await Future<void>.delayed(Duration.zero);
    backend.pending.addAll([
      observation(PlayerObservationKind.startFile),
      observation(PlayerObservationKind.endFile, -13),
    ]);
    adapter.poll();
    await failure;
    expect(adapter.isUsable, isFalse);
    expect(
      playerPortShouldRetry(
        sessionId: session.id,
        generation: ticket.generation,
      ),
      isTrue,
    );
    await adapter.dispose();
    expect(
      playerPortShouldRetry(
        sessionId: session.id,
        generation: ticket.generation,
      ),
      isFalse,
    );
  }, skip: !enabled);

  test(
    'a later source can adopt the backend while surface initialization awaits',
    () async {
      final backend = Backend()..blocked = Completer<void>();
      final session = RustPlayerPortSession();
      final adapter = FlutterMediaKitAdapter(
        session: session,
        backend: backend,
        automaticPolling: false,
      );
      session.prepare(openIntent('A.mp4'));
      final attaching = adapter.send(
        PlayerIntent(kind: .attachSurface, surfaceId: BigInt.one),
      );
      await Future<void>.delayed(Duration.zero);
      final b = session.prepare(openIntent('B.mp4'));
      backend.blocked!.complete();
      expect(await attaching, isFalse);
      // 失效的是 A 的资源意图；平台后端与 Rust 会话仍能由 B 接管。
      expect(session.isCurrent(b), isTrue);
      backend.blocked = null;
      final opening = adapter.execute(b);
      await Future<void>.delayed(Duration.zero);
      backend.pending.addAll([
        observation(PlayerObservationKind.startFile),
        observation(PlayerObservationKind.fileLoaded),
      ]);
      adapter.poll();
      expect(await opening, isTrue);
      await adapter.dispose();
    },
    skip: !enabled,
  );
}
