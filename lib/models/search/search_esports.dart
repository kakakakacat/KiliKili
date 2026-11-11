import 'dart:convert';

import 'package:kilikili/models_new/match/match_info/contest.dart';
import 'package:flutter/foundation.dart' show debugPrint, kDebugMode;

/// 综合搜索里的赛事卡片。
///
/// ## 为什么这一块还在 Dart 解析
///
/// 它的条目是 `MatchContest`，而那个模型还有一个**未迁**的生产者：
/// `lib/http/match.dart` 的赛事详情（S15 / #66）。按 ADR-011 决策六，共享
/// 运行时模型要等它的所有生产者迁完才能动，所以 Rust 侧只把这一块原样
/// 带回成 JSON 串（`SearchAllData.esportsJson`），由这里喂给既有的
/// `MatchContest.fromSearch`。
///
/// S15 迁完之后，这一块应当一并改成 typed 直连，本文件随之删除。
class SearchEsports {
  List<MatchContest> contest;

  SearchEsports({required this.contest});

  /// 从 Rust 带回的原始 JSON 构造。`json` 为空或结构不符时返回 `null`
  /// ——赛事卡片解析失败不该让整页综合搜索出不来，基线在这里也是
  /// `try { … } catch (_) {}` 吞掉。
  static SearchEsports? fromRustJson(String? json) {
    if (json == null || json.isEmpty) return null;
    try {
      final decoded = jsonDecode(json);
      if (decoded is! Map<String, dynamic>) return null;
      if (decoded['contest'] case final List list) {
        return SearchEsports(
          contest: list
              .map((e) => MatchContest.fromSearch(e as Map<String, dynamic>))
              .toList(),
        );
      }
      return null;
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 赛事卡片解析失败: $e');
      return null;
    }
  }
}
