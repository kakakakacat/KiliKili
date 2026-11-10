import 'dart:convert';
import 'dart:io';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/services/rust/danmaku_normalize.dart';
import 'package:fixnum/fixnum.dart';

/// 弹幕分段 golden fixture 的生成器。
///
/// 与其他 fixture 的不同之处：弹幕的原始数据是 **protobuf 字节**，不是
/// 可读的 JSON。因此多一层「源描述」：
///
/// ```text
/// <name>.source.json   人写的样本描述（本文件的输入）
///   ↓ 基线的 protobuf 编码器
/// <name>.raw.pb        原始字节，Rust 解码器的输入
///   ↓ 基线的解码 + danmaku_normalize.dart
/// <name>.normalized.json  期望值，Rust 解码结果须与之逐字段相同
/// ```
///
/// 让基线来编码而不是手搓字节，是因为这条链上真正要钉死的是「**基线
/// 写出什么、Rust 就得读出什么**」。手搓的字节只能证明 Rust 读得懂我们
/// 自己的想象。
const fixtureDir = 'rust/crates/bili-api/tests/fixtures/danmaku_seg';

void main() {
  final sources =
      Directory(fixtureDir)
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.source.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  if (sources.isEmpty) {
    stderr.writeln('没有弹幕分段 fixture');
    exit(1);
  }

  for (final source in sources) {
    final spec = jsonDecode(source.readAsStringSync()) as Map<String, dynamic>;
    final reply = _replyFromSpec(spec);

    final stem = source.path.replaceFirst('.source.json', '');
    File('$stem.raw.pb').writeAsBytesSync(reply.writeToBuffer());

    // 从**重新解码**的结果出发，而不是直接规范化上面构造的对象：后者
    // 会让编码器的任何缺陷自我掩盖（没写进字节的字段，在内存对象里仍
    // 然在）。golden 必须是「字节里有什么」。
    final decoded = DmSegMobileReply.fromBuffer(
      File('$stem.raw.pb').readAsBytesSync(),
    );
    File('$stem.normalized.json').writeAsStringSync(
      '${const JsonEncoder.withIndent('  ').convert(normalizeDanmakuSegment(decoded))}\n',
    );
  }
}

DmSegMobileReply _replyFromSpec(Map<String, dynamic> spec) {
  final reply = DmSegMobileReply();
  if (spec['state'] case final int state) {
    reply.state = state;
  }
  for (final elem in (spec['elems'] as List? ?? const [])) {
    reply.elems.add(_elemFromSpec(elem as Map<String, dynamic>));
  }
  return reply;
}

/// 源描述里**没有**的键一律不设置，由 proto3 取零值——fixture 要覆盖的
/// 正是「字段缺席」这种形态。
DanmakuElem _elemFromSpec(Map<String, dynamic> spec) {
  final elem = DanmakuElem();
  if (spec['id'] case final int v) elem.id = Int64(v);
  if (spec['progress'] case final int v) elem.progress = v;
  if (spec['mode'] case final int v) elem.mode = v;
  if (spec['fontsize'] case final int v) elem.fontsize = v;
  if (spec['color'] case final int v) elem.color = v;
  if (spec['mid_hash'] case final String v) elem.midHash = v;
  if (spec['content'] case final String v) elem.content = v;
  if (spec['ctime'] case final int v) elem.ctime = Int64(v);
  if (spec['weight'] case final int v) elem.weight = v;
  if (spec['action'] case final String v) elem.action = v;
  if (spec['pool'] case final int v) elem.pool = v;
  if (spec['id_str'] case final String v) elem.idStr = v;
  if (spec['attr'] case final int v) elem.attr = v;
  if (spec['like_count'] case final int v) elem.likeCount = Int64(v);
  if (spec['animation'] case final String v) elem.animation = v;
  if (spec['extra'] case final String v) elem.extra = v;
  if (spec['colorful'] case final int v) {
    // valueOf 返回 null 表示生成代码不认识这个取值。源描述里出现未知值
    // 多半是写错了，而不是要测试未知枚举——后者属于 Rust 侧的单测，
    // 因为基线的编码器根本写不出它认不得的枚举。
    final value = DmColorfulType.valueOf(v);
    if (value == null) {
      stderr.writeln('未知的 colorful 取值: $v');
      exit(1);
    }
    elem.colorful = value;
  }
  if (spec['dm_type'] case final int v) elem.type = v;
  if (spec['oid'] case final int v) elem.oid = Int64(v);
  if (spec['dm_from'] case final int v) {
    final value = DmFromType.valueOf(v);
    if (value == null) {
      stderr.writeln('未知的 dm_from 取值: $v');
      exit(1);
    }
    elem.dmFrom = value;
  }
  return elem;
}
