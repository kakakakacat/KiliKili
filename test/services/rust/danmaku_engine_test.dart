import 'dart:convert';
import 'dart:io';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/models/user/danmaku_rule.dart';
import 'package:kilikili/pages/danmaku/engine.dart';
import 'package:kilikili/services/rust/danmaku_normalize.dart';
import 'package:flutter_test/flutter_test.dart';

/// 弹幕**引擎** fixture 的 golden 是给 Rust 引擎当裁判用的
/// （`rust/crates/bili-danmaku/tests/engine_golden.rs`）。裁判本身得有人
/// 看着：本测试用基线自己的 `bucketDanmaku` 重新跑一遍同一份输入，核对
/// golden 确实是基线会得出的结果。
///
/// 没有这个测试，`.buckets.json` 就只是一份来历不明的期望值——Rust 全绿
/// 只能证明它与那份文件一致，证明不了它与基线一致。
void main() {
  const fixtureDir = 'rust/crates/bili-danmaku/tests/fixtures/engine';

  late List<File> cases;

  setUpAll(() {
    cases =
        Directory(fixtureDir)
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.case.json'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
  });

  test('fixture 目录非空，否则本测试形同虚设', () {
    expect(cases, isNotEmpty);
  });

  test('golden 与基线引擎的输出一致', () {
    for (final source in cases) {
      final spec = jsonDecode(source.readAsStringSync()) as Map<String, dynamic>;
      final stem = source.path.replaceFirst('.case.json', '');
      final reply = DmSegMobileReply.fromBuffer(
        File('$stem.raw.pb').readAsBytesSync(),
      );

      final options = spec['options'] as Map<String, dynamic>;
      final buckets = <int, List<DanmakuElem>>{};
      bucketDanmaku(
        reply.elems,
        into: buckets,
        isLogin: options['is_login'] as bool? ?? false,
        midHash: options['self_mid_hash'] as String? ?? '',
        mergeDanmaku: options['merge'] as bool? ?? false,
        filters: _filtersFromSpec(options['rules'] as Map<String, dynamic>?),
      );

      expect(
        normalizeBuckets(buckets),
        jsonDecode(File('$stem.buckets.json').readAsStringSync()),
        reason: '${source.path}: golden 不是基线跑出来的结果',
      );
    }
  });

  test('raw.pb 是基线编码器写出的规范形式', () {
    // 理由同解码 fixture：不成立说明 Rust 即使全绿，也只是对着一份线上
    // 不会出现的输入全绿。
    for (final source in cases) {
      final raw = File(source.path.replaceFirst('.case.json', '.raw.pb'));
      final bytes = raw.readAsBytesSync();
      expect(
        DmSegMobileReply.fromBuffer(bytes).writeToBuffer(),
        bytes,
        reason: '${raw.path}: 不是规范编码，请用 fixture 生成器重新生成',
      );
    }
  });

  test('输入里不含引擎自己的产物', () {
    // count / isSelf 由引擎写入。出现在输入里，fixture 就成了自问自答。
    for (final source in cases) {
      final spec = jsonDecode(source.readAsStringSync()) as Map<String, dynamic>;
      final elems =
          (spec['segment'] as Map<String, dynamic>)['elems'] as List? ??
          const [];
      for (final elem in elems.cast<Map<String, dynamic>>()) {
        expect(elem.containsKey('count'), isFalse, reason: source.path);
        expect(elem.containsKey('is_self'), isFalse, reason: source.path);
      }
    }
  });

  group('基线行为的回归锚点', () {
    // 这些断言不依赖 fixture：它们说明 bucketDanmaku 从控制器里搬出来时
    // 行为没有变。Rust 侧有一一对应的单测（bili-danmaku::engine）。
    DanmakuElem elem(int progress, String content, String midHash) =>
        DanmakuElem()
          ..progress = progress
          ..content = content
          ..midHash = midHash;

    test('合并保留首条并累加 count', () {
      final buckets = <int, List<DanmakuElem>>{};
      bucketDanmaku(
        [elem(1000, '草', 'a'), elem(2000, '草', 'b'), elem(3000, '草', 'c')],
        into: buckets,
        isLogin: false,
        midHash: '',
        mergeDanmaku: true,
        filters: RuleFilter.empty(),
      );
      expect(buckets[20], isNull);
      expect(buckets[10]!.single.count, 3);
    });

    test('被过滤的首条也会吞掉同内容的后续', () {
      final buckets = <int, List<DanmakuElem>>{};
      bucketDanmaku(
        [
          elem(1000, '广告内容', 'a'),
          elem(2000, '广告内容', 'b'),
          elem(2000, '正常弹幕', 'c'),
        ],
        into: buckets,
        isLogin: false,
        midHash: '',
        mergeDanmaku: true,
        filters: RuleFilter(['广告'], [], {}),
      );
      expect(buckets[10], isNull);
      expect(buckets[20]!.single.content, '正常弹幕');
    });

    test('自己发的弹幕不合并也不过滤', () {
      final buckets = <int, List<DanmakuElem>>{};
      bucketDanmaku(
        [elem(1000, '广告内容', 'me'), elem(1000, '广告内容', 'me')],
        into: buckets,
        isLogin: true,
        midHash: 'me',
        mergeDanmaku: true,
        filters: RuleFilter(['广告'], [], {}),
      );
      expect(buckets[10]!.length, 2);
      expect(buckets[10]!.every((e) => e.isSelf), isTrue);
      expect(buckets[10]!.every((e) => e.count == 0), isTrue);
    });

    test('未登录时不标记 isSelf', () {
      final buckets = <int, List<DanmakuElem>>{};
      bucketDanmaku(
        [elem(1000, '广告内容', 'me')],
        into: buckets,
        isLogin: false,
        midHash: 'me',
        mergeDanmaku: true,
        filters: RuleFilter(['广告'], [], {}),
      );
      expect(buckets, isEmpty);
    });
  });
}

RuleFilter _filtersFromSpec(Map<String, dynamic>? rules) {
  List<String> strings(String key) =>
      (rules?[key] as List? ?? const []).cast<String>();
  return RuleFilter(
    strings('keywords'),
    strings('regexes').map((i) => RegExp(i, caseSensitive: false)).toList(),
    strings('uids').toSet(),
  );
}
