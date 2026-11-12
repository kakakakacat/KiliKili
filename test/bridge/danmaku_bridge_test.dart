import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/models/user/danmaku_rule.dart';
import 'package:kilikili/rust/api/danmaku.dart' as rust_api;
import 'package:kilikili/services/rust/rust_danmaku.dart';
import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:flutter_test/flutter_test.dart';

import 'bridge_env.dart';

/// 弹幕引擎跨 FFI 的集成测试：开会话 → 并入分段 → 按桶取 → 关会话。
///
/// 判据仍是同一份 golden（`.buckets.json`，由基线的 `bucketDanmaku` 产出，
/// `test/services/rust/danmaku_engine_test.dart` 盯着它确实是基线的结果）。
/// Rust 侧的 `engine_golden.rs` 已经验过引擎算得对，这里验的是另一件事：
/// **跨过 FFI 之后还是它**——`DanmakuBucketItem` 的字段映射、
/// `PlatformInt64` 的往返、`#[frb(sync)]` 的取桶，以及会话的生命周期。
void main() {
  const fixtureDir = 'rust/crates/bili-danmaku/tests/fixtures/engine';

  /// 跨 FFI 带回来的字段，见 `DanmakuBucketItem` 的说明：只有渲染与交互
  /// 真正要用的那些。golden 里的其余字段（action、extra、id_str、oid 等）
  /// 没有消费方，不拷过来，因此比对只取这个投影。
  const carried = [
    'id',
    'progress',
    'mode',
    'fontsize',
    'color',
    'mid_hash',
    'content',
    'weight',
    'like_count',
    'colorful',
    'count',
    'is_self',
  ];

  Map<String, dynamic> project(DanmakuElem e) => {
    'id': e.id.toInt(),
    'progress': e.progress,
    'mode': e.mode,
    'fontsize': e.fontsize,
    'color': e.color,
    'mid_hash': e.midHash,
    'content': e.content,
    'weight': e.weight,
    'like_count': e.likeCount.toInt(),
    'colorful': e.colorful.value,
    'count': e.count,
    'is_self': e.isSelf,
  };

  late List<File> cases;

  setUpAll(() async {
    if (!bridgeTestsRequested) return;
    await initRustBridge();
    await RustFlags.setRustDanmakuEngine(true);
    expect(
      RustFlags.rustDanmakuEngine,
      isTrue,
      reason: '开关没打开，下面的测试会在验 Dart 原路径',
    );
    cases =
        Directory(fixtureDir)
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.case.json'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
  });

  tearDownAll(() async {
    if (!bridgeTestsRequested) return;
    await closeRustBridge();
  });

  test('分桶结果跨 FFI 回到 Dart 后与基线 golden 一致', () async {
    expect(cases, isNotEmpty, reason: 'fixture 没找到，本测试形同虚设');

    for (final source in cases) {
      final spec = jsonDecode(source.readAsStringSync()) as Map<String, dynamic>;
      final options = spec['options'] as Map<String, dynamic>;
      final stem = source.path.replaceFirst('.case.json', '');
      final golden =
          jsonDecode(File('$stem.buckets.json').readAsStringSync())
              as Map<String, dynamic>;

      final session = RustDanmakuSession.open(
        isLogin: options['is_login'] as bool? ?? false,
        midHash: options['self_mid_hash'] as String? ?? '',
        mergeDanmaku: options['merge'] as bool? ?? false,
        filters: _filtersFromSpec(options['rules'] as Map<String, dynamic>?),
      );
      expect(session, isNotNull, reason: '${source.path}: 会话没开起来');

      final push = await session!.pushSegment(
        0,
        File('$stem.raw.pb').readAsBytesSync(),
      );
      expect(push, isNotNull, reason: '${source.path}: 分段并入失败');

      for (final entry in golden.entries) {
        final progressMs = int.parse(entry.key) * 100;
        final actual = session.bucket(progressMs);
        expect(
          actual,
          isNotNull,
          reason: '${source.path}: 桶 ${entry.key} 在 Rust 侧是空的',
        );
        expect(
          actual!.map(project).toList(),
          (entry.value as List)
              .cast<Map<String, dynamic>>()
              .map((e) => {for (final k in carried) k: e[k]})
              .toList(),
          reason: '${source.path}: 桶 ${entry.key} 与基线不一致',
        );
      }

      // 过滤后的总条数：桶内都对得上而总数少，说明有整桶没被带回来。
      expect(
        push!.total,
        golden.values.fold<int>(0, (n, v) => n + (v as List).length),
        reason: '${source.path}: 入桶总条数与基线不一致',
      );

      // golden 之外不该凭空多出桶来。取最大键之后的一桶。
      final maxKey = golden.keys.map(int.parse).fold(0, (a, b) => a > b ? a : b);
      expect(
        session.bucket((maxKey + 1) * 100),
        isNull,
        reason: '${source.path}: golden 之外多出了弹幕',
      );

      session.close();
    }

    expect(
      rust_api.danmakuOpenSessionCount(),
      0,
      reason: '会话没关干净——泄漏一个会话就是泄漏一个视频的全部弹幕',
    );
  }, skip: bridgeSkip);

  test('重复并入同一个分段不会翻倍', () async {
    // 基线按 segmentIndex 记已请求的分段，Rust 侧也必须幂等：同一段并两次
    // 而条数翻倍，表现是「弹幕突然变密」，没有任何报错。
    final bytes = File('$fixtureDir/typical.raw.pb').readAsBytesSync();
    final session = RustDanmakuSession.open(
      isLogin: false,
      midHash: '',
      mergeDanmaku: false,
      filters: RuleFilter.empty(),
    );
    expect(session, isNotNull);

    final first = await session!.pushSegment(0, bytes);
    final second = await session.pushSegment(0, bytes);
    expect(first, isNotNull);
    expect(second, isNotNull);
    expect(second!.alreadyLoaded, isTrue);
    expect(second.total, first!.total);

    session.close();
  }, skip: bridgeSkip);

  test('关掉的会话不再返回弹幕，重复关闭无副作用', () async {
    final session = RustDanmakuSession.open(
      isLogin: false,
      midHash: '',
      mergeDanmaku: false,
      filters: RuleFilter.empty(),
    );
    expect(session, isNotNull);
    await session!.pushSegment(
      0,
      File('$fixtureDir/typical.raw.pb').readAsBytesSync(),
    );
    expect(session.bucket(0), isNotNull);

    session
      ..close()
      ..close();

    // 关闭后调用方可能还在跑最后一帧：必须返回空而不是抛异常。
    expect(session.bucket(0), isNull);
    expect(await session.pushSegment(1, Uint8List(0)), isNull);
  }, skip: bridgeSkip);

  test('Rust 语义不等价的正则让整条路径降级，而不是少过滤一条', () {
    // ADR-010 的核心取舍。开不出会话就是降级信号，`PlDanmakuController`
    // 据此走基线。
    final divergent = RustDanmakuSession.open(
      isLogin: false,
      midHash: '',
      mergeDanmaku: false,
      // 两端都编得过，但 Dart 的 \d 只认 ASCII、Rust 认 Unicode 全集，
      // 全角数字只在 Rust 这边命中。静默的不等价，必须拒。
      filters: RuleFilter([], [RegExp(r'\d+')], {}),
    );
    expect(divergent, isNull, reason: r'带 \d 的规则本应让会话开不起来');

    // 写成显式字符类就两端一致，不该被误拒——多拒只是回到基线，但拒得
    // 太宽等于这条路径永远用不上。
    final equivalent = RustDanmakuSession.open(
      isLogin: false,
      midHash: '',
      mergeDanmaku: false,
      filters: RuleFilter([], [RegExp('[0-9]+')], {}),
    );
    expect(equivalent, isNotNull, reason: '[0-9] 两端等价，不该被拒');
    equivalent!.close();

    expect(rust_api.danmakuOpenSessionCount(), 0);
  }, skip: bridgeSkip);

  test('开关关闭时不碰 Rust', () async {
    await RustFlags.setRustDanmakuEngine(false);
    addTearDown(() => RustFlags.setRustDanmakuEngine(true));
    expect(
      RustDanmakuSession.open(
        isLogin: false,
        midHash: '',
        mergeDanmaku: false,
        filters: RuleFilter.empty(),
      ),
      isNull,
      reason: '开关关闭却开出了会话——降级开关失效',
    );
  }, skip: bridgeSkip);

  // 进度游标（issue #89）。判据本身由 `bili_danmaku::cursor` 的单测钉住，
  // 这里验的是跨 FFI 之后还是它：`DanmakuProgressCursor` 把状态原样带回
  // 下一帧，而不是每次都从零开始。游标不依赖会话，所以不必开会话。
  test('进度游标：正常推进逐桶放行，同一桶只放一次', () async {
    final cursor = DanmakuProgressCursor();
    expect(cursor.step(0, 0)?.bucketMs, 0);
    expect(cursor.step(40, 0), isNull, reason: '同一个 100ms 桶不得放两次');
    expect(cursor.step(120, 0)?.bucketMs, 100);
  }, skip: bridgeSkip);

  test('进度游标：没有 seek 的回跳一桶都不放，追上后恢复', () async {
    final cursor = DanmakuProgressCursor();
    expect(cursor.step(600000, 0)?.bucketMs, 600000);
    // 退到后台再回前台，position 流先吐一个 0 再追上来。
    for (final rewound in [0, 100, 300000, 600000]) {
      expect(
        cursor.step(rewound, 0),
        isNull,
        reason: '回跳到 $rewound 时放了桶，视频开头的弹幕会飘出来',
      );
    }
    expect(cursor.step(600100, 0)?.bucketMs, 600100);
  }, skip: bridgeSkip);

  test('进度游标：真正的 seek 清屏并从新位置重开', () async {
    final cursor = DanmakuProgressCursor();
    expect(cursor.step(600000, 0)?.bucketMs, 600000);
    final seeked = cursor.step(10000, 1);
    expect(seeked?.bucketMs, 10000, reason: '向后 seek 必须从新位置继续');
    expect(seeked?.clear, isTrue, reason: 'seek 后屏上的旧弹幕要清掉');
    expect(cursor.step(10100, 1)?.bucketMs, 10100);
  }, skip: bridgeSkip);
}

/// 与 `test/services/rust/danmaku_engine_test.dart` 同一份转换：fixture 的
/// 规则描述 → 基线的 `RuleFilter`。两边都从 spec 构造，不共用中间表示——
/// 规则的解读本身就是要对齐的东西（ADR-010）。
RuleFilter _filtersFromSpec(Map<String, dynamic>? rules) {
  List<String> strings(String key) =>
      (rules?[key] as List? ?? const []).cast<String>();
  return RuleFilter(
    strings('keywords'),
    strings('regexes').map((i) => RegExp(i, caseSensitive: false)).toList(),
    strings('uids').toSet(),
  );
}
