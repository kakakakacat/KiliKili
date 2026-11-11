/// 基线的字幕格式转换：B 站的字幕 JSON（`body` 数组）→ WebVTT / SRT。
///
/// 这几个函数原本写在 `SubtitleUtils` 里，逐字搬到这里，**行为未作任何
/// 改动**。搬出来的理由与 `lib/pages/danmaku/engine.dart` 相同：Rust 侧
/// 要与它对齐（`bili-core::subtitle_format`），而对齐靠同一份 fixture 两边
/// 各跑一遍（`tool/fixture-gen/subtitle_fixture.dart`）。生成器是纯 dart
/// 脚本，import 不了 `SubtitleUtils`——那个类经 `http/init.dart` 依赖 dio
/// 与 flutter。
///
/// 本文件**不得** import flutter 或 dio。
library;

import 'package:collection/collection.dart' show IterableExtension;

/// WebVTT 的时间戳：`HH:MM:SS.mmm`。
///
/// 注意秒是 `toStringAsFixed(3)` 之后再 `padLeft(6, '0')`：不足两位整数秒
/// 的会被补成 `05.500`。
String vttTimecode(num seconds) {
  final h = (seconds ~/ 3600).toString().padLeft(2, '0');
  seconds %= 3600;
  final m = (seconds ~/ 60).toString().padLeft(2, '0');
  seconds %= 60;
  final sms = seconds.toStringAsFixed(3).padLeft(6, '0');
  return "$h:$m:$sms";
}

String json2Vtt(List list) {
  final sb = StringBuffer('WEBVTT\n\n')
    ..writeAll(
      list.map(
        (item) =>
            '${vttTimecode(item['from'])} --> ${vttTimecode(item['to'])}\n${item['content'].trim()}',
      ),
      '\n\n',
    );
  return sb.toString();
}

/// SRT 的时间戳：`HH:MM:SS,mmm`。
///
/// 毫秒走的是 `((seconds - s) * 1000).round()`，与 VTT 的
/// `toStringAsFixed(3)` 不是同一套舍入——两者在边界上可能差 1 毫秒，这是
/// 基线的既有行为，不要「顺手修掉」。
String srtTimecode(num seconds) {
  final h = (seconds ~/ 3600).toString().padLeft(2, '0');
  seconds %= 3600;
  final m = (seconds ~/ 60).toString().padLeft(2, '0');
  seconds %= 60;
  final s = seconds.toInt();
  final ms = ((seconds - s) * 1000).round().toString().padLeft(3, '0');
  return '$h:$m:${s.toString().padLeft(2, '0')},$ms';
}

String json2Srt(List list) {
  final sb = StringBuffer()
    ..writeAll(
      list.mapIndexed(
        (i, e) =>
            '${i + 1}\n${srtTimecode(e['from'])} --> ${srtTimecode(e['to'])}\n${e['content'].trim()}',
      ),
      '\n\n',
    );
  return sb.toString();
}
