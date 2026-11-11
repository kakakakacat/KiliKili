import 'package:kilikili/rust/api/subtitle.dart' as rust_api;
import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:kilikili/utils/subtitle_utils.dart' show SubtitleFormat;
import 'package:flutter/foundation.dart';

/// 字幕转换的 Rust 路径：字幕文件正文 → WebVTT / SRT。
///
/// 与其他 Rust 封装的区别是它有**三种**结果，调用方必须分开处理：
///
/// - 返回 null：失败或开关关闭 → 走 Dart 原路径（同一份正文，不重新请求）
/// - 返回 `text == null`：响应里没有 `body`，基线此时也返回 null →
///   当作「这份字幕没有内容」，**不要**降级重试
/// - 返回 `text != null`：转换好的文本
abstract final class RustSubtitle {
  static Future<({String? text})?> convert(
    String body,
    SubtitleFormat format,
  ) async {
    if (!RustFlags.rustSubtitleConvert) return null;
    try {
      final result = await rust_api.convertSubtitle(
        body: body,
        // 下标即约定，见 Rust 侧 `convert_subtitle` 的文档。
        format: format.index,
      );
      if (result.missingBody) return (text: null);
      if (result.text == null) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 字幕转换失败，降级回 Dart: '
            '${result.errorKind} / ${result.errorDetail}',
          );
        }
        return null;
      }
      return (text: result.text);
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 字幕转换桥接失败，降级回 Dart: $e');
      return null;
    }
  }
}
