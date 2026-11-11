/// `DmView` 字幕轨的**规范化**：把 `DmViewReply` 转成与 Rust 侧
/// `bili_domain::subtitle::Subtitle` 逐字段对应的 Map 列表。
///
/// ## 它与 `DmGrpc.dmViewToSubtitle` 的关系
///
/// 本文件是那段映射（取字段 → 构造 `Subtitle` → `..sort()`）的**转写**，
/// 而不是对 `Subtitle` 实例的序列化。原因与 `video_detail_normalize.dart`
/// 相同：`Subtitle` 经 `path_utils` 依赖 flutter，而 fixture 生成器是纯
/// dart 脚本，跑不起那些依赖。
///
/// 转写意味着可能与基线脱节。兜底在
/// `test/services/rust/dm_view_normalize_test.dart`：它用**真正的**
/// `DmGrpc.dmViewToSubtitle` 解同一份 fixture，逐字段核对本文件的输出。
///
/// 键名跟 Rust 的字段名走（`subtitle_type` 而不是 `type`）：Rust 侧
/// `type` 是关键字。同弹幕那边的 `dm_type`。
///
/// 本文件**不得** import flutter——fixture 生成器是纯 dart 脚本。
library;

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';

/// 规范化一个 `DmViewReply` 里的字幕轨，顺序与基线相同。
///
/// 无字幕时返回空列表。基线在「没有 subtitle 字段」和「列表为空」两种
/// 情况下都返回 null，调用方对两者的处理也相同（`sub.isNotEmpty` 才用），
/// 因此这里不区分。
List<Map<String, dynamic>> normalizeDmViewSubtitles(DmViewReply? reply) {
  if (reply == null || !reply.hasSubtitle()) return const [];
  final items = reply.subtitle.subtitles;
  if (items.isEmpty) return const [];
  return items.map(normalizeSubtitleItem).toList()..sort(compareNormalized);
}

/// 规范化一条字幕轨。字段取舍同基线：`id_str`、`author`、`lan_doc_brief`、
/// `ai_type`、`ai_status`、`role` 都没有消费方。
Map<String, dynamic> normalizeSubtitleItem(SubtitleItem e) => {
  'id': e.id.toInt(),
  'lan': e.lan,
  'lan_doc': e.lanDoc,
  'subtitle_url': e.subtitleUrl,
  'subtitle_type': e.type.value,
  // 基线按 type 判定，不看 lan 是否以 ai 开头。
  'is_ai': e.type == SubtitleType.AI,
};

/// `Subtitle.compareTo` 的转写：含 `zh` 的在前，其次非 AI 在前，否则相等。
int compareNormalized(Map<String, dynamic> a, Map<String, dynamic> b) {
  final aZh = (a['lan'] as String).contains('zh');
  final bZh = (b['lan'] as String).contains('zh');
  if (aZh != bZh) return aZh ? -1 : 1;
  final aAi = a['is_ai'] as bool;
  final bAi = b['is_ai'] as bool;
  if (aAi != bAi) return aAi ? 1 : -1;
  return 0;
}
