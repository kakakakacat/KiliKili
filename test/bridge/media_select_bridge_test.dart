import 'dart:convert';
import 'dart:io';

import 'package:kilikili/models/common/video/video_decode_type.dart';
import 'package:kilikili/rust/api/play_url.dart';
import 'package:kilikili/utils/video_utils.dart';
import 'package:flutter_test/flutter_test.dart';

import 'bridge_env.dart';

/// 清晰度 / 编码 / 音质选择跨 FFI 的集成测试。
///
/// ## 判据换过一次
///
/// 迁移期 Rust 这条路径藏在 `rust.select.media` 开关后面，`VideoUtils`
/// 里还留着一份 Dart 原实现，所以当时的判据是「同一份输入，开关关与开
/// 各跑一遍，结果必须一致」——不看期望值，只盯两条路径分叉。
///
/// 取流迁成 typed 直连时（issue #79）Dart 原实现与开关一并删除，没有
/// 第二条路可比了。判据改成：**每条用例的选择结果必须等于
/// `cases.json` 里写明的期望值**，而且是经由真正的动态库算出来的。
///
/// 这与 Rust 侧的 `media_select_golden.rs` 不重复：那边验的是算法本身，
/// 这里验的是**桥接**——入参的下标、`Int64List` 的映射、编码枚举的序号
/// 对齐，任何一处错位都会在这里变红，而在 Rust 进程内测不出来。
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

  List<MediaStream> videosOf(Map<String, dynamic> case_) => [
    for (final v in case_['videos'] as List)
      MediaStream(id: v['id'] as int, codecs: v['codecs'] as String?),
  ];

  List<PlayFormat> formatsOf(Map<String, dynamic> case_) => [
    for (final f in case_['support_formats'] as List)
      PlayFormat(
        quality: f['quality'] as int,
        codecs: (f['codecs'] as List).cast<String>(),
      ),
  ];

  test('每条用例经桥接选出的清晰度与编码等于期望值', () async {
    expect(cases['video'], isNotEmpty);

    for (final case_ in (cases['video'] as List).cast<Map<String, dynamic>>()) {
      final videoList = videosOf(case_);
      final supportFormats = formatsOf(case_);
      final expected = case_['expected'] as Map<String, dynamic>;

      final (quality, format, decode, video) = VideoUtils.findTargetVideo(
        videoList,
        case_['prefer_quality'] as int,
        (case_['accept_quality'] as List?)?.cast<int>(),
        supportFormats,
        [
          for (final c in case_['prefer_codecs'] as List)
            VideoDecodeFormatType.values.byName(c as String),
        ],
      );

      final reason = case_['name'] as String;
      expect(quality, expected['quality'], reason: '$reason: 清晰度');
      expect(
        supportFormats.indexOf(format),
        expected['format_index'],
        reason: '$reason: support_formats 下标',
      );
      expect(
        videoList.indexOf(video),
        expected['video_index'],
        reason: '$reason: 视频流下标',
      );
      expect(decode.name, expected['codec'], reason: '$reason: 编码');
    }
  }, skip: bridgeSkip);

  test('每条用例经桥接选出的音质等于期望值', () async {
    expect(cases['audio'], isNotEmpty);

    for (final case_ in (cases['audio'] as List).cast<Map<String, dynamic>>()) {
      final audioList = [
        for (final id in case_['audios'] as List) MediaStream(id: id as int),
      ];
      final expected = case_['expected'] as Map<String, dynamic>;
      final target = VideoUtils.findTargetAudio(
        audioList,
        case_['prefer_quality'] as int,
      );

      final reason = case_['name'] as String;
      expect(
        audioList.indexOf(target),
        expected['audio_index'],
        reason: '$reason: 音频流下标',
      );
      expect(target.id, expected['id'], reason: '$reason: 音质码');
    }
  }, skip: bridgeSkip);

  test('选不出来时抛错，而不是悄悄给一个差不多的', () async {
    // 基线在这几种输入上本来就抛（编码认不出、codecs 缺席）。没有第二
    // 条路可降级之后，这里必须继续抛——悄悄兜底的表现是「今天画质怎么
    // 不一样」，没有任何报错，正是最难发现的那种。
    expect(
      () => VideoUtils.findTargetVideo(
        [const MediaStream(id: 80)], // codecs 缺席
        80,
        null,
        [const PlayFormat(quality: 80)],
        [VideoDecodeFormatType.AV1],
      ),
      throwsA(isA<StateError>()),
    );
  }, skip: bridgeSkip);
}
