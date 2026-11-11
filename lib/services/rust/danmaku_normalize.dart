/// 弹幕分段的**规范化**：把 `DmSegMobileReply` 转成与 Rust 侧
/// `bili_domain::danmaku::DanmakuSegment` 逐字段对应的 Map。
///
/// 与 `video_detail_normalize.dart` 的角色相同（两个使用者、同一份规则），
/// 但来源不同：那边转写的是一套 JSON 模型解析规则，这边的来源是**生成
/// 代码**（`lib/grpc/bilibili/community/service/dm/v1.pb.dart`）。因此这里
/// 不存在「转写与模型脱节」的风险，只有一个风险：生成代码重新生成后字段
/// 变了。`test/services/rust/danmaku_normalize_test.dart` 用真正的
/// `DmSegMobileReply.fromBuffer` 解析 fixture 的原始字节并与 golden 比对，
/// 字段若变化该测试会红。
///
/// 本文件**不得** import flutter——fixture 生成器是纯 dart 脚本。
library;

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';

/// 规范化一个弹幕分段。
///
/// 只取**服务端下发的字段**。生成代码里的 100（`count`）与 101
/// （`isSelf`）是上游手工加进生成代码的客户端字段，由客户端在合并弹幕、
/// 标记自己发的弹幕时写入，不是解码结果的一部分（详见
/// `rust/crates/bili-domain/src/danmaku.rs`）。
Map<String, dynamic> normalizeDanmakuSegment(DmSegMobileReply reply) => {
  'elems': reply.elems.map(normalizeDanmakuElem).toList(),
  'state': reply.state,
};

/// 规范化一条弹幕。
Map<String, dynamic> normalizeDanmakuElem(DanmakuElem e) => {
  'id': e.id.toInt(),
  'progress': e.progress,
  'mode': e.mode,
  'fontsize': e.fontsize,
  'color': e.color,
  'mid_hash': e.midHash,
  'content': e.content,
  'ctime': e.ctime.toInt(),
  'weight': e.weight,
  'action': e.action,
  'pool': e.pool,
  'id_str': e.idStr,
  'attr': e.attr,
  'like_count': e.likeCount.toInt(),
  'animation': e.animation,
  'extra': e.extra,
  // 枚举取数值而非名称：Rust 侧同样序列化成数值，且名称属于生成代码的
  // 细节，改名不该让 golden 全部失效。
  'colorful': e.colorful.value,
  'dm_type': e.type,
  'oid': e.oid.toInt(),
  'dm_from': e.dmFrom.value,
};

/// 规范化**引擎处理过**的一条弹幕：服务端字段 + 客户端字段。
///
/// `count` / `isSelf` 是 protobuf 字段 100 / 101，服务端不下发，由引擎
/// （基线的 `bucketDanmaku`、Rust 的 `bili_danmaku::engine`）写入。它们
/// 不出现在解码的 golden 里，只出现在引擎的 golden 里——两份 golden
/// 覆盖的是两件事，混在一起会让解码的差异与引擎的差异分不开。
Map<String, dynamic> normalizeProcessedElem(DanmakuElem e) => {
  ...normalizeDanmakuElem(e),
  'count': e.count,
  'is_self': e.isSelf,
};

/// 规范化分桶结果。键是 `progress ~/ 100`。
///
/// 键转成字符串而非保留 int：JSON 的对象键只能是字符串，Rust 侧
/// `BTreeMap<i32, _>` 序列化出来也是字符串键。排序只为文件稳定可读，
/// 比对本身不依赖顺序。
Map<String, dynamic> normalizeBuckets(Map<int, List<DanmakuElem>> buckets) {
  final keys = buckets.keys.toList()..sort();
  return {
    for (final key in keys)
      '$key': buckets[key]!.map(normalizeProcessedElem).toList(),
  };
}
