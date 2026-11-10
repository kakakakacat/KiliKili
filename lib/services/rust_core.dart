import 'package:kilikili/rust/api/init.dart' as rust_api;
import 'package:kilikili/rust/frb_generated.dart';
import 'package:flutter/foundation.dart';

/// Rust Core 的启动入口。
///
/// Stage 1（RFC-0001 §5）中 Rust 尚未接管任何关键路径，Dart 实现仍是
/// 唯一在用的实现。因此初始化失败**不得**让 App 启动失败——否则一个
/// 尚未投入使用的组件就能让整个应用打不开，这与 §5.3 要求的
/// 「发生故障可降级回 Dart」正好相反。
///
/// 失败时记录状态并继续，由 [RustCore.isReady] 决定是否走 Rust 路径。
abstract final class RustCore {
  static bool _ready = false;
  static String? _version;
  static Object? _error;

  /// Rust Core 是否可用。为 false 时调用方必须走 Dart 实现。
  static bool get isReady => _ready;

  /// Rust 侧版本号，用于核对桥接两端是否同源。未就绪时为 null。
  static String? get version => _version;

  /// 初始化失败的原因，供诊断与问题上报。未失败时为 null。
  static Object? get error => _error;

  /// 初始化 Rust Core。可安全重复调用。
  ///
  /// 典型失败原因是原生库未被打进产物（cargokit 未参与构建），
  /// 此时 [RustLib.init] 会抛出找不到动态库的异常。
  static Future<void> init() async {
    if (_ready) return;
    try {
      await RustLib.init();
      // 日志级别保持克制：Rust 侧日志会进 logcat / 控制台，
      // debug 级在弹幕与播放事件密集时会淹没其他输出。
      _version = await rust_api.initApp(
        maxLevel: kDebugMode ? 'info' : 'warn',
      );
      _ready = true;
      if (kDebugMode) debugPrint('RustCore ready, version $_version');
    } catch (e, st) {
      _error = e;
      _ready = false;
      // 不上报到崩溃服务：Stage 1 中这不是用户可见故障，
      // 且在未集成原生库的开发环境中属预期情况。
      if (kDebugMode) debugPrint('RustCore unavailable: $e\n$st');
    }
  }
}
