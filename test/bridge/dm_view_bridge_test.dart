import 'dart:io';
import 'dart:typed_data';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/grpc/dm.dart';
import 'package:kilikili/models_new/video/video_play_info/subtitle.dart';
import 'package:kilikili/services/rust/rust_dm_view.dart';
import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:flutter_test/flutter_test.dart';

import 'bridge_env.dart';

/// DmView 字幕轨跨 FFI 的集成测试。
///
/// 判据不是 golden 而是**基线本人**：同一份字节，`DmGrpc.dmViewToSubtitle`
/// 与 Rust 路径各跑一遍，逐字段比对。两条路径在生产里是可以互换的
/// （开关随时能关），所以这里要的就是「换了之后调用方拿到同一组对象」。
void main() {
  const fixtureDir = 'rust/crates/bili-api/tests/fixtures/dm_view';

  List<Object?> project(List<Subtitle> subs) => [
    for (final s in subs)
      [s.id, s.lan, s.lanDoc, s.subtitleUrl, s.type, s.isAi],
  ];

  late List<File> raws;

  setUpAll(() async {
    if (!bridgeTestsRequested) return;
    await initRustBridge();
    await RustFlags.setRustDanmakuEngine(false);
    await RustFlags.setRustDmViewSubtitle(true);
    expect(RustFlags.rustDmViewSubtitle, isTrue, reason: '开关没打开');
    raws =
        Directory(fixtureDir)
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.raw.pb'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
  });

  tearDownAll(() async {
    if (!bridgeTestsRequested) return;
    await closeRustBridge();
  });

  test('Rust 解出的字幕轨与基线逐字段相同，顺序也相同', () async {
    expect(raws, isNotEmpty, reason: 'fixture 没找到，本测试形同虚设');

    for (final raw in raws) {
      final bytes = raw.readAsBytesSync();
      final fromRust = await RustDmView.parseSubtitles(bytes);
      expect(fromRust, isNotNull, reason: '${raw.path}: Rust 路径降级了');

      // 基线在「没有 subtitle 字段」与「列表为空」时都返回 null，调用方
      // 对两者处理相同；Rust 侧统一给空列表（见 RustDmView 的说明）。
      final fromDart =
          DmGrpc.dmViewToSubtitle(DmViewReply.fromBuffer(bytes)) ??
          const <Subtitle>[];

      expect(
        project(fromRust!),
        project(fromDart),
        reason: '${raw.path}: 两条路径给出的字幕不一致',
      );
    }
  }, skip: bridgeSkip);

  test('解不动的字节降级回 Dart，而不是抛异常', () async {
    // 线上真出现这种响应时，调用方必须还能用同一份字节走基线。
    final broken = await RustDmView.parseSubtitles(
      // 字段号 3（subtitle）声明为长度前缀，但长度超出剩余字节。
      Uint8List.fromList([0x1a, 0x7f, 0x01, 0x02]),
    );
    expect(broken, isNull, reason: '损坏的响应本应降级（返回 null）');
  }, skip: bridgeSkip);

  test('开关关闭时不碰 Rust', () async {
    await RustFlags.setRustDmViewSubtitle(false);
    addTearDown(() => RustFlags.setRustDmViewSubtitle(true));
    expect(
      await RustDmView.parseSubtitles(
        File('$fixtureDir/typical.raw.pb').readAsBytesSync(),
      ),
      isNull,
      reason: '开关关闭却走了 Rust——降级开关失效',
    );
  }, skip: bridgeSkip);
}
