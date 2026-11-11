import 'dart:convert';
import 'dart:io';

import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:kilikili/services/rust/rust_subtitle.dart';
import 'package:kilikili/utils/subtitle_format.dart' as baseline;
import 'package:kilikili/utils/subtitle_utils.dart' show SubtitleFormat;
import 'package:flutter_test/flutter_test.dart';

import 'bridge_env.dart';

/// 字幕转换跨 FFI 的集成测试。
///
/// 两道判据：与 fixture 的 `.vtt` / `.srt` 逐字节相同（那是基线自己写出
/// 来的），以及与基线 `json2Vtt` / `json2Srt` 当场跑出来的结果相同。
/// 前者钉住 Rust 侧，后者保证 fixture 没有和基线一起漂走。
void main() {
  const fixtureDir = 'rust/crates/bili-core/tests/fixtures/subtitle';

  late List<File> sources;

  setUpAll(() async {
    if (!bridgeTestsRequested) return;
    await initRustBridge();
    await RustFlags.setRustSubtitleConvert(true);
    expect(RustFlags.rustSubtitleConvert, isTrue, reason: '开关没打开');
    sources =
        Directory(fixtureDir)
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.source.json'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
  });

  tearDownAll(() async {
    if (!bridgeTestsRequested) return;
    await closeRustBridge();
  });

  test('Rust 转出的 VTT / SRT 与 fixture 和基线三方一致', () async {
    expect(sources, isNotEmpty, reason: 'fixture 没找到，本测试形同虚设');

    for (final source in sources) {
      final body = source.readAsStringSync();
      final stem = source.path.replaceFirst('.source.json', '');
      final list = (jsonDecode(body) as Map<String, dynamic>)['body'] as List;

      for (final (format, ext, fromDart) in [
        (SubtitleFormat.vtt, 'vtt', baseline.json2Vtt(list)),
        (SubtitleFormat.srt, 'srt', baseline.json2Srt(list)),
      ]) {
        final result = await RustSubtitle.convert(body, format);
        expect(result, isNotNull, reason: '${source.path}: $ext 降级了');
        expect(
          result!.text,
          File('$stem.$ext').readAsStringSync(),
          reason: '${source.path}: $ext 与 fixture 不一致',
        );
        expect(
          result.text,
          fromDart,
          reason: '${source.path}: $ext 与基线当场跑出来的结果不一致',
        );
      }
    }
  }, skip: bridgeSkip);

  test('没有 body 字段时报「这份字幕没有内容」，不是失败', () async {
    // 这两种结果调用方的处理完全不同：前者直接返回 null 不再重试，
    // 后者要用同一份正文走基线。混淆的后果是对一份空字幕反复转换。
    final result = await RustSubtitle.convert('{"font_size":0.4}', .vtt);
    expect(result, isNotNull, reason: '缺 body 不是失败，不该降级');
    expect(result!.text, isNull, reason: '缺 body 时文本应为 null');
  }, skip: bridgeSkip);

  test('解不动的正文降级回 Dart，而不是抛异常', () async {
    expect(await RustSubtitle.convert('{not json', .vtt), isNull);
  }, skip: bridgeSkip);

  test('开关关闭时不碰 Rust', () async {
    await RustFlags.setRustSubtitleConvert(false);
    addTearDown(() => RustFlags.setRustSubtitleConvert(true));
    expect(
      await RustSubtitle.convert(
        File('$fixtureDir/typical.source.json').readAsStringSync(),
        .vtt,
      ),
      isNull,
      reason: '开关关闭却走了 Rust——降级开关失效',
    );
  }, skip: bridgeSkip);
}
