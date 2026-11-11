import 'dart:convert';
import 'dart:io';

import 'package:kilikili/utils/subtitle_format.dart';

/// 字幕转换 golden fixture 的生成器。
///
/// ```text
/// <name>.source.json  字幕文件的正文（与服务端下发的同形）
///   ↓ 基线 json2Vtt / json2Srt
/// <name>.vtt / <name>.srt   期望值，Rust 的输出须逐字节相同
/// ```
///
/// 源文件不经任何改写：Rust 解析的就是这一份，中间没有「规范化」一层。
/// 刁钻的时间戳（精确平局、会让 SRT 毫秒进到 1000 的那个）是**刻意**
/// 放进去的——两端的舍入规则只在那些点上分岔。
const fixtureDir = 'rust/crates/bili-core/tests/fixtures/subtitle';

void main() {
  final sources =
      Directory(fixtureDir)
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.source.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  if (sources.isEmpty) {
    stderr.writeln('没有字幕 fixture');
    exit(1);
  }

  for (final source in sources) {
    final spec = jsonDecode(source.readAsStringSync()) as Map<String, dynamic>;
    final body = spec['body'] as List;
    final stem = source.path.replaceFirst('.source.json', '');
    // 不补尾随换行：基线的输出就是没有的，加了 Rust 就得跟着加一个
    // 它本不该有的换行。
    File('$stem.vtt').writeAsStringSync(json2Vtt(body));
    File('$stem.srt').writeAsStringSync(json2Srt(body));
  }
}
