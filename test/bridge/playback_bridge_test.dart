import 'package:kilikili/models/common/video/cdn_type.dart';
import 'package:kilikili/rust/api/play_url.dart';
import 'package:kilikili/rust/api/playback.dart' as rust_api;
import 'package:kilikili/services/rust/play_url_ext.dart';
import 'package:kilikili/utils/video_utils.dart';
import 'package:flutter_test/flutter_test.dart';

import 'bridge_env.dart';

/// 起播决策跨 FFI 的集成测试（issue #79）。
///
/// 算法本身的判据在 Rust（`bili-core` 的 `cdn` / `playback` 单测，其中
/// CDN 那九条是从删掉的 `test/utils/cdn_url_test.dart` 原样搬过去的）。
/// 这里验的是**桥接**：字符串、列表与可空字段能不能原样跨过 FFI。
///
/// ## 为什么必须有这一层
///
/// 取流迁移前，`VideoUtils.getCdnUrl` 是纯 Dart 的，`test/utils/` 下那个
/// 测试直接调它就够了。迁移后它要过 FFI，而 `test/utils/` 跑在**没有
/// 原生库**的进程里——那个测试于是变成了九条
/// 「flutter_rust_bridge has not been initialized」。
///
/// 换句话说：把算法下沉之后，宿主侧的判据也要跟着搬家，否则留下的不是
/// 回归保护，是一组必然失败的用例。
void main() {
  setUpAll(() async {
    if (!bridgeTestsRequested) return;
    await initRustBridge();
  });

  tearDownAll(() async {
    if (!bridgeTestsRequested) return;
    await closeRustBridge();
  });

  // 与 Rust 侧那九条用的是同三个样本。akamai 这条**刻意**带着签名与
  // `%2b`：它要钉的正是「一个字节都不改」，而跨 FFI 的字符串编码恰恰是
  // 最容易在这里出问题的地方。
  const cosov =
      'https://upos-sz-mirrorcosov.bilivideo.com/upgcxcode/01/video.m4s'
      '?os=cosovbv&upsig=cosov-signature';
  const akamai =
      'https://upos-hz-mirrorakam.akamaized.net/upgcxcode/01/video.m4s'
      '?os=akam&upsig=akamai-signature&hdnts=exp%3D123~hmac%3Dabc&x=one%2btwo';
  const mcdn = 'https://example.mcdn.bilivideo.cn:4483/v1/resource/video.m4s';

  test('选中的地址跨 FFI 回来时一个字节都没变', () {
    VideoUtils.cdnService = CDNService.akamai;
    VideoUtils.disableAudioCDN = false;
    expect(VideoUtils.getCdnUrl([cosov, akamai]), akamai);
    expect(VideoUtils.getCdnUrl([akamai, cosov]), akamai);
    expect(VideoUtils.getCdnUrl([mcdn, cosov, akamai]), akamai);
  }, skip: bridgeSkip);

  test('设置项经枚举名传到 Rust，两边是同一张表', () {
    VideoUtils.cdnService = CDNService.akamai;
    VideoUtils.disableAudioCDN = false;

    // 枚举名即 Rust 侧的 key。少一项、拼错一个字母，这里就会退化成
    // 「认不出就回落 ali」，而回落是静默的。
    final keys = {for (final e in rust_api.cdnServices()) e.key};
    for (final s in CDNService.values) {
      expect(keys, contains(s.name), reason: '${s.name} 在 Rust 侧的表里没有');
    }
    expect(
      {for (final e in rust_api.cdnServices()) e.key: e.host},
      {for (final s in CDNService.values) s.name: s.host},
      reason: '两侧的 host 对不上',
    );

    expect(
      VideoUtils.getCdnUrl([
        mcdn,
        cosov,
        akamai,
      ], defaultCDNService: CDNService.baseUrl),
      mcdn,
    );
    expect(
      VideoUtils.getCdnUrl([cosov, akamai], defaultCDNService: CDNService.ali),
      'https://upos-sz-mirrorali.bilivideo.com/upgcxcode/01/video.m4s'
      '?os=cosovbv&upsig=cosov-signature',
    );
  }, skip: bridgeSkip);

  test('音频开关是布尔，不是「传了就算真」', () {
    VideoUtils.cdnService = CDNService.akamai;
    VideoUtils.disableAudioCDN = true;
    expect(VideoUtils.getCdnUrl([cosov, akamai], isAudio: true), cosov);
    expect(VideoUtils.getCdnUrl([cosov, akamai]), akamai);
  }, skip: bridgeSkip);

  test('候选地址的顺序与可空字段跨 FFI 一致', () {
    const stream = MediaStream(
      id: 80,
      baseUrl: 'https://primary.example.invalid/a.m4s',
      backupUrl: ['https://backup1.example.invalid/a.m4s'],
    );
    expect(stream.playUrls, [
      'https://primary.example.invalid/a.m4s',
      'https://backup1.example.invalid/a.m4s',
    ]);

    // 主地址缺席或为空串时只剩备用，空串不该变成一个空候选。
    const noBase = MediaStream(
      id: 80,
      baseUrl: '',
      backupUrl: ['https://backup1.example.invalid/a.m4s'],
    );
    expect(noBase.playUrls, ['https://backup1.example.invalid/a.m4s']);
    expect(const MediaStream(id: 80).playUrls, isEmpty);
  }, skip: bridgeSkip);

  test('补档的缺档计算与合并跨 FFI 一致', () {
    const play = PlayUrl(
      lastPlayTime: 0,
      supportFormats: [
        PlayFormat(quality: 120),
        PlayFormat(quality: 80),
        PlayFormat(quality: 32),
      ],
      dash: Dash(
        video: [MediaStream(id: 120, codecid: 7), MediaStream(id: 80, codecid: 7)],
      ),
    );
    // 120 是最高档、80 已有，只剩 32 可补
    expect(play.missingVideoQualityBelowHighest, 32);

    final merged = play.dash!.video!.merged(const [
      MediaStream(id: 32, codecid: 7),
      // 同 (id, codecid) 重复，应被丢弃
      MediaStream(id: 80, codecid: 7),
      // 同档不同编码，应保留
      MediaStream(id: 80, codecid: 12),
    ]);
    expect(merged.map((e) => e.id).toList(), [120, 80, 80, 32]);
    expect(merged.length, 4);
  }, skip: bridgeSkip);

  test('音量均衡的参数串跨 FFI 一致', () {
    const volume = Volume(
      measuredI: -14.5,
      measuredLra: 7,
      measuredTp: -1,
      measuredThreshold: -25,
      targetOffset: 0,
      targetI: -16,
      targetTp: -1.5,
    );
    expect(volume.isNotEmpty, isTrue);
    expect(
      const Volume(
        measuredI: 0,
        measuredLra: 0,
        measuredTp: 0,
        measuredThreshold: 0,
        targetOffset: 0,
        targetI: -16,
        targetTp: -1.5,
      ).isNotEmpty,
      isFalse,
      reason: '全为 0 应视为服务端没给',
    );

    final params = volume.format({'lra': 11, 'i': -16, 'tp': -1.5, 'offset': 0});
    expect(params, startsWith('LRA=11:I=-16:TP=-1.5:offset=0:'));
    expect(params, contains('measured_I=-14.5'));
  }, skip: bridgeSkip);

  test('PGC 片头片尾跨 FFI 转成毫秒的片段', () {
    final segments = pgcClipSegments(const [
      ClipInfo(start: 0, end: 88.5, clipType: 'CLIP_TYPE_OP'),
      ClipInfo(start: 1350, end: 1434, clipType: 'CLIP_TYPE_ED'),
    ], 1440000);
    expect(segments.length, 2);
    expect(segments[0].category, 'intro');
    expect(segments[0].endMs, 88500);
    expect(segments[1].category, 'outro');
    expect(segments[0].videoDurationMs, 1440000);
  }, skip: bridgeSkip);
}
