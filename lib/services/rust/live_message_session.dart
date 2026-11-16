import 'dart:async';

import 'package:kilikili/rust/api/live_danmaku.dart';

/// 宿主的订阅寿命。连接策略在 Rust；这里仅防止异步结果更新已销毁的页面。
/// 回调对应可替换的 FFI 入口，离线测试不需要读取账号或初始化全局存储。
class LiveMessageSession {
  LiveMessageSession({
    required this.token,
    required this.open,
    required this.poll,
    required this.closeHandle,
    required this.onBatch,
    required this.onError,
  });

  final Future<LiveDanmakuTokenResult> Function() token;
  final LiveSocketOpenResult Function(LiveDanmakuToken, int) open;
  final LiveSocketPollResult Function(LiveSocketHandle) poll;
  final LiveSocketActionResult Function(LiveSocketHandle) closeHandle;
  final void Function(LiveSocketBatch) onBatch;
  final void Function(String?, String?) onError;
  Timer? _timer;
  LiveSocketHandle? _handle;
  bool _active = false;
  int _generation = 0;

  bool get active => _active;
  int get generation => _generation;
  LiveSocketHandle? get handle => _handle;

  Future<void> start() async {
    if (_active) return;
    _active = true;
    final generation = ++_generation;
    try {
      final response = await token();
      if (!_active || generation != _generation) return;
      final data = response.data;
      if (data == null) {
        _fail(response.errorKind, response.errorDetail);
        return;
      }
      final opened = open(data, generation);
      final handle = opened.data;
      if (handle == null) {
        _fail(opened.errorKind, opened.errorDetail);
        return;
      }
      // open 当前为同步调用；保留此检查，注入入口未来改实现时仍不泄漏句柄。
      if (!_active || generation != _generation) {
        closeHandle(handle);
        return;
      }
      _handle = handle;
      _timer = Timer.periodic(
        const Duration(milliseconds: 100),
        (_) => _poll(generation, handle),
      );
    } catch (_) {
      if (_active && generation == _generation) _fail('network', '直播弹幕连接失败');
    }
  }

  void _poll(int generation, LiveSocketHandle handle) {
    if (!_active || generation != _generation) return;
    try {
      final response = poll(handle);
      if (!_active || generation != _generation) return;
      final batch = response.data;
      if (batch != null &&
          batch.sessionId == handle.sessionId &&
          batch.generation == generation) {
        onBatch(batch);
      }
      if (!_active || generation != _generation) return;
      if (response.errorKind != null) {
        _fail(response.errorKind, response.errorDetail);
      } else if (batch == null || batch.ended) {
        close();
      }
    } catch (_) {
      if (_active && generation == _generation) _fail('network', '直播弹幕读取失败');
    }
  }

  void _fail(String? kind, String? detail) {
    close();
    if (kind != 'cancelled') onError(kind, detail);
  }

  void close() {
    _active = false;
    ++_generation;
    _timer?.cancel();
    _timer = null;
    final handle = _handle;
    _handle = null;
    if (handle != null) closeHandle(handle);
  }
}
