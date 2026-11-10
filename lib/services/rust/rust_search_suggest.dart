import 'dart:convert';

import 'package:PiliPlus/rust/api/search_suggest.dart' as rust_api;
import 'package:PiliPlus/services/rust/rust_flags.dart';
import 'package:flutter/foundation.dart';

/// 搜索建议的 Rust 解析路径。只处理同一份 Dart 已取回的响应。
abstract final class RustSearchSuggest {
  static Future<String?> parse(Map<String, dynamic> body) async {
    if (!RustFlags.rustSearchSuggest) return null;
    try {
      final parsed = await rust_api.parseSearchSuggest(body: jsonEncode(body));
      if (parsed.json == null && kDebugMode) {
        debugPrint(
          '[rust] 搜索建议解析失败，降级回 Dart: '
          '${parsed.errorKind} / ${parsed.errorDetail}',
        );
      }
      return parsed.json;
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 搜索建议桥接失败，降级回 Dart: $e');
      return null;
    }
  }
}
