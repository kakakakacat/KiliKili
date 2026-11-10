// 生成视频详情 golden fixture：把 *.raw.json 经基线规范化规则转成
// *.normalized.json，供 Rust 侧 `bili-api` 对照。
//
// 规范化规则来自 lib/services/rust/video_detail_normalize.dart——**与线上
// shadow mode 用的是同一份**。若在这里另写一份，fixture 可能常绿而线上
// 天天报差异（或反之），对照就失去了意义。
//
// 该文件刻意不依赖 flutter / hive，因此本脚本可用纯 dart 运行。
//
// 用法（仓库根目录）: dart tool/fixture-gen/video_detail_fixture.dart
import 'dart:convert';
import 'dart:io';

import 'package:kilikili/services/rust/video_detail_normalize.dart';

const _fixtureDir = 'rust/crates/bili-api/tests/fixtures/video_detail';

void main() {
  final dir = Directory(_fixtureDir);
  if (!dir.existsSync()) {
    stderr.writeln('找不到 $_fixtureDir，请在仓库根目录运行');
    exit(1);
  }

  final raws =
      dir
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.raw.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));

  if (raws.isEmpty) {
    stderr.writeln('$_fixtureDir 下没有 *.raw.json，fixture 对照会形同虚设');
    exit(1);
  }

  for (final raw in raws) {
    final body = jsonDecode(raw.readAsStringSync()) as Map<String, dynamic>;
    if (body['code'] != 0) {
      stderr.writeln('${raw.path}: code 非 0 的样本不产出规范化结果');
      exit(1);
    }
    final out = normalizeVideoDetail(body['data'] as Map<String, dynamic>);
    final target = raw.path.replaceFirst(
      RegExp(r'\.raw\.json$'),
      '.normalized.json',
    );
    File(
      target,
    ).writeAsStringSync('${const JsonEncoder.withIndent('  ').convert(out)}\n');
    stderr.writeln('写入 $target');
  }
}
