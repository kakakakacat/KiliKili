import 'dart:async';
import 'dart:io';

/// 进程内的「联网已放行」状态。
///
/// iOS 上由 `NetworkPermissionGate` 在拿到联网授权（或用户选择仍然继续）
/// 后调用 [markReady]；其他平台没有这道授权，一开始就是放行状态。
///
/// 启动早期、不经过页面的请求（如匿名账号取 buvid）应先 `await ready`，
/// 否则在授权弹框期间发出，只会得到一个连接错误。
abstract final class NetworkPermission {
  static final Completer<void> _ready = Completer<void>();

  static bool get isReady => !Platform.isIOS || _ready.isCompleted;

  static Future<void> get ready => isReady ? Future.value() : _ready.future;

  static void markReady() {
    if (!_ready.isCompleted) _ready.complete();
  }
}
