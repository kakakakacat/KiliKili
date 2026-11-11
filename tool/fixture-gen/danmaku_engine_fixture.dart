import 'dart:convert';
import 'dart:io';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/models/user/danmaku_rule.dart';
import 'package:kilikili/pages/danmaku/engine.dart';
import 'package:kilikili/services/rust/danmaku_normalize.dart';
import 'package:fixnum/fixnum.dart';

/// 弹幕**引擎** golden fixture 的生成器。
///
/// 与解码的 fixture（`danmaku_seg_fixture.dart`）是两条链：那条钉「字节
/// 里有什么」，这条钉「拿到弹幕之后怎么处理」。
///
/// ```text
/// <name>.case.json     人写的样本：引擎参数 + 一个分段的弹幕
///   ↓ 基线的 protobuf 编码器
/// <name>.raw.pb        原始字节，Rust 解码器的输入
///   ↓ 基线解码 + 基线 bucketDanmaku + danmaku_normalize.dart
/// <name>.buckets.json  期望值，Rust 引擎的输出须与之逐字段相同
/// ```
///
/// `options` 不在这里翻译成某种中间表示，而是由两边各自解读：Dart 构造
/// `RuleFilter`，Rust 构造 `FilterRules`。规则的解读本身就是要对齐的
/// 东西，翻译一遍等于把它从对照范围里摘出去。
const fixtureDir = 'rust/crates/bili-danmaku/tests/fixtures/engine';

void main() {
  final cases =
      Directory(fixtureDir)
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.case.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  if (cases.isEmpty) {
    stderr.writeln('没有弹幕引擎 fixture');
    exit(1);
  }

  for (final source in cases) {
    final spec = jsonDecode(source.readAsStringSync()) as Map<String, dynamic>;
    final segment = spec['segment'] as Map<String, dynamic>;
    final reply = _replyFromSpec(segment);

    final stem = source.path.replaceFirst('.case.json', '');
    File('$stem.raw.pb').writeAsBytesSync(reply.writeToBuffer());

    // 从**重新解码**的结果出发，理由同解码 fixture：golden 必须是
    // 「字节里有什么」，而不是内存里构造了什么。
    final decoded = DmSegMobileReply.fromBuffer(
      File('$stem.raw.pb').readAsBytesSync(),
    );

    final options = spec['options'] as Map<String, dynamic>;
    final buckets = <int, List<DanmakuElem>>{};
    bucketDanmaku(
      decoded.elems,
      into: buckets,
      isLogin: options['is_login'] as bool? ?? false,
      midHash: options['self_mid_hash'] as String? ?? '',
      mergeDanmaku: options['merge'] as bool? ?? false,
      filters: _filtersFromSpec(options['rules'] as Map<String, dynamic>?),
    );

    File('$stem.buckets.json').writeAsStringSync(
      '${const JsonEncoder.withIndent('  ').convert(normalizeBuckets(buckets))}\n',
    );
  }
}

/// 按 `.case.json` 的 `rules` 构造基线的 `RuleFilter`。
///
/// 正则的构造方式与基线从 Hive 读回规则时完全一致
/// （`RuleFilterAdapter.read`：`RegExp(i, caseSensitive: false)`）——
/// fixture 要对照的是用户真正会遇到的那条路径。
RuleFilter _filtersFromSpec(Map<String, dynamic>? rules) {
  List<String> strings(String key) =>
      (rules?[key] as List? ?? const []).cast<String>();
  return RuleFilter(
    strings('keywords'),
    strings('regexes').map((i) => RegExp(i, caseSensitive: false)).toList(),
    strings('uids').toSet(),
  );
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

/// 源描述里**没有**的键一律不设置，由 proto3 取零值。
///
/// 刻意不接受 `count` / `is_self`：它们是引擎的产物，写进输入等于让
/// fixture 自问自答（而且服务端根本不会下发，见 `danmaku_normalize.dart`）。
DanmakuElem _elemFromSpec(Map<String, dynamic> spec) {
  for (final forbidden in const ['count', 'is_self']) {
    if (spec.containsKey(forbidden)) {
      stderr.writeln('输入里不得出现引擎产物: $forbidden');
      exit(1);
    }
  }
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
