import 'dart:convert';
import 'dart:io';

import 'package:kilikili/utils/subtitle_format.dart';
import 'package:flutter_test/flutter_test.dart';

/// 字幕 fixture 的 golden 是给 Rust 当裁判用的
/// （`rust/crates/bili-core/tests/subtitle_golden.rs`）。裁判本身得有人
/// 看着：本测试用基线的 `json2Vtt` / `json2Srt` 重新转一遍同一份输入，
/// 核对 golden 确实是基线会得出的结果。
///
/// 本测试只 import 纯 dart 的 `subtitle_format.dart`，因此在**没有打补丁
/// 的 Flutter SDK 上也能跑**（`SubtitleUtils` 经 http/init.dart 依赖 dio
/// 与 flutter，那条路径只能在 CI 上验证）。`SubtitleUtils.json2Vtt` 现在
/// 只是转调这里的同名函数，一行委托，编译期就能看出有没有接错。
void main() {
  const fixtureDir = 'rust/crates/bili-core/tests/fixtures/subtitle';

  late List<File> sources;

  setUpAll(() {
    sources =
        Directory(fixtureDir)
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.source.json'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
  });

  test('fixture 目录非空，否则本测试形同虚设', () {
    expect(sources, isNotEmpty);
  });

  test('golden 与基线的转换结果一致', () {
    for (final source in sources) {
      final body =
          (jsonDecode(source.readAsStringSync()) as Map<String, dynamic>)['body']
              as List;
      final stem = source.path.replaceFirst('.source.json', '');

      expect(
        File('$stem.vtt').readAsStringSync(),
        json2Vtt(body),
        reason: '${source.path}: WebVTT golden 不是基线转出来的',
      );
      expect(
        File('$stem.srt').readAsStringSync(),
        json2Srt(body),
        reason: '${source.path}: SRT golden 不是基线转出来的',
      );
    }
  });

  group('基线行为的回归锚点', () {
    // 这些值是 Rust 侧 `to_fixed_3` 必须对齐的那些点：Dart 的
    // `toStringAsFixed` 按**精确值**四舍五入且平局进位，Rust 的 `{:.3}`
    // 平局取偶。写在这里，是为了基线哪天换了实现能立刻看出来。
    test('toStringAsFixed(3) 在精确平局上进位', () {
      expect(0.0625.toStringAsFixed(3), '0.063');
      expect(0.1875.toStringAsFixed(3), '0.188');
    });

    test('看的是精确值而不是写出来的那串十进制', () {
      expect(1.0005.toStringAsFixed(3), '1.000');
      expect(9.9995.toStringAsFixed(3), '9.999');
      expect(0.0015.toStringAsFixed(3), '0.002');
    });

    test('SRT 的毫秒会进到 1000（基线既有行为）', () {
      expect(srtTimecode(1.9996), '00:00:01,1000');
    });

    test('VTT 的秒会进到 60（基线既有行为）', () {
      expect(vttTimecode(59.9999), '00:00:60.000');
    });

    test('空列表：VTT 只有文件头，SRT 为空串', () {
      expect(json2Vtt(const []), 'WEBVTT\n\n');
      expect(json2Srt(const []), '');
    });
  });
}
