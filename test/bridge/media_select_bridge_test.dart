import 'dart:convert';
import 'dart:io';

import 'package:kilikili/models/common/video/video_decode_type.dart';
import 'package:kilikili/models/video/play/url.dart';
import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:kilikili/services/rust/rust_media_select.dart';
import 'package:kilikili/utils/video_utils.dart';
import 'package:flutter_test/flutter_test.dart';

import 'bridge_env.dart';

/// 清晰度 / 编码 / 音质选择跨 FFI 的集成测试。
///
/// 这条路径的对照用例（`cases.json`）期望值是**人写的**——`VideoUtils`
/// 依赖 `Pref`，纯 dart 的 fixture 生成器跑不起来，没法让基线自己产出
/// golden。`test/utils/media_select_golden_test.dart` 用真正的 `VideoUtils`
/// 验那份期望值，但它跑的是**开关关闭**的 Dart 原路径。
///
/// 所以这里换一个判据，完全不看期望值：**同一份输入，开关关与开各跑
/// 一遍，结果必须一致**。期望值即使写错了，这个测试照样能发现两条路径
/// 分叉——而分叉的表现是「今天画质怎么不一样」，不会报错。
void main() {
  const fixture =
      'rust/crates/bili-core/tests/fixtures/media_select/cases.json';

  late Map<String, dynamic> cases;

  setUpAll(() async {
    if (!bridgeTestsRequested) return;
    await initRustBridge();
    cases = jsonDecode(File(fixture).readAsStringSync());
  });

  tearDownAll(() async {
    if (!bridgeTestsRequested) return;
    await closeRustBridge();
  });

  /// 跑一遍选择，把结果（或基线抛出的异常）压成可比对的值。
  ///
  /// 异常也要比：ADR-010 决定 Rust 不替基线「修」掉它会抛的那几种输入，
  /// 一边抛一边不抛同样是分叉。
  Object videoSelection(Map<String, dynamic> case_) {
    final playUrl = PlayUrlModel.fromJson({
      'dash': {
        'duration': 0,
        'video': [
          for (final v in case_['videos'] as List)
            {'id': v['id'], 'codecs': v['codecs']},
        ],
      },
      'accept_quality': case_['accept_quality'],
      'support_formats': [
        for (final f in case_['support_formats'] as List)
          {'quality': f['quality'], 'codecs': f['codecs']},
      ],
    });
    final videoList = playUrl.dash!.video!;
    final supportFormats = playUrl.supportFormats!;
    try {
      final (quality, format, decode, video) = VideoUtils.findTargetVideo(
        videoList,
        case_['prefer_quality'] as int,
        playUrl.acceptQuality,
        supportFormats,
        [
          for (final c in case_['prefer_codecs'] as List)
            VideoDecodeFormatType.values.byName(c as String),
        ],
      );
      return [
        quality,
        supportFormats.indexOf(format),
        videoList.indexOf(video),
        decode.name,
      ].toString();
    } catch (e) {
      return 'threw ${e.runtimeType}';
    }
  }

  Object audioSelection(Map<String, dynamic> case_) {
    final playUrl = PlayUrlModel.fromJson({
      'dash': {
        'duration': 0,
        'audio': [
          for (final id in case_['audios'] as List) {'id': id},
        ],
      },
    });
    final audioList = playUrl.dash!.audio!;
    try {
      final target = VideoUtils.findTargetAudio(
        audioList,
        case_['prefer_quality'] as int,
      );
      return [audioList.indexOf(target), target.id].toString();
    } catch (e) {
      return 'threw ${e.runtimeType}';
    }
  }

  test('开关开与关，每条用例选出的流完全相同', () async {
    expect(cases['video'], isNotEmpty);
    expect(cases['audio'], isNotEmpty);

    for (final case_ in (cases['video'] as List).cast<Map<String, dynamic>>()) {
      await RustFlags.setRustMediaSelect(false);
      final baseline = videoSelection(case_);
      await RustFlags.setRustMediaSelect(true);
      expect(RustFlags.rustMediaSelect, isTrue, reason: '开关没打开');
      expect(
        videoSelection(case_),
        baseline,
        reason: '${case_['name']}: 开关打开后选出的清晰度/编码变了',
      );
    }

    for (final case_ in (cases['audio'] as List).cast<Map<String, dynamic>>()) {
      await RustFlags.setRustMediaSelect(false);
      final baseline = audioSelection(case_);
      await RustFlags.setRustMediaSelect(true);
      expect(
        audioSelection(case_),
        baseline,
        reason: '${case_['name']}: 开关打开后选出的音质变了',
      );
    }
  }, skip: bridgeSkip);

  test('Rust 路径确实被走到了', () async {
    // 上一个测试「两边一致」有一种平凡解：Rust 从头到尾没被调用过
    // （桥接异常被吞、开关读错），于是两边都是基线。那样它永远绿，
    // 却什么也没验。这里直接问服务层要结果，拿不到就是没走到。
    await RustFlags.setRustMediaSelect(true);
    final case_ = (cases['video'] as List).cast<Map<String, dynamic>>().first;
    final playUrl = PlayUrlModel.fromJson({
      'dash': {
        'duration': 0,
        'video': [
          for (final v in case_['videos'] as List)
            {'id': v['id'], 'codecs': v['codecs']},
        ],
      },
      'accept_quality': case_['accept_quality'],
      'support_formats': [
        for (final f in case_['support_formats'] as List)
          {'quality': f['quality'], 'codecs': f['codecs']},
      ],
    });

    expect(
      RustMediaSelect.selectVideo(
        videoList: playUrl.dash!.video!,
        preferQuality: case_['prefer_quality'] as int,
        acceptQuality: playUrl.acceptQuality,
        supportFormats: playUrl.supportFormats!,
        preferCodecs: [
          for (final c in case_['prefer_codecs'] as List)
            VideoDecodeFormatType.values.byName(c as String),
        ],
      ),
      isNotNull,
      reason: '第一条用例连 Rust 都没走到，上面的「两边一致」是平凡解',
    );
  }, skip: bridgeSkip);

  test('开关关闭时不碰 Rust', () async {
    await RustFlags.setRustMediaSelect(false);
    expect(
      RustMediaSelect.selectAudio(
        audioList: PlayUrlModel.fromJson({
          'dash': {
            'duration': 0,
            'audio': [
              {'id': 30280},
            ],
          },
        }).dash!.audio!,
        preferQuality: 30280,
      ),
      isNull,
      reason: '开关关闭却走了 Rust——降级开关失效',
    );
  }, skip: bridgeSkip);
}
