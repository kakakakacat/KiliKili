import 'dart:convert';
import 'dart:io';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/grpc/dm.dart';
import 'package:kilikili/models_new/video/video_play_info/subtitle.dart';
import 'package:kilikili/services/rust/dm_view_normalize.dart';
import 'package:flutter_test/flutter_test.dart';

/// `dm_view_normalize.dart` 是 `DmGrpc.dmViewToSubtitle` 那段映射的转写
/// （转写的理由见该文件的文档注释）。转写会脱节，本测试是兜底：用**真正
/// 的**基线函数解析同一份 fixture，再逐字段核对规范化器的输出。
///
/// 同时它也是 golden 的裁判：`.normalized.json` 若不是基线会得出的结果，
/// Rust 全绿只能证明它与那份文件一致。
void main() {
  const fixtureDir = 'rust/crates/bili-api/tests/fixtures/dm_view';

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

  test('规范化器与真正的 dmViewToSubtitle 等价', () {
    for (final raw in raws) {
      final reply = DmViewReply.fromBuffer(raw.readAsBytesSync());
      expect(
        normalizeDmViewSubtitles(reply),
        _fromModel(DmGrpc.dmViewToSubtitle(reply)),
        reason: '${raw.path}: 转写与基线脱节了',
      );
    }
  });

  test('golden 与基线结果一致', () {
    for (final raw in raws) {
      final reply = DmViewReply.fromBuffer(raw.readAsBytesSync());
      expect(
        jsonDecode(
          File(
            raw.path.replaceFirst('.raw.pb', '.normalized.json'),
          ).readAsStringSync(),
        ),
        _fromModel(DmGrpc.dmViewToSubtitle(reply)),
        reason: '${raw.path}: golden 不是基线解出来的结果',
      );
    }
  });

  test('raw.pb 是基线编码器写出的规范形式', () {
    for (final raw in raws) {
      final bytes = raw.readAsBytesSync();
      expect(
        DmViewReply.fromBuffer(bytes).writeToBuffer(),
        bytes,
        reason: '${raw.path}: 不是规范编码，请用 fixture 生成器重新生成',
      );
    }
  });

  test('fixture 里有没有消费方的字段，否则跳过逻辑没被测到', () {
    final hasUnconsumed = raws.any((raw) {
      final reply = DmViewReply.fromBuffer(raw.readAsBytesSync());
      return reply.kv.isNotEmpty ||
          reply.specialDms.isNotEmpty ||
          reply.sendBoxStyle != 0 ||
          reply.textPlaceholder.isNotEmpty;
    });
    expect(hasUnconsumed, isTrue);
  });
}

/// 把**真正的**模型实例规范化。读的是实例字段，不是 JSON——基线改了映射
/// 而规范化器没跟上时，两边的值会对不上。
List<Map<String, dynamic>> _fromModel(List<Subtitle>? subtitles) =>
    (subtitles ?? const [])
        .map(
          (s) => {
            'id': s.id ?? 0,
            'lan': s.lan,
            'lan_doc': s.lanDoc ?? '',
            'subtitle_url': s.subtitleUrl ?? '',
            'subtitle_type': s.type ?? 0,
            'is_ai': s.isAi,
          },
        )
        .toList();
