import 'dart:convert';

import 'package:kilikili/models/search/suggest.dart';

/// 优先使用 Rust 的规范化结果；桥接结果无效时沿用基线模型解析。
SearchSuggestModel searchSuggestFromRustOrDart(
  Map<String, dynamic> body,
  String? rustJson,
) {
  if (rustJson != null) {
    try {
      return SearchSuggestModel.fromJson(
        jsonDecode(rustJson) as Map<String, dynamic>,
      );
    } catch (_) {
      // 只丢弃这次 Rust 结果。Dart 基线仍按原有异常语义处理响应。
    }
  }
  return SearchSuggestModel.fromJson(
    body['data']['result'] as Map<String, dynamic>,
  );
}
