import 'dart:convert';
import 'dart:io';
import 'dart:math' show pow;

import 'package:flutter_test/flutter_test.dart';

/// 搜索迁移的**基线对照**（S9 / issue #59，机制见 ADR-011 决策五）。
///
/// ## 这个文件里为什么有一份 Dart 解析代码
///
/// typed 直连删掉了 Dart 侧的协议模型，golden fixture 的期望值就失去了
/// 外部来源——只剩 Rust 跟自己比。SponsorBlock 那次还能借到活着的
/// `SegmentItemModel` 当基线；搜索这次，被取代的那些 `fromJson`
/// （`SearchVideoItemModel`、`SearchTrendingItemModel`、`Em.regTitle`、
/// `DurationUtils.parseDuration`）**随本 PR 一起从 `lib/` 删除了**，借不到。
///
/// 所以下面 `_Baseline` 里的代码是从 `lib/models/search/result.dart`、
/// `lib/utils/em.dart`、`lib/utils/duration_utils.dart` 在迁移前那一版
/// **逐字拷贝**过来的，只改了取值入口（原来从 `json[...]` 读，这里一样）。
/// 它不引用 `lib/` 的任何东西，因此不会随 lib 的改动而漂移——这正是要的：
/// 它是**冻结的基线**，不是一份需要维护的第二实现。
///
/// ## 与 ADR-011 的一处偏差，有意为之
///
/// ADR 说「基线模型被删干净的那个 PR 应当连同 parity 测试一起删掉」。
/// 那条针对的是基线还活在 `lib/` 里、parity 只是临时借用的情形。搜索这边
/// 基线和 parity 在同一个 PR 里消失，照字面执行等于这次迁移没有任何外部
/// 对照。因此这里保留本文件：拷贝一经写下就不再改，和 `*.rust.json` 快照
/// 一样是迁移当时行为的冻结证据。Rust 侧日后改动导致输出变化时，这两样
/// 会一起变红。
///
/// 若所有者认为不值得留，删掉本文件即可——快照仍在。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/search';

  Map<String, dynamic> readRaw(String name) =>
      jsonDecode(File('$dir/$name.raw.json').readAsStringSync())
          as Map<String, dynamic>;

  Map<String, dynamic> readSnapshot(String name) =>
      jsonDecode(File('$dir/$name.rust.json').readAsStringSync())
          as Map<String, dynamic>;

  group('视频搜索', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('search_video');
      final snapshot = readSnapshot('search_video');

      final result = (raw['data']['result'] as List)
          .map((e) => _Baseline.videoItem(e as Map<String, dynamic>))
          .toList();
      final items = snapshot['items'] as List;

      expect(
        snapshot['num_results'],
        (raw['data']['numResults'] as num?)?.toInt(),
        reason: 'numResults',
      );
      expect(items.length, result.length, reason: '条数不同');

      for (var i = 0; i < result.length; i++) {
        final b = result[i];
        final r = items[i] as Map<String, dynamic>;
        final where = '第 $i 条（${b['bvid']}）';

        expect(r['aid'], b['aid'], reason: '$where: aid');
        expect(r['bvid'], b['bvid'], reason: '$where: bvid');
        expect(r['id'], b['id'], reason: '$where: id');
        expect(r['arcurl'], b['arcurl'], reason: '$where: arcurl');
        expect(r['pic'], b['pic'], reason: '$where: 封面');
        expect(r['description'], b['description'], reason: '$where: 简介');
        expect(r['pubdate'], b['pubdate'], reason: '$where: 发布时间');
        // 基线把 senddate 读进 ctime。
        expect(r['senddate'], b['ctime'], reason: '$where: senddate');
        expect(r['tag'], b['tag'], reason: '$where: tag');
        expect(r['item_type'], b['type'], reason: '$where: 类型');
        expect(r['roomid'], b['roomid'], reason: '$where: 房间号');
        expect(
          r['is_union_video'],
          b['is_union_video'],
          reason: '$where: 合作标记',
        );

        // 富文本：基线 Em.regTitle 的拆分结果必须逐段一致。差一段就会让
        // 命中高亮画错位置。
        expect(
          r['title_spans'],
          b['title_spans'],
          reason: '$where: 富文本标题分段',
        );
        expect(r['title'], b['title'], reason: '$where: 纯文本标题');

        // 时长：基线 DurationUtils.parseDuration("12:34") == 754。
        // 唯一有意的偏差是「解不出来时基线抛异常、Rust 给 0」，见下面
        // 单独的测试。
        if (b['duration_secs'] != null) {
          expect(
            r['duration_secs'],
            b['duration_secs'],
            reason: '$where: 时长秒数',
          );
        }

        final owner = r['owner'] as Map<String, dynamic>;
        expect(owner['mid'], b['mid'], reason: '$where: UP mid');
        expect(owner['name'], b['author'], reason: '$where: UP 名');
        expect(owner['face'], b['upic'], reason: '$where: UP 头像');

        // 统计：基线走 safeToInt，字符串形态的数字也要读成数字。
        final stat = r['stat'] as Map<String, dynamic>;
        expect(stat['view'], b['play'], reason: '$where: 播放');
        expect(stat['danmaku'], b['danmaku'], reason: '$where: 弹幕');
        expect(stat['favorite'], b['favorite'], reason: '$where: 收藏');
        expect(stat['reply'], b['review'], reason: '$where: 评论');
        expect(stat['like'], b['like'], reason: '$where: 点赞');
      }
    });

    test('时长解不出来时：基线抛异常，Rust 退化成 0', () {
      // 这是本次迁移**有意**偏离基线的一处，钉在这里免得日后被当成 bug
      // "修回去"。基线 int.parse('直播中') 抛 FormatException，调用点把
      // 整次搜索判为失败；一条时长读不出来不该让整页结果消失。
      expect(() => _Baseline.parseDuration('直播中'), throwsFormatException);

      final snapshot = readSnapshot('search_video');
      final last = (snapshot['items'] as List).last as Map<String, dynamic>;
      expect(last['duration_secs'], 0);
      expect(last['bvid'], 'BV1cc4y1c0c3', reason: '其余字段照常可用');
    });
  });

  group('热搜榜', () {
    test('置顶拼接与 show_name 回落与基线一致', () {
      final raw = readRaw('search_trending');
      final snapshot = readSnapshot('search_trending');
      final data = raw['data'] as Map<String, dynamic>;

      // 基线 SearchTrendingData.fromJson(needsTop: true)：top_list 插到
      // list 前面，topCount 记条数。
      final topList = (data['top_list'] as List?) ?? const [];
      final list = (data['list'] as List?) ?? const [];
      final baseline = [
        ...topList.map((e) => _Baseline.trendingItem(e)),
        // 基线对 list 里缺 keyword 的条目会抛异常，整个榜拿不到；Rust
        // 丢掉该条。这里按 Rust 的口径筛掉再比其余字段。
        ...list
            .where((e) => (e as Map)['keyword'] != null)
            .map((e) => _Baseline.trendingItem(e)),
      ];

      expect(snapshot['top_count'], topList.length, reason: '置顶条数');
      final items = snapshot['list'] as List;
      expect(items.length, baseline.length, reason: '条数不同');

      for (var i = 0; i < baseline.length; i++) {
        final b = baseline[i];
        final r = items[i] as Map<String, dynamic>;
        expect(r['keyword'], b['keyword'], reason: '第 $i 条: keyword');
        // 基线 show_name ?? keyword 的回落。
        expect(r['show_name'], b['show_name'], reason: '第 $i 条: 展示名');
        expect(r['icon'], b['icon'], reason: '第 $i 条: 图标');
        expect(
          r['show_live_icon'],
          b['show_live_icon'],
          reason: '第 $i 条: 直播角标',
        );
      }
    });

    test('缺 keyword 的条目：基线抛异常，Rust 丢掉该条', () {
      // 同样是有意偏差。基线 `json['keyword'] as String` 对 null 抛
      // TypeError，整个热搜榜拿不到。
      expect(
        () => _Baseline.trendingItem({'show_name': '没有 keyword'}),
        throwsA(isA<TypeError>()),
      );
    });

    test('recommend_reason 原样带回，排版留给 Dart', () {
      // 基线在模型里就把首个中点换成空格了。Rust 给原值，替换留在 UI。
      final snapshot = readSnapshot('search_trending');
      final first = (snapshot['list'] as List).first as Map<String, dynamic>;
      expect(first['recommend_reason'], '热点·示例解读');
      expect(
        _Baseline.recommendReason('热点·示例解读'),
        '热点 示例解读',
        reason: '基线确实做了替换——这一步现在归 UI',
      );
    });
  });

  group('富文本边界行为', () {
    // Rust 侧用手写扫描器替代了基线的正则，这是两边最容易悄悄分叉的地方。
    //
    // 这里的期望值**逐条抄自** `rust/crates/bili-api/src/search_text.rs`
    // 的单元测试（`title_splits_around_em_markers` 等）。也就是说：Rust
    // 的行为写死在那边，本测试断言 Dart 基线对同样的输入给出同样的结果。
    // 两份实现、同一组期望值，任何一边改了行为都会红。
    //
    // 比上面两个 group 的快照对照更细：快照只覆盖 fixture 里出现过的标题，
    // 而未闭合标签、空内层、孤立代理项这些边界不会出现在真实样本里。
    const expectations = <String, List<(bool, String)>>{
      '普通标题': [(false, '普通标题')],
      '前<em class="keyword">命中</em>后': [
        (false, '前'),
        (true, '命中'),
        (false, '后'),
      ],
      // 相邻标记之间不产生空段落。
      '<em>a</em><em>b</em>': [(true, 'a'), (true, 'b')],
      // 空串守卫只写在基线的 onNonMatch 里，onMatch 没有——于是空的
      // **命中**段会产出，空的非命中段不会。这条是本测试第一次跑就抓到
      // 的分叉：Rust 原本两边都跳过空段，已按基线改回。
      'a<em></em>b': [(false, 'a'), (true, ''), (false, 'b')],
      // 未闭合标签整体不匹配，原样作为非命中段留下——刻意不"修好"。
      '前<em>没有闭合': [(false, '前<em>没有闭合')],
      '<em>a&amp;b</em>&lt;c&gt;': [(true, 'a&b'), (false, '<c>')],
      // &nbsp; 解成 ASCII 空格（U+0020），不是 U+00A0。
      'a&nbsp;b': [(false, 'a b')],
      '&#x41;&#x4E2D;': [(false, 'A中')],
      // &#x 只认 2–4 位十六进制，位数不符原样保留。
      '&#x4;': [(false, '&#x4;')],
      '&#x41414;': [(false, '&#x41414;')],
      '&copy;': [(false, '&copy;')],
      // 空输入不产生空段落。
      '': <(bool, String)>[],
    };

    expectations.forEach((input, expected) {
      test('regTitle(${jsonEncode(input)})', () {
        final spans = _Baseline.regTitle(input);
        expect(
          spans.map((e) => (e['is_em'] as bool, e['text'] as String)).toList(),
          expected,
          reason: 'Dart 基线与 Rust 单元测试写死的期望不一致',
        );
      });
    });

    test('时长解析与 Rust 单元测试的期望一致', () {
      // 抄自 search_text.rs 的 duration_parses_minutes_and_hours
      // 与 duration_accepts_the_fullwidth_colon。
      expect(_Baseline.parseDuration('12:34'), 754);
      expect(_Baseline.parseDuration('1:02:03'), 3723);
      expect(_Baseline.parseDuration('45'), 45);
      expect(_Baseline.parseDuration('12：34'), 754, reason: '全角冒号也认');
      expect(_Baseline.parseDuration(''), 0);
    });
  });
}

/// 迁移前的 Dart 实现，逐字拷贝，**不要改**。
///
/// 来源：
/// - `lib/models/search/result.dart` 的 `SearchVideoItemModel.fromJson`
///   与 `SearchStat.fromJson` / `SearchOwner.fromJson`
/// - `lib/models_new/search/search_trending/list.dart` 的
///   `SearchTrendingItemModel.fromJson`
/// - `lib/utils/em.dart` 的 `regTitle` / `parseHtml`
/// - `lib/utils/duration_utils.dart` 的 `parseDuration`
/// - `lib/utils/parse_int.dart` 的 `safeToInt`
abstract final class _Baseline {
  static final _exp = RegExp('<[^>]*>([^<]*)</[^>]*>');
  static final _htmlRegExp = RegExp(
    r'&(lt|gt|quot|apos|nbsp|amp|#x[a-fA-F\d]{2,4});',
  );
  static final _splitRegex = RegExp(r'[:：]');

  static String parseHtml(String str) {
    return str.replaceAllMapped(
      _htmlRegExp,
      (match) => switch (match.group(1)) {
        'lt' => '<',
        'gt' => '>',
        'quot' => '"',
        'apos' => "'",
        'nbsp' => ' ',
        'amp' => '&',
        var i? when (i.startsWith('#x')) => String.fromCharCode(
          int.parse(i.substring(2), radix: 16),
        ),
        _ => match.group(0)!,
      },
    );
  }

  /// 基线返回 `List<({bool isEm, String text})>`；这里转成 Map 以便与
  /// 快照里的 JSON 直接比较。
  static List<Map<String, Object>> regTitle(String origin) {
    final res = <Map<String, Object>>[];
    origin.splitMapJoin(
      _exp,
      onMatch: (Match match) {
        res.add({'is_em': true, 'text': parseHtml(match[1] ?? match[0]!)});
        return '';
      },
      onNonMatch: (String str) {
        if (str != '') {
          res.add({'is_em': false, 'text': parseHtml(str)});
        }
        return '';
      },
    );
    return res;
  }

  static int parseDuration(String? data) {
    if (data == null || data.isEmpty) {
      return 0;
    }
    final split = data.split(_splitRegex).reversed.map(int.parse).toList();
    var duration = 0;
    for (var i = 0; i < split.length; i++) {
      duration += split[i] * pow(60, i).toInt();
    }
    return duration;
  }

  static int? safeToInt(dynamic value) => switch (value) {
    int _ => value,
    String _ => int.tryParse(value),
    num _ => value.toInt(),
    _ => null,
  };

  /// 基线 `SearchTrendingItemModel` 展示前对 recommend_reason 的处理。
  static String? recommendReason(String? raw) => raw?.replaceFirst('·', ' ');

  static Map<String, dynamic> videoItem(Map<String, dynamic> json) {
    final spans = regTitle(json['title'] as String? ?? '');
    int? duration;
    try {
      duration = parseDuration(json['duration'] as String?);
    } on FormatException {
      // 基线在这里抛出去，调用点把整次搜索判为失败。对照时记成 null，
      // 由专门的测试覆盖这处有意偏差。
      duration = null;
    }
    return {
      'id': json['id'],
      'arcurl': json['arcurl'],
      'aid': json['aid'],
      'bvid': json['bvid'],
      'title_spans': spans,
      'title': spans.map((e) => e['text']).join(),
      'description': json['description'],
      'pic': json['pic'],
      'pubdate': json['pubdate'],
      'ctime': json['senddate'],
      'duration_secs': duration,
      'tag': json['tag'],
      'type': json['type'],
      'roomid': json['roomid'],
      'is_union_video': json['is_union_video'],
      'mid': json['mid'],
      'author': json['author'],
      'upic': json['upic'],
      'play': safeToInt(json['play']),
      'danmaku': safeToInt(json['danmaku']),
      'favorite': safeToInt(json['favorite']),
      'review': safeToInt(json['review']),
      'like': safeToInt(json['like']),
    };
  }

  static Map<String, dynamic> trendingItem(dynamic raw) {
    final json = raw as Map<String, dynamic>;
    return {
      'keyword': json['keyword'] as String,
      'show_name': json['show_name'] ?? json['keyword'],
      'icon': json['icon'] as String?,
      'show_live_icon': json['show_live_icon'] as bool?,
    };
  }
}
