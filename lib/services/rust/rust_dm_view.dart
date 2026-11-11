import 'dart:convert';

import 'package:kilikili/models_new/video/video_play_info/subtitle.dart';
import 'package:kilikili/rust/api/dm_view.dart' as rust_api;
import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:flutter/foundation.dart';

/// `DmView` 字幕轨的 Rust 解析路径。只处理同一份 Dart 已取回的响应字节。
abstract final class RustDmView {
  /// 解析字幕轨。返回 null 表示降级回 Dart（开关关闭、Rust 未就绪、
  /// 解析失败），**空列表则是「这个视频没有字幕」这个确定的结论**。
  static Future<List<Subtitle>?> parseSubtitles(Uint8List bytes) async {
    if (!RustFlags.rustDmViewSubtitle) return null;
    try {
      final parsed = await rust_api.parseDmViewSubtitles(bytes: bytes);
      final json = parsed.json;
      if (json == null) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 字幕解析失败，降级回 Dart: '
            '${parsed.errorKind} / ${parsed.errorDetail}',
          );
        }
        return null;
      }
      return (jsonDecode(json) as List)
          .cast<Map<String, dynamic>>()
          .map(_toSubtitle)
          .toList();
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 字幕桥接失败，降级回 Dart: $e');
      return null;
    }
  }
}

/// 字段与基线 `DmGrpc.dmViewToSubtitle` 构造的 `Subtitle` 一一对应；
/// 顺序由 Rust 侧按基线的 `compareTo` 排好，这里不再排。
Subtitle _toSubtitle(Map<String, dynamic> json) => Subtitle(
  id: json['id'] as int,
  lan: json['lan'] as String,
  lanDoc: json['lan_doc'] as String,
  subtitleUrl: json['subtitle_url'] as String,
  type: json['subtitle_type'] as int,
  isAi: json['is_ai'] as bool,
);
