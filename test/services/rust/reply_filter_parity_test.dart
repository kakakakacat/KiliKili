import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// S2（issue #52）评论过滤规则的**一次性基线对照**，机制见 ADR-011 决策五。
///
/// 规则已迁到 `bili_domain::reply_filter`，`ReplyGrpc` 里的
/// `needRemoveGrpc` / `_needRemoveGoodGrpc` / `_blockAtOnlyReply` /
/// `_blockReplyAtCount` 随之删除——于是没有「还活着的基线实现」可借。
///
/// 因此本文件自带 oracle：下面的 `_baselineNeedRemove` 等是迁移前那几个
/// 方法的**逐行转写**，取自合并前的 `main`（`13a85fdc3`）的
/// `lib/grpc/reply.dart`。转写时**不许顺手修正**任何怪异之处——它要证明的
/// 正是 Rust 与迁移前的行为一致。
///
/// 两侧跑同一份文件
/// （`rust/crates/bili-domain/tests/fixtures/reply_filter/cases.json`）：
/// 本测试是判据，`reply_filter_golden.rs` 是被验的那一侧。期望值与基线
/// 不符时本测试会红。
///
/// **这个测试是有寿命的**：S2 的行为在 Phase 2 真机回归里确认过之后就
/// 可以删掉它，fixture 与 Rust 侧长期留下。
///
/// ## 为什么不构造真的 `ReplyInfo`
///
/// 规则读的是 `message` / `atNameToMid` / `emotes` / `pictures` /
/// `urls` / `root` / `parent` 这几项，转写后读同名字段，判定逻辑一字不
/// 差；而真正要钉住的两端差异是 **Dart `String` 与 `RegExp` 的语义**
/// （UTF-16 码元、大小写、`\d` 之类的构造），那一半在这里是真货。
/// 反过来依赖 protobuf 生成类，只会让本测试在 pb 子树被删的那一片
/// （#52 续单）跟着坏掉，而它那时本来就该走了。
void main() {
  const path =
      'rust/crates/bili-domain/tests/fixtures/reply_filter/cases.json';
  final fixture =
      jsonDecode(File(path).readAsStringSync()) as Map<String, dynamic>;

  group('评论过滤规则与迁移前的基线一致', () {
    final cases = fixture['cases'] as List<dynamic>;

    test('用例不为空，否则本测试形同虚设', () {
      expect(cases, isNotEmpty);
    });

    for (final raw in cases) {
      final c = raw as Map<String, dynamic>;
      final name = c['name'] as String;

      test(name, () {
        final settings = _BaselineSettings.fromJson(
          c['settings'] as Map<String, dynamic>,
        );
        final item = _BaselineReply.fromJson(c['item'] as Map<String, dynamic>);

        expect(
          _baselineNeedRemove(settings, item),
          c['removed'] as bool,
          reason: 'fixture 的期望值与迁移前的基线不符',
        );
      });
    }
  });

  group('两端语义不同的正则', () {
    final patterns =
        (fixture['rejected_patterns'] as Map<String, dynamic>)['patterns']
            as List<dynamic>;

    for (final raw in patterns) {
      final pattern = raw as String;

      // 这些构造在 Dart 侧**编得过**——正是「编得过但语义不同」才危险，
      // 所以 Rust 侧必须拒绝而不是照单全收（ADR-010 决策二）。
      test('$pattern 在基线里能编过，所以 Rust 必须显式拒绝', () {
        expect(() => RegExp(pattern, caseSensitive: false), returnsNormally);
      });
    }
  });
}

// ---------------------------------------------------------------------------
// 以下是迁移前 `ReplyGrpc` 的逐行转写。不要「顺手改好」。
// ---------------------------------------------------------------------------

/// 基线 `Constants.goodsUrlPrefix`。
const String _goodsUrlPrefix = 'https://gaoneng.bilibili.com/tetris';

class _BaselineSettings {
  final Set<int> blackMids;
  final bool antiGoodsReply;
  final bool blockAtOnlyReply;
  final int? blockReplyAtCount;
  final RegExp replyRegExp;
  final bool enableFilter;

  _BaselineSettings({
    required this.blackMids,
    required this.antiGoodsReply,
    required this.blockAtOnlyReply,
    required this.blockReplyAtCount,
    required String banWord,
  }) : // 基线的构造点：`RegExp(Pref.banWordForReply, caseSensitive: false)`。
       replyRegExp = RegExp(banWord, caseSensitive: false),
       enableFilter = banWord.isNotEmpty;

  factory _BaselineSettings.fromJson(Map<String, dynamic> json) =>
      _BaselineSettings(
        blackMids: ((json['blocked_mids'] as List<dynamic>?) ?? const [])
            .map((e) => e as int)
            .toSet(),
        antiGoodsReply: (json['anti_goods'] as bool?) ?? false,
        blockAtOnlyReply: (json['block_at_only'] as bool?) ?? false,
        // 缺字段 = 基线的 null = 不启用。
        blockReplyAtCount: json['block_at_count'] as int?,
        banWord: (json['ban_word'] as String?) ?? '',
      );
}

class _BaselineUrlExtra {
  final int goodsCmControl;
  final bool hasGoodsItemId;
  final bool hasGoodsPrefetchedCache;

  _BaselineUrlExtra.fromJson(Map<String, dynamic> json)
    : goodsCmControl = (json['goods_cm_control'] as int?) ?? 0,
      hasGoodsItemId = (json['has_goods_item_id'] as bool?) ?? false,
      hasGoodsPrefetchedCache =
          (json['has_goods_prefetched_cache'] as bool?) ?? false;
}

class _BaselineReply {
  final int mid;
  final String message;
  final Map<String, int> atNameToMid;
  final bool hasEmotes;
  final bool hasPictures;

  /// 只装**带 `extra` 的** URL，与宿主递给 Rust 的那一批一致。
  ///
  /// 基线写的是 `urls.isNotEmpty && urls.values.any(_hasGoods)`，而
  /// `_hasGoods` 第一句就是 `url.hasExtra()`：没有 extra 的条目恒为
  /// false，所以前半句那个 `isNotEmpty` 本来就是多余的，两种表达等价。
  final List<_BaselineUrlExtra> urlExtras;

  final int root;
  final int parent;

  _BaselineReply.fromJson(Map<String, dynamic> json)
    : mid = (json['mid'] as int?) ?? 0,
      message = (json['message'] as String?) ?? '',
      atNameToMid = {
        for (final name
            in (json['at_names'] as List<dynamic>?) ?? const <dynamic>[])
          name as String: 0,
      },
      hasEmotes = (json['has_emotes'] as bool?) ?? false,
      hasPictures = (json['has_pictures'] as bool?) ?? false,
      urlExtras =
          ((json['url_extras'] as List<dynamic>?) ?? const <dynamic>[])
              .map(
                (e) => _BaselineUrlExtra.fromJson(e as Map<String, dynamic>),
              )
              .toList(),
      root = (json['root'] as int?) ?? 0,
      parent = (json['parent'] as int?) ?? 0;
}

/// 基线 `_hasGoods` + `_needRemoveGoodGrpc`。
bool _baselineNeedRemoveGood(_BaselineReply reply) {
  return reply.urlExtras.any(
        (extra) =>
            extra.goodsCmControl == 1 ||
            extra.hasGoodsItemId ||
            extra.hasGoodsPrefetchedCache,
      ) ||
      reply.message.contains(_goodsUrlPrefix);
}

/// 基线 `_blockAtOnlyReply`，逐 UTF-16 码元扫描，原样转写。
bool _baselineBlockAtOnlyReply(
  _BaselineSettings settings,
  _BaselineReply reply,
) {
  if (!settings.blockAtOnlyReply) return false;

  final atNameToMid = reply.atNameToMid;
  if (atNameToMid.isEmpty) return false;
  if (reply.hasEmotes || reply.hasPictures) return false;

  final message = reply.message;
  final length = message.length;

  int i = 0;

  if (reply.root != reply.parent) {
    i = message.indexOf(':') + 1;
    if (i <= 0) return false;
  }

  var hasAt = false;
  while (i < length) {
    var c = message.codeUnitAt(i);

    if (c case 0x20 /* ` ` */ || 0x0A /* `\n` */ ) {
      i++;
      continue;
    }

    if (c != 0x40 /* `@` */ ) return false;

    i++;
    final nameStart = i;

    while (i < length) {
      c = message.codeUnitAt(i);
      if (c case 0x20 || 0x40 || 0x0A) {
        break;
      }
      i++;
    }

    if (i == nameStart) return false;
    final name = message.substring(nameStart, i);
    if (!atNameToMid.containsKey(name)) return false;

    hasAt = true;
  }

  return hasAt;
}

/// 基线 `_blockReplyAtCount`。
bool _baselineBlockReplyAtCount(
  _BaselineSettings settings,
  _BaselineReply reply,
) {
  return settings.blockReplyAtCount != null &&
      reply.atNameToMid.length >= settings.blockReplyAtCount!;
}

/// 基线 `needRemoveGrpc`。五条判断的顺序照抄。
bool _baselineNeedRemove(_BaselineSettings settings, _BaselineReply reply) {
  return settings.blackMids.contains(reply.mid) ||
      _baselineBlockReplyAtCount(settings, reply) ||
      _baselineBlockAtOnlyReply(settings, reply) ||
      (settings.antiGoodsReply && _baselineNeedRemoveGood(reply)) ||
      (settings.enableFilter && settings.replyRegExp.hasMatch(reply.message));
}
