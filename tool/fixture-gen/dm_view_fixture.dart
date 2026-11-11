import 'dart:convert';
import 'dart:io';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/services/rust/dm_view_normalize.dart';
import 'package:fixnum/fixnum.dart';

/// `DmView` 字幕 golden fixture 的生成器。
///
/// 链条与弹幕分段的那条相同（`danmaku_seg_fixture.dart`）：
///
/// ```text
/// <name>.source.json      人写的样本描述
///   ↓ 基线的 protobuf 编码器
/// <name>.raw.pb           原始字节，Rust 解码器的输入
///   ↓ 基线解码 + dm_view_normalize.dart（基线映射的转写）
/// <name>.normalized.json  期望值
/// ```
///
/// 源描述里**刻意**放了几个没有消费方的字段（`kv`、`special_dms`、
/// `send_box_style`……）：它们会进字节，Rust 必须跳过而不是报错。没有
/// 它们，fixture 就只证明了「我们解得了自己写的那几个字段」。
const fixtureDir = 'rust/crates/bili-api/tests/fixtures/dm_view';

void main() {
  final sources =
      Directory(fixtureDir)
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.source.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  if (sources.isEmpty) {
    stderr.writeln('没有 DmView fixture');
    exit(1);
  }

  for (final source in sources) {
    final spec = jsonDecode(source.readAsStringSync()) as Map<String, dynamic>;
    final reply = _replyFromSpec(spec);

    final stem = source.path.replaceFirst('.source.json', '');
    File('$stem.raw.pb').writeAsBytesSync(reply.writeToBuffer());

    // 从重新解码的结果出发：golden 必须是「字节里有什么」。
    final decoded = DmViewReply.fromBuffer(
      File('$stem.raw.pb').readAsBytesSync(),
    );
    File('$stem.normalized.json').writeAsStringSync(
      '${const JsonEncoder.withIndent('  ').convert(normalizeDmViewSubtitles(decoded))}\n',
    );
  }
}

DmViewReply _replyFromSpec(Map<String, dynamic> spec) {
  final reply = DmViewReply();
  // 没有消费方、但真实响应里一定有的字段。写进 fixture 是为了让 Rust
  // 的「跳过未消费子树」被真的测到。
  if (spec['closed'] case final bool v) reply.closed = v;
  if (spec['send_box_style'] case final int v) reply.sendBoxStyle = v;
  if (spec['check_box_show_msg'] case final String v) reply.checkBoxShowMsg = v;
  if (spec['text_placeholder'] case final String v) reply.textPlaceholder = v;
  if (spec['kv'] case final String v) reply.kv = v;
  if (spec['special_dms'] case final List v) {
    reply.specialDms.addAll(v.cast<String>());
  }

  if (spec['subtitle'] case final Map<String, dynamic> sub) {
    reply.subtitle = _subtitleFromSpec(sub);
  }
  return reply;
}

VideoSubtitle _subtitleFromSpec(Map<String, dynamic> spec) {
  final subtitle = VideoSubtitle();
  if (spec['lan'] case final String v) subtitle.lan = v;
  if (spec['lan_doc'] case final String v) subtitle.lanDoc = v;
  for (final item in (spec['subtitles'] as List? ?? const [])) {
    subtitle.subtitles.add(_itemFromSpec(item as Map<String, dynamic>));
  }
  return subtitle;
}

/// 源描述里**没有**的键一律不设置，由 proto3 取零值。
SubtitleItem _itemFromSpec(Map<String, dynamic> spec) {
  final item = SubtitleItem();
  if (spec['id'] case final int v) item.id = Int64(v);
  if (spec['id_str'] case final String v) item.idStr = v;
  if (spec['lan'] case final String v) item.lan = v;
  if (spec['lan_doc'] case final String v) item.lanDoc = v;
  if (spec['subtitle_url'] case final String v) item.subtitleUrl = v;
  if (spec['lan_doc_brief'] case final String v) item.lanDocBrief = v;
  if (spec['type'] case final int v) {
    final value = SubtitleType.valueOf(v);
    if (value == null) {
      stderr.writeln('未知的 type 取值: $v');
      exit(1);
    }
    item.type = value;
  }
  return item;
}
