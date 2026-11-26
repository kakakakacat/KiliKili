import 'dart:async';

import 'package:kilikili/rust/api/player_port.dart';

/// 每个播放控制器拥有独立 Rust 会话，平台初始化前即可预留播放意图。
class RustPlayerPortSession {
  final BigInt id = playerPortCreate();
  BigInt generation = BigInt.zero;
  bool _closed = false;

  PlayerCommandTicket prepare(PlayerIntent intent) {
    if (_closed) throw StateError('Player Port 已关闭');
    final result = playerPortPrepare(sessionId: id, intent: intent);
    final ticket = result.data;
    if (ticket == null) throw StateError('Player Port: ${result.errorKind}');
    generation = ticket.generation;
    return ticket;
  }

  bool isCurrent(PlayerCommandTicket ticket) =>
      !_closed &&
      ticket.sessionId == id &&
      playerPortIsCurrent(sessionId: id, generation: ticket.generation);

  PlayerPortBatch observe(List<PlayerObservation> observations) {
    final result = playerPortObserve(sessionId: id, observations: observations);
    return result.data ??
        (throw StateError('Player Port: ${result.errorKind}'));
  }

  void abort(BigInt generation) {
    if (!_closed) playerPortAbort(sessionId: id, generation: generation);
  }

  void close() {
    if (_closed) return;
    _closed = true;
    playerPortClose(sessionId: id);
  }
}

/// 此接口只隔离平台调用；代际、资源编码和状态判断全部经过 Rust。
abstract interface class PlayerPortBackend {
  List<PlayerObservation> drain();
  void sample(BigInt generation);
  Future<void> execute(
    PlayerCommandTicket ticket, {
    Duration? start,
    Map<String, String>? extras,
    bool play = false,
  });
  void detachObserver();
  Future<void> dispose();
}

class FlutterMediaKitAdapter {
  FlutterMediaKitAdapter({
    required this.session,
    required this.backend,
    bool automaticPolling = true,
    this.confirmationTimeout = const Duration(seconds: 15),
  }) {
    if (automaticPolling) {
      _timer = Timer.periodic(const Duration(milliseconds: 40), (_) => poll());
    }
  }

  final RustPlayerPortSession session;
  final PlayerPortBackend backend;
  final Duration confirmationTimeout;
  final _events = StreamController<PlayerPortBatch>.broadcast(sync: true);
  Stream<PlayerPortBatch> get events => _events.stream;

  /// 后端被永久停用（确认超时、执行出错、事件溢出）时触发一次。
  ///
  /// 按 ADR-003，停用后只能新建后端恢复，而且超时不授予自动重试（Rust
  /// `should_retry` 对超时恒为 false）。所以这里只**通知**，不重试：宿主据此
  /// 退出「播放中」，给出明确的重试入口（#244）。之前停用是静默的——轮询一停，
  /// UI 就永远停在最后一张快照上。
  final _faults = StreamController<void>.broadcast(sync: true);
  Stream<void> get faults => _faults.stream;
  bool _awaiting = false;
  Timer? _timer;
  Future<void> _tail = Future.value();
  Future<void>? _disposing;
  Completer<void>? _confirmation;
  BigInt? _confirming;
  bool _closed = false;
  bool _faulted = false;
  bool get isUsable => !_closed && !_faulted;

  /// 40ms 是采样周期，不是第二个播放时钟。每批只跨一次 FRB。
  void poll() {
    if (!isUsable) return;
    final batch = session.observe(backend.drain());
    for (final failure in batch.failures) {
      if (failure.generation == _confirming) {
        _confirmation?.completeError(StateError('播放器: ${failure.kind.name}'));
        _confirmation = null;
      }
    }
    if (batch.confirmed.contains(_confirming)) {
      _confirmation?.complete();
      _confirmation = null;
    }
    // 待确认期间没有快照也没有失败，只有 awaiting 在变；不推送的话宿主
    // 看不到「命令还在路上」。
    final awaitingChanged = batch.awaiting != _awaiting;
    _awaiting = batch.awaiting;
    if (batch.snapshot != null ||
        batch.failures.isNotEmpty ||
        awaitingChanged) {
      _events.add(batch);
    }
    if (!playerPortIsCurrent(
      sessionId: session.id,
      generation: session.generation,
    )) {
      _fault();
    }
    if (isUsable) backend.sample(batch.emittingGeneration);
  }

  Future<bool> send(PlayerIntent intent) {
    if (!isUsable) return Future.value(false);
    return execute(session.prepare(intent));
  }

  /// 队列中的命令仍带调用时的戳，不能等执行时再盖当前代际。
  Future<bool> execute(
    PlayerCommandTicket ticket, {
    Duration? start,
    Map<String, String>? extras,
    bool play = false,
  }) {
    final result = _tail.then((_) async {
      if (!isUsable || !session.isCurrent(ticket)) return false;
      final discontinuity = switch (ticket.intent.kind) {
        PlayerIntentKind.open ||
        PlayerIntentKind.seek ||
        PlayerIntentKind.selectTracks => true,
        _ => false,
      };
      Completer<void>? confirmation;
      if (discontinuity) {
        poll();
        if (!isUsable || !session.isCurrent(ticket)) return false;
        final begun = playerPortBegin(
          sessionId: session.id,
          generation: ticket.generation,
        );
        if (begun.errorKind != null) return false;
        confirmation = _confirmation = Completer<void>();
        _confirming = ticket.generation;
        // 在后端 Future 之前监听错误，避免轮询先报失败时出现未处理异常。
        unawaited(
          confirmation.future.then<void>(
            (_) {},
            onError: (Object _, StackTrace _) {},
          ),
        );
      }
      try {
        await backend.execute(ticket, start: start, extras: extras, play: play);
        if (!isUsable) return false;
        if (confirmation != null) {
          await confirmation.future.timeout(confirmationTimeout);
        }
        return isUsable && session.isCurrent(ticket);
      } catch (_) {
        if (isUsable) {
          session.abort(ticket.generation);
          _fault();
        }
        rethrow;
      } finally {
        if (_confirming == ticket.generation) {
          _confirming = null;
          _confirmation = null;
        }
      }
    });
    _tail = result.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return result;
  }

  void _fault() {
    if (!isUsable) return;
    _faulted = true;
    _timer?.cancel();
    _awaiting = false;
    _faults.add(null);
  }

  Future<void> dispose() => _disposing ??= _dispose();

  Future<void> _dispose() async {
    // 逻辑销毁必须同步完成；物理销毁等库内 open/seek 的 continuation 退出。
    _closed = true;
    _timer?.cancel();
    session.close();
    backend.detachObserver();
    _confirmation?.complete();
    _confirmation = null;
    await _tail;
    await backend.dispose();
    await _events.close();
    await _faults.close();
  }
}
