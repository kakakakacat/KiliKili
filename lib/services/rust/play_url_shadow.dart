import 'dart:convert';

import 'package:PiliPlus/models/common/video/video_type.dart';
import 'package:PiliPlus/rust/api/play_url.dart' as rust_api;
import 'package:PiliPlus/services/rust/play_url_normalize.dart';
import 'package:PiliPlus/services/rust/rust_flags.dart';
import 'package:flutter/foundation.dart';

/// 取流的 shadow mode 比对（RFC-0001 §5.3）。
///
/// 规则与边界同 `VideoDetailShadow`：**只比对、不播放，不重复发请求，
/// 绝不抛出**。取流接口上「不重复发请求」尤其要紧——每次请求都会真的
/// 签发一组 CDN 地址并计入风控。
abstract final class PlayUrlShadow {
  /// 单次运行最多报告多少条差异。协议一变，差异会出现在每次取流上。
  static const int _maxReports = 20;
  static int _reported = 0;

  /// 比对一次取流响应。
  ///
  /// [body] 是整个响应体（dio 已完成 JSON 解码），[videoType] 与
  /// [fallback] 指出业务数据在响应里的位置——响应本身不足以区分。
  ///
  /// 调用方应当 `unawaited(...)`：比对不在播放的关键路径上。
  static Future<void> compare(
    Map<String, dynamic> body,
    VideoType videoType,
    bool fallback,
  ) async {
    if (!RustFlags.shadowPlayUrl) return;
    if (_reported >= _maxReports) return;

    try {
      final shape = PlayUrlShape.of(videoType.name, fallback);
      final dart = normalizePlayUrl(body, shape);

      // 重新编码：dio 已经把响应解码成了 Map，原始字节不再可得。对规范化
      // 语义而言这是等价输入。
      final rust = await rust_api.parsePlayUrl(
        body: jsonEncode(body),
        videoType: videoType.name,
        fallback: fallback,
      );

      if (rust.needsFallback) {
        // DRM 内容：Rust 要求降级，Dart 侧的规范化同样给不出流。两端
        // 一致，无可比对。
        if (dart != null) _report('Rust 要求降级，Dart 却规范化出了流');
        return;
      }

      final rustJson = rust.json;
      if (rustJson == null) {
        // Dart 侧也没能规范化出结果时，两端都认为这不是一次成功取流
        // （非 0 码、缺 video_info），不构成差异。
        if (dart != null) {
          _report('Rust 解析失败: ${rust.errorKind} / ${rust.errorDetail}');
        }
        return;
      }
      if (dart == null) {
        _report('Rust 解析出了流，Dart 规范化却为空');
        return;
      }

      final diff = _firstDifference(jsonDecode(rustJson), dart, r'$');
      if (diff != null) {
        _report(diff);
      } else if (kDebugMode) {
        debugPrint('[shadow] 取流解析一致');
      }
    } catch (e, st) {
      if (kDebugMode) debugPrint('PlayUrlShadow 比对自身出错: $e\n$st');
    }
  }

  static void _report(String message) {
    _reported++;
    final suffix = _reported >= _maxReports ? '（已达上限，后续不再报告）' : '';
    // 用 debugPrint 而非上报服务：接上报之前先要有脱敏与采样策略
    // （issue #24）。取流响应里有带鉴权参数的 CDN 地址，更不该外传。
    debugPrint('[shadow] 取流解析差异: $message$suffix');
  }

  /// 找出两份已规范化结果的第一处不同，返回其路径。只报第一处：协议
  /// 变更往往牵连整棵子树。
  static String? _firstDifference(dynamic rust, dynamic dart, String path) {
    if (rust is Map && dart is Map) {
      final keys = {...rust.keys, ...dart.keys}.toList()..sort();
      for (final k in keys) {
        if (!rust.containsKey(k)) return '$path.$k: 仅 Dart 有';
        if (!dart.containsKey(k)) return '$path.$k: 仅 Rust 有';
        final d = _firstDifference(rust[k], dart[k], '$path.$k');
        if (d != null) return d;
      }
      return null;
    }
    if (rust is List && dart is List) {
      if (rust.length != dart.length) {
        return '$path: 长度不同，Rust ${rust.length} / Dart ${dart.length}';
      }
      for (var i = 0; i < rust.length; i++) {
        final d = _firstDifference(rust[i], dart[i], '$path[$i]');
        if (d != null) return d;
      }
      return null;
    }
    if (rust == dart) return null;
    return '$path: Rust ${_brief(rust)} / Dart ${_brief(dart)}';
  }

  /// 截断差异值：CDN 地址动辄几百字符，一条差异就能刷出几屏，而那串
  /// 地址里还带着鉴权参数。
  static String _brief(dynamic v) {
    final s = v is String ? '"$v"' : '$v';
    return s.length <= 80 ? s : '${s.substring(0, 80)}…';
  }
}
