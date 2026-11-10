// 生成取流 golden fixture：把 *.raw.json 经基线规范化规则转成
// *.normalized.json，供 Rust 侧 `bili-api` 对照。
//
// 规范化规则来自 lib/services/rust/play_url_normalize.dart——**与线上
// shadow mode 用的是同一份**。若在这里另写一份，fixture 可能常绿而线上
// 天天报差异（或反之），对照就失去了意义。
//
// 样本的文件名要指出响应形状，因为响应本身不足以区分（PUGV 降级与 UGC
// 的外层形状相同）：
//
//     <名字>.<ugc|pgc|pugv>[-fallback].raw.json
//
// 该文件刻意不依赖 flutter / hive，因此本脚本可用纯 dart 运行。
//
// 用法（仓库根目录）: dart tool/fixture-gen/play_url_fixture.dart
import 'dart:convert';
import 'dart:io';

import 'package:kilikili/services/rust/play_url_normalize.dart';

const _fixtureDir = 'rust/crates/bili-api/tests/fixtures/play_url';

/// 从文件名解析出响应形状。
(String videoType, bool fallback)? shapeOf(String fileName) {
  final match = RegExp(
    r'\.(ugc|pgc|pugv)(-fallback)?\.raw\.json$',
  ).firstMatch(fileName);
  if (match == null) return null;
  return (match.group(1)!, match.group(2) != null);
}

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
    final name = raw.uri.pathSegments.last;
    final shape = shapeOf(name);
    if (shape == null) {
      stderr.writeln('$name 的文件名没有指出响应形状，见本文件头部的命名约定');
      exit(1);
    }

    final body = jsonDecode(raw.readAsStringSync()) as Map<String, dynamic>;
    final out = normalizePlayUrl(body, PlayUrlShape.of(shape.$1, shape.$2));
    if (out == null) {
      stderr.writeln('$name: 规范化为空（非 0 码或缺流），不产出期望值');
      exit(1);
    }

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
