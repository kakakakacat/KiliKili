import 'dart:convert';
import 'dart:io';

import 'package:kilikili/models/common/video/video_decode_type.dart';
import 'package:kilikili/models/video/play/url.dart';
import 'package:kilikili/utils/video_utils.dart';
import 'package:flutter_test/flutter_test.dart';

/// 清晰度 / 编码 / 音质选择的对照用例，与 Rust 侧共用一份输入和期望值
/// （`rust/crates/bili-core/tests/fixtures/media_select/cases.json`，Rust
/// 那侧是 `tests/media_select_golden.rs`）。
///
/// **本测试是判据**。与其他 fixture 链不同，那份期望值是人写的——
/// `VideoUtils` 依赖 `Pref`（hive + flutter），纯 dart 的 fixture 生成器
/// 跑不起来，没法让基线自己产出 golden。于是改成：期望值写进文件，由
/// 这里用**真正的** `VideoUtils` 验证。期望值错了，这个测试会红，而不是
/// Rust 与一份来历不明的文件自洽。
void main() {
  const fixture =
      'rust/crates/bili-core/tests/fixtures/media_select/cases.json';

  late Map<String, dynamic> cases;

  setUpAll(() {
    cases = jsonDecode(File(fixture).readAsStringSync());
  });

  test('用例非空，否则本测试形同虚设', () {
    expect(cases['video'], isNotEmpty);
    expect(cases['audio'], isNotEmpty);
  });

  test('清晰度与编码选择与基线一致', () {
    for (final case_ in (cases['video'] as List).cast<Map<String, dynamic>>()) {
      final name = case_['name'];
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
      final (
        int targetQuality,
        FormatItem targetFormat,
        VideoDecodeFormatType targetDecode,
        VideoItem targetVideo,
      ) = VideoUtils.findTargetVideo(
        videoList,
        case_['prefer_quality'] as int,
        playUrl.acceptQuality,
        supportFormats,
        [
          for (final c in case_['prefer_codecs'] as List)
            VideoDecodeFormatType.values.byName(c as String),
        ],
      );

      final expected = case_['expected'] as Map<String, dynamic>;
      expect(targetQuality, expected['quality'], reason: '$name: 清晰度');
      expect(
        supportFormats.indexOf(targetFormat),
        expected['format_index'],
        reason: '$name: support_formats 下标',
      );
      expect(
        videoList.indexOf(targetVideo),
        expected['video_index'],
        reason: '$name: 视频流下标',
      );
      expect(targetDecode.name, expected['codec'], reason: '$name: 编码');
    }
  });

  test('音质选择与基线一致', () {
    for (final case_ in (cases['audio'] as List).cast<Map<String, dynamic>>()) {
      final name = case_['name'];
      final playUrl = PlayUrlModel.fromJson({
        'dash': {
          'duration': 0,
          'audio': [
            for (final id in case_['audios'] as List) {'id': id},
          ],
        },
      });

      final audioList = playUrl.dash!.audio!;
      final target = VideoUtils.findTargetAudio(
        audioList,
        case_['prefer_quality'] as int,
      );

      final expected = case_['expected'] as Map<String, dynamic>;
      expect(
        audioList.indexOf(target),
        expected['audio_index'],
        reason: '$name: 音频流下标',
      );
      expect(target.id, expected['id'], reason: '$name: 音质 code');
    }
  });
}
