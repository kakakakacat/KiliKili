import 'dart:convert';
import 'dart:io';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/services/rust/danmaku_normalize.dart';
import 'package:flutter_test/flutter_test.dart';

/// 弹幕 fixture 的 golden 是给 Rust 解码器当裁判用的
/// （`rust/crates/bili-api/tests/danmaku_seg_golden.rs`）。裁判本身得有人
/// 看着：本测试用**基线**的 protobuf 解码器重新解一遍同一份字节，核对
/// golden 确实是基线会得出的结果。
///
/// 没有这个测试，`.normalized.json` 就只是一份来历不明的期望值——Rust
/// 全绿只能证明它与那份文件一致，证明不了它与基线一致。
void main() {
  const fixtureDir = 'rust/crates/bili-api/tests/fixtures/danmaku_seg';

  late List<File> raws;

  setUpAll(() {
    raws =
        Directory(fixtureDir)
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.raw.pb'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
  });

  test('fixture 目录非空，否则本测试形同虚设', () {
    expect(raws, isNotEmpty);
  });

  test('golden 与基线解码结果一致', () {
    for (final raw in raws) {
      final reply = DmSegMobileReply.fromBuffer(raw.readAsBytesSync());
      final golden = jsonDecode(
        File(
          raw.path.replaceFirst('.raw.pb', '.normalized.json'),
        ).readAsStringSync(),
      );

      expect(
        normalizeDanmakuSegment(reply),
        golden,
        reason: '${raw.path}: golden 不是基线解出来的结果',
      );
    }
  });

  test('raw.pb 是基线编码器写出的规范形式', () {
    // 解码再编码应当字节不变。不成立说明 fixture 里的字节不是
    // protobuf.dart 会写出的形式（字段乱序、写了默认值、长度前缀多占了
    // 字节……），那样 Rust 即使全绿，也只是对着一份线上不会出现的输入
    // 全绿。
    for (final raw in raws) {
      final bytes = raw.readAsBytesSync();
      expect(
        DmSegMobileReply.fromBuffer(bytes).writeToBuffer(),
        bytes,
        reason: '${raw.path}: 不是规范编码，请用 fixture 生成器重新生成',
      );
    }
  });

  test('服务端不下发的本地字段不出现在 golden 里', () {
    // 100 / 101 是上游在生成代码里追加的客户端字段（count / isSelf）。
    // 它们若混进 fixture，Rust 侧会因为解不出它们而「差异」，而那是
    // 规范化规则的错，不是解码器的错。
    for (final raw in raws) {
      for (final elem in DmSegMobileReply.fromBuffer(
        raw.readAsBytesSync(),
      ).elems) {
        expect(elem.count, 0, reason: '${raw.path}: fixture 不应带 count');
        expect(elem.isSelf, false, reason: '${raw.path}: fixture 不应带 isSelf');
      }
    }
  });
}
