import 'dart:convert';
import 'dart:io';
import 'dart:math' show pow;

import 'package:flutter_test/flutter_test.dart';

/// S2 续单（issue #179）评论正文分词的**一次性基线对照**，机制见
/// ADR-011 决策五。
///
/// 判定已迁到 `bili_domain::reply_richtext`，`ReplyItemGrpc._buildMessage`
/// 里的分词随之删除——于是没有「还活着的基线实现」可借。
///
/// 因此本文件自带 oracle：下面的 `_baselineSegments` 是迁移前那段代码的
/// **逐行转写**，取自合并前的 `main`（`de0899f81`）的
/// `lib/pages/video/reply/widgets/reply_item_grpc.dart`。转写时**不许顺手
/// 修正**任何怪异之处——它要证明的正是 Rust 与迁移前的行为一致。
///
/// 两侧跑同一份文件
/// （`rust/crates/bili-domain/tests/fixtures/reply_richtext/cases.json`）：
/// 本测试是判据，`reply_richtext_golden.rs` 是被验的那一侧。
///
/// **这个测试是有寿命的**：行为在 Phase 2 真机回归里确认过之后就可以删掉
/// 它，fixture 与 Rust 侧长期留下。
///
/// ## 为什么比较的是「段」而不是 `InlineSpan`
///
/// 基线那段代码的产物是 `TextSpan` / `WidgetSpan`，里面挂着 `NetworkImgLayer`
/// 之类的 app widget，拿到测试里要连整个 widget 层一起拖进来（本机还跑不
/// 起来，见 AGENTS.md §5）。而真正迁走的是**判定**：哪一段是什么、点下去
/// 跳哪。所以转写时把 `spanChildren.add(TextSpan(...))` 换成「记一段」，
/// 其余——正则怎么拼、`splitMapJoin` 怎么切、`insertAll(0, ...)` 把笔记挪到
/// 哪、`matchedUrls` 怎么去重——全是真货。
///
/// 点击行为在基线里藏在 `onTap` 闭包里，转写时**提前求值**成
/// `(action, target)`：同一棵判定树，只是立即算而不是点下去才算。
///
/// ## 一处已知的、故意的行为变更
///
/// 基线备选词的顺序来自 `content.emotes.keys` 等的迭代序。#176 把这些字段
/// 投影成 `HashMap` 之后，Dart 侧拿到的顺序**已经不确定**了：`@甲乙` 与
/// `@甲乙丙` 哪个先匹配，取决于一次哈希迭代。Rust 把它定死成「长度降序」，
/// 于是长的不会被自己的前缀吃掉。
///
/// 这类用例在 fixture 里标了 `baseline_order_dependent`，本测试**跳过**
/// 它们——拿一个本来就不确定的结果当判据，判的只是今天这次哈希。Rust 侧
/// 的 `only_the_known_divergence_is_marked` 守着这个标记别被滥用。
void main() {
  const path =
      'rust/crates/bili-domain/tests/fixtures/reply_richtext/cases.json';
  final fixture =
      jsonDecode(File(path).readAsStringSync()) as Map<String, dynamic>;

  group('正文分词与迁移前的基线一致', () {
    final cases = fixture['cases'] as List<dynamic>;

    test('用例不为空，否则本测试形同虚设', () {
      expect(cases, isNotEmpty);
    });

    for (final raw in cases) {
      final c = raw as Map<String, dynamic>;
      final name = c['name'] as String;
      final skip = c['baseline_order_dependent'] as bool? ?? false;

      test(
        name,
        () {
          final content = _BaselineContent.fromJson(
            c['content'] as Map<String, dynamic>,
          );
          final settings = _BaselineSettings.fromJson(
            c['settings'] as Map<String, dynamic>?,
          );

          expect(
            _baselineSegments(content, settings)
                .map((s) => s.toJson())
                .toList(),
            c['segments'] as List<dynamic>,
            reason: 'fixture 的期望值与迁移前的基线不符',
          );
        },
        skip: skip ? '基线在这一处的结果取决于哈希迭代序，不能当判据' : null,
      );
    }
  });
}

// ─────────────────────────── fixture 的投影 ───────────────────────────

class _BaselineSettings {
  const _BaselineSettings({this.enableWordRe = false, this.videoDurationMs});

  final bool enableWordRe;
  final int? videoDurationMs;

  factory _BaselineSettings.fromJson(Map<String, dynamic>? json) {
    if (json == null) return const _BaselineSettings();
    return _BaselineSettings(
      enableWordRe: json['enable_word_re'] as bool? ?? false,
      videoDurationMs: json['video_duration_ms'] as int?,
    );
  }
}

class _BaselineEmote {
  const _BaselineEmote();

  factory _BaselineEmote.fromJson(Map<String, dynamic> _) =>
      const _BaselineEmote();
}

class _BaselineUrl {
  const _BaselineUrl({
    required this.title,
    required this.appUrlSchema,
    required this.isWordSearch,
    required this.isNote,
  });

  final String title;
  final String appUrlSchema;
  final bool isWordSearch;
  final bool isNote;

  factory _BaselineUrl.fromJson(Map<String, dynamic> json) => _BaselineUrl(
    title: json['title'] as String? ?? '',
    appUrlSchema: json['app_url_schema'] as String? ?? '',
    isWordSearch: json['is_word_search'] as bool? ?? false,
    isNote: json['is_note'] as bool? ?? false,
  );
}

class _BaselineVote {
  const _BaselineVote(this.title);

  final String title;
}

class _BaselineNote {
  const _BaselineNote(this.clickUrl, this.opusId);

  final String? clickUrl;
  final int? opusId;
}

class _BaselineContent {
  const _BaselineContent({
    required this.message,
    required this.emotes,
    required this.topics,
    required this.atNameToMid,
    required this.urls,
    required this.vote,
    required this.note,
    required this.isNote,
    required this.isNoteV2,
  });

  final String message;
  final Map<String, _BaselineEmote> emotes;
  final List<String> topics;
  final Map<String, int> atNameToMid;
  final Map<String, _BaselineUrl> urls;
  final _BaselineVote? vote;
  final _BaselineNote? note;
  final bool isNote;
  final bool isNoteV2;

  factory _BaselineContent.fromJson(Map<String, dynamic> json) {
    final vote = json['vote'] as Map<String, dynamic>?;
    final note = json['note'] as Map<String, dynamic>?;
    return _BaselineContent(
      message: json['message'] as String? ?? '',
      emotes: ((json['emotes'] as Map<String, dynamic>?) ?? const {}).map(
        (key, value) => MapEntry(
          key,
          _BaselineEmote.fromJson(value as Map<String, dynamic>),
        ),
      ),
      topics: ((json['topics'] as List<dynamic>?) ?? const [])
          .cast<String>()
          .toList(),
      atNameToMid: ((json['at_name_to_mid'] as Map<String, dynamic>?) ??
              const {})
          .map((key, value) => MapEntry(key, value as int)),
      urls: ((json['urls'] as Map<String, dynamic>?) ?? const {}).map(
        (key, value) =>
            MapEntry(key, _BaselineUrl.fromJson(value as Map<String, dynamic>)),
      ),
      vote: vote == null ? null : _BaselineVote(vote['title'] as String? ?? ''),
      note: note == null
          ? null
          : _BaselineNote(note['click_url'] as String?, note['opus_id'] as int?),
      isNote: json['is_note'] as bool? ?? false,
      isNoteV2: json['is_note_v2'] as bool? ?? false,
    );
  }
}

/// 一段。字段与 fixture 一一对应，缺省的不写出来。
class _Seg {
  _Seg(
    this.kind,
    this.text, {
    this.action,
    this.target,
    this.mid,
    this.topic,
    this.seek,
  });

  final String kind;
  final String text;
  final String? action;
  final String? target;
  final int? mid;
  final String? topic;
  final int? seek;

  Map<String, dynamic> toJson() => {
    'kind': kind,
    'text': text,
    if (action != null) 'action': action,
    if (action != null) 'target': target,
    if (mid != null) 'mid': mid,
    if (topic != null) 'topic': topic,
    if (seek != null) 'seek': seek,
  };
}

// ───────────────────── 迁移前 `_buildMessage` 的逐行转写 ─────────────────────

/// 基线 `Constants.urlRegex`。
final _urlRegex = RegExp(
  r'https?://[-A-Za-z0-9+&@#/%?=~_|!:,.;]+[-A-Za-z0-9+&@#/%=~_|]',
);

/// 基线 `ReplyItemGrpc._voteRegExp` / `_timeRegExp`。
final _voteRegExp = RegExp(r"^\{vote:\d+?\}$");
final _timeRegExp = RegExp(r'^(?:\d+[:：])?\d+[:：]\d+$');

/// 基线 `DurationUtils.parseDuration`。
int _parseDuration(String? data) {
  if (data == null || data.isEmpty) return 0;
  final split = data.split(RegExp(r'[:：]')).reversed.map(int.parse).toList();
  var duration = 0;
  for (var i = 0; i < split.length; i++) {
    duration += split[i] * pow(60, i).toInt();
  }
  return duration;
}

/// 基线 `String.substring1`。
String _substring1(String s) => s.substring(1, s.length - 1);

/// 基线 `addUrl` 里 `onTap` 的判定，提前求值。
(String, String) _tapOf(String matchStr, _BaselineUrl url) {
  if (url.appUrlSchema.isEmpty) {
    if (RegExp(r'^(av|bv)', caseSensitive: false).hasMatch(matchStr)) {
      return ('archive', matchStr);
    }
    final match = RegExp(
      r'^cv(\d+)|/read/cv(\d+)|note-app/view\?cvid=(\d+)',
      caseSensitive: false,
    ).firstMatch(matchStr);
    final cvid = match?.group(1) ?? match?.group(2) ?? match?.group(3);
    if (cvid != null) return ('article', cvid);
    return ('webview', matchStr);
  }
  if (url.isWordSearch) return ('search', url.title);
  return ('webview', matchStr);
}

List<_Seg> _baselineSegments(
  _BaselineContent content,
  _BaselineSettings settings,
) {
  final List<_Seg> spanChildren = <_Seg>[];
  bool hasNote = false;

  final urlKeys = content.urls.keys;
  final List<String> specialTokens = [
    ...content.emotes.keys,
    ...content.topics.map((e) => '#$e#'),
    ...content.atNameToMid.keys.map((e) => '@$e'),
    ...urlKeys,
  ];
  final String patternStr = [
    ...specialTokens.map(RegExp.escape),
    r'(?:\d+[:：])?\d+[:：]\d+',
    r'\{vote:\d+?\}',
    _urlRegex.pattern,
  ].join('|');
  final RegExp pattern = RegExp(patternStr);

  final List<String> matchedUrls = [];

  void addPlainTextSpan(String str) {
    spanChildren.add(_Seg('text', str));
  }

  void addUrl(String matchStr, _BaselineUrl url, {bool addPlainText = false}) {
    if (url.isWordSearch && !settings.enableWordRe) {
      if (addPlainText) {
        addPlainTextSpan(matchStr);
      }
      return;
    }
    final isCv = url.isNote;
    if (isCv) {
      hasNote = true;
    }
    final (action, target) = _tapOf(matchStr, url);
    // 基线这里是 `[图标 span?, 文本 span]` 两条，图标不参与比较（它是
    // 透传字段，不是判定），于是转写成一段。
    final children = [
      _Seg('link', isCv ? '[笔记] ' : url.title, action: action, target: target),
    ];
    if (isCv) {
      spanChildren.insertAll(0, children);
    } else {
      spanChildren.addAll(children);
    }
  }

  content.message.splitMapJoin(
    pattern,
    onMatch: (Match match) {
      String matchStr = match[0]!;
      final name = matchStr.substring(1);
      final topic = matchStr.length > 1 ? _substring1(matchStr) : '';
      if (content.emotes.containsKey(matchStr)) {
        spanChildren.add(_Seg('emote', matchStr));
      } else if (matchStr.startsWith("@") &&
          content.atNameToMid.containsKey(name)) {
        spanChildren.add(
          _Seg('mention', matchStr, mid: content.atNameToMid[name]),
        );
      } else if (_voteRegExp.hasMatch(matchStr) && content.vote != null) {
        spanChildren.add(_Seg('vote', matchStr));
      } else if (_timeRegExp.hasMatch(matchStr)) {
        matchStr = matchStr.replaceAll('：', ':');
        bool isValid = false;
        try {
          final timeLength = settings.videoDurationMs;
          if (timeLength != null) {
            isValid = _parseDuration(matchStr) * 1000 <= timeLength;
          }
        } catch (_) {
          // 基线这里也是整段吞掉，结果同样是「不可点」。
        }
        spanChildren.add(
          _Seg(
            'timestamp',
            matchStr,
            seek: isValid ? _parseDuration(matchStr) : null,
          ),
        );
      } else {
        final url = content.urls[matchStr];
        if (url != null && !matchedUrls.contains(matchStr)) {
          addUrl(matchStr, url, addPlainText: true);
          matchedUrls.add(matchStr);
        } else if (matchStr.length > 1 && content.topics.contains(topic)) {
          spanChildren.add(
            _Seg(
              'topic',
              matchStr,
              topic: topic,
              action: 'search',
              target: topic,
            ),
          );
        } else if (_urlRegex.hasMatch(matchStr)) {
          spanChildren.add(
            _Seg('link', matchStr, action: 'webview', target: matchStr),
          );
        } else {
          addPlainTextSpan(matchStr);
        }
      }
      return '';
    },
    onNonMatch: (String nonMatchStr) {
      addPlainTextSpan(nonMatchStr);
      return nonMatchStr;
    },
  );

  if (!hasNote && content.isNote && content.isNoteV2) {
    final note = content.note;
    String? action;
    String? target;
    if (note != null) {
      if (note.clickUrl case final clickUrl?) {
        action = 'scheme';
        target = clickUrl;
      } else if (note.opusId case final opusId?) {
        action = 'opus';
        target = opusId.toString();
      }
    }
    spanChildren.insert(
      0,
      _Seg('note', '[笔记] ', action: action, target: target),
    );
  }

  // 基线在相邻两个匹配之间会塞一个 `TextSpan(text: '')`，画出来什么都
  // 没有；Rust 侧直接不发这种段。比较前去掉，这不是行为差异。
  return spanChildren
      .where((s) => s.kind != 'text' || s.text.isNotEmpty)
      .toList();
}
