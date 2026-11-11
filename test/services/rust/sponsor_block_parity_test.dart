import 'dart:convert';
import 'dart:io';

import 'package:kilikili/models_new/sponsor_block/segment_item.dart';
import 'package:flutter_test/flutter_test.dart';

/// typed 直连迁移的**一次性基线对照**（样板，见 issue #64 / ADR-011 #48）。
///
/// SponsorBlock 的四个只读接口已迁到 Rust，Dart 侧不再有解析实现；
/// 但 `SegmentItemModel` 还活着（PGC 片段、下载缓存、尚未迁移的写接口都在
/// 产出它）。趁它还在，用它当**外部基线**：
///
/// - Rust 侧 `tests/sponsor_block_golden.rs` 把解析结果冻结成
///   `segments.rust.json`；
/// - 本测试用基线模型解析同一份 `segments.raw.json`，与那份快照逐字段比对。
///
/// 单看任何一侧都只是"自己跟自己一致"，合起来才是对照。
///
/// **这个测试是有寿命的**：基线模型被删干净的那个 PR（写接口迁移，#50 之后）
/// 应当连同本文件一起删掉，快照则留在仓库里作为迁移当时行为的冻结证据。
/// 其他切片照抄时请一并抄走这句话——parity 测试不是长期资产，长期资产是快照。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/sponsor_block';

  test('Rust 快照与 Dart 基线模型逐字段一致', () {
    final raw =
        jsonDecode(File('$dir/segments.raw.json').readAsStringSync()) as List;
    final snapshot =
        jsonDecode(File('$dir/segments.rust.json').readAsStringSync()) as List;

    final baseline = raw
        .map((e) => SegmentItemModel.fromJson(e as Map<String, dynamic>))
        .toList();

    expect(
      snapshot.length,
      baseline.length,
      reason: 'Rust 保留的片段条数与基线不同',
    );

    for (var i = 0; i < baseline.length; i++) {
      final b = baseline[i];
      final r = snapshot[i] as Map<String, dynamic>;
      final where = '第 $i 条（${b.uuid}）';

      expect(r['uuid'], b.uuid, reason: '$where: uuid');
      expect(r['category'], b.category, reason: '$where: 分类');
      expect(r['action_type'], b.actionType, reason: '$where: 动作');
      expect(r['cid'], b.cid, reason: '$where: cid');
      expect(r['votes'], b.votes, reason: '$where: 票数');

      // 基线把秒 * 1000 后 round()，Rust 必须一致——差 1ms 就会让片段
      // 边界上的跳过判定在两实现间漂移。
      expect(r['start_ms'], b.segment[0], reason: '$where: 起点毫秒');
      expect(r['end_ms'], b.segment[1], reason: '$where: 终点毫秒');

      // 时长基线不取整（num * 1000），保持浮点比较。
      final rustDuration = (r['video_duration_ms'] as num?)?.toDouble();
      final baseDuration = b.videoDuration?.toDouble();
      if (baseDuration == null) {
        expect(rustDuration, isNull, reason: '$where: 时长应为空');
      } else {
        expect(
          rustDuration,
          closeTo(baseDuration, 1e-9),
          reason: '$where: 时长毫秒',
        );
      }
    }
  });
}
