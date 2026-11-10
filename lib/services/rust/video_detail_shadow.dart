import 'dart:convert';

import 'package:kilikili/rust/api/video_detail.dart' as rust_api;
import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:kilikili/services/rust/video_detail_normalize.dart';
import 'package:flutter/foundation.dart';

/// 视频详情的 shadow mode 比对（RFC-0001 §5.3）。
///
/// 规则与边界：
///
/// - **只读接口、只比对、不显示**：显示路径仍是 Dart 的
///   `VideoDetailData.fromJson`，本类的结果不进入任何 UI。
/// - **不重复发请求**：比对的是同一份已经拿到的响应。RFC 禁止对写操作
///   重复执行，而对只读接口重复请求同样会平白加倍流量、徒增风控风险，
///   且两次请求的响应本就可能不同，差异无法归因到解析。
/// - **绝不抛出**：比对是诊断设施。它若能让详情页崩掉，就是用一个尚未
///   投入使用的组件去降低线上可用性。
abstract final class VideoDetailShadow {
  /// 单次运行最多报告多少条差异。
  ///
  /// 协议一旦变化，差异会出现在每一次请求上。不设上限的话，日志会被同
  /// 一个问题刷满，真正的第二个问题反而看不见。
  static const int _maxReports = 20;
  static int _reported = 0;

  /// 比对一次详情响应。
  ///
  /// [body] 是**整个响应体**已解码的 Map（dio 已完成 JSON 解码），含
  /// `code` / `data`。调用方无需判断开关，本方法自行短路。
  ///
  /// 调用方应当 `unawaited(...)`：比对不在详情页的关键路径上，不该让
  /// 页面多等一次解析。
  static Future<void> compare(Map<String, dynamic> body) async {
    if (!RustFlags.shadowVideoDetail) return;
    if (_reported >= _maxReports) return;

    try {
      final data = body['data'];
      if (data is! Map<String, dynamic>) return;

      // 重新编码：dio 已经把响应解码成了 Map，原始字节不再可得。对规范化
      // 语义而言这是等价输入——本次比对要验证的是「同一份数据两种实现
      // 是否得出同一结果」，不是 JSON 解码器的行为差异。
      final reencoded = jsonEncode(body);

      final rust = await rust_api.parseVideoDetail(body: reencoded);
      final rustJson = rust.json;
      if (rustJson == null) {
        _report(
          'Rust 解析失败: ${rust.errorKind} / ${rust.errorDetail}',
        );
        return;
      }

      final diff = _firstDifference(
        jsonDecode(rustJson),
        normalizeVideoDetail(data),
        r'$',
      );
      if (diff != null) {
        _report(diff);
      } else if (kDebugMode) {
        debugPrint('[shadow] 视频详情解析一致');
      }
    } catch (e, st) {
      // 包括 Rust 未就绪、桥接异常、本地 JSON 异常在内的一切意外。
      if (kDebugMode) debugPrint('VideoDetailShadow 比对自身出错: $e\n$st');
    }
  }

  static void _report(String message) {
    _reported++;
    final suffix = _reported >= _maxReports ? '（已达上限，后续不再报告）' : '';
    // 用 debugPrint 而非上报服务：Stage 1 的差异是开发期信号，不是用户
    // 可见故障；接上报之前先要有脱敏与采样策略。
    debugPrint('[shadow] 视频详情解析差异: $message$suffix');
  }

  /// 找出两份已规范化结果的第一处不同，返回其路径。
  ///
  /// 只报第一处：协议变更往往牵连整棵子树，全量 diff 会把一行有用信息
  /// 埋进几十行噪声。
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

  /// 截断差异值。日志里出现的是稿件标题、简介这类用户内容，没有截断
  /// 的话一条差异就能刷出几屏。
  static String _brief(dynamic v) {
    final s = v is String ? '"$v"' : '$v';
    return s.length <= 80 ? s : '${s.substring(0, 80)}…';
  }
}
