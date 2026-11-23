import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// S5 续单第一批（issue #150）的一次性基线对照，机制见 ADR-011 决策五。
///
/// `ArticleInfoData`、`ArticleListData`、`TopicItem`、`TopDetails`、
/// `TopicPubSearchData`、`BubbleData` 都已随本次迁移删除，没有「还活着的
/// 基线模型」可借，于是本文件自带 oracle——下面的 `_Baseline*` 是迁移前
/// 那几个 `fromJson` 的**逐行转写**，取自合并前的 `main`：
///
/// - `lib/models_new/article/article_info/{data,stats}.dart`
/// - `lib/models_new/article/article_list/{data,list,article,stats}.dart`
/// - `lib/models_new/dynamic/dyn_topic_top/{top_details,topic_item,topic_creator}.dart`
/// - `lib/models_new/dynamic/dyn_topic_pub_search/{data,page_info}.dart`
/// - `lib/models_new/bubble/*.dart`
///
/// 本测试**刻意不 import `lib/`**，这样它在没打 Flutter SDK 补丁的机器上
/// 也跑得起来（AGENTS.md §5）。
///
/// 转写时不许顺手修正任何怪异之处：它要证明的正是 Rust 与迁移前行为一致。
/// 几处原样保留并在断言里点名的：
///
/// 1. 话题的 `id` / `name` 在基线里是**非空**字段（`json['id']` 直接赋给
///    `int id`），缺任一个就抛。迁移后改成「整条丢掉」而不是抛——对照时
///    把基线会抛的那些条目先过滤掉再比，两边剩下的必须完全一样。
/// 2. 文集的 `author` 基线套的是共享的 `Owner`，迁移后摊平成三个字段，
///    比较时逐字段取。
/// 3. 部落的 `base_info.tribee_info`（两个 e）基线多套一层，迁移后摊平。
/// 4. 部落动态的 `meta` 同样摊平进条目。
///
/// 六份 fixture **全部手工构造**，原因写在
/// `rust/crates/bili-api/tests/dyn_batch1_golden.rs` 的文件头。它们证明的是
/// 映射一致，不能证明我们对协议的理解正确——那要等 Phase 2 的真机验证。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/dyn_batch1';

  Map<String, dynamic> read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as Map<String, dynamic>;

  List<dynamic> readList(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as List<dynamic>;

  test('专栏信息：Rust 快照与基线逐字段一致', () {
    final data = read('article_info.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('article_info.rust.json');
    final b = _BaselineArticleInfo.fromJson(data);

    expect(snapshot['favorite'], b.favorite);
    expect(snapshot['title'], b.title);
    expect(snapshot['origin_image_urls'], b.originImageUrls);
    final stats = snapshot['stats'] as Map<String, dynamic>;
    expect(stats['favorite'], b.stats!.favorite);
    // 基线这里是 `json['like'] as int?`，而样本里 like 是字符串 "901"。
    // 基线会抛；迁移后按 #166 的判据放宽成 901——这是**刻意的不一致**，
    // 展示用的计数不该让整条响应作废。
    expect(stats['like'], 901);
    expect(() => data['stats']['like'] as int?, throwsA(isA<TypeError>()));
    expect(stats['reply'], b.stats!.reply);
    expect(stats['share'], b.stats!.share);
  });

  test('文集：Rust 快照与基线逐字段一致', () {
    final data = read('article_list.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('article_list.rust.json');
    final b = _BaselineArticleList.fromJson(data);

    final info = snapshot['info'] as Map<String, dynamic>;
    expect(info['id'], b.list!.id);
    expect(info['name'], b.list!.name);
    expect(info['image_url'], b.list!.imageUrl);
    expect(info['update_time'], b.list!.updateTime);
    expect(info['words'], b.list!.words);
    // read 在样本里是字符串 "98765"：同上，基线抛，迁移后放宽。
    expect(info['read'], 98765);
    expect(info['articles_count'], b.list!.articlesCount);

    // 作者：基线的 Owner 摊平成三个字段。
    expect(snapshot['author_mid'], b.author!['mid']);
    expect(snapshot['author_name'], b.author!['name']);
    expect(snapshot['author_face'], b.author!['face']);

    final articles = snapshot['articles'] as List<dynamic>;
    expect(articles.length, b.articles!.length);
    for (var i = 0; i < articles.length; i++) {
      final got = articles[i] as Map<String, dynamic>;
      final want = b.articles![i];
      expect(got['id'], want.id);
      expect(got['title'], want.title);
      expect(got['image_urls'], want.imageUrls ?? <String>[]);
      expect(got['summary'], want.summary);
      expect(got['dyn_id_str'], want.dynIdStr);
      if (want.stats == null) {
        expect(got['stats'], isNull);
      } else {
        final stats = got['stats'] as Map<String, dynamic>;
        expect(stats['view'], want.stats!['view']);
        expect(stats['like'], want.stats!['like']);
        // 第一篇的 reply 是字符串 "30"，基线声明 int?，同样刻意放宽。
        expect(stats['reply'], i == 0 ? 30 : want.stats!['reply']);
      }
    }
  });

  test('话题顶部：Rust 快照与基线逐字段一致', () {
    final data = read('topic_top.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('topic_top.rust.json');
    final b = _BaselineTopDetails.fromJson(
      data['top_details'] as Map<String, dynamic>,
    );

    final topic = snapshot['topic'] as Map<String, dynamic>;
    expect(topic['id'], b.topicItem!.id);
    expect(topic['name'], b.topicItem!.name);
    expect(topic['view'], b.topicItem!.view);
    // discuss 是字符串 "890"：基线的 `json['discuss'] ?? 0` 会把它原样留成
    // String 再赋给 int，抛 TypeError；迁移后放宽成 890。
    expect(topic['discuss'], 890);
    expect(topic['fav'], b.topicItem!.fav);
    expect(topic['like'], b.topicItem!.like);
    expect(topic['description'], b.topicItem!.description);
    expect(topic['is_fav'], b.topicItem!.isFav);
    expect(topic['is_like'], b.topicItem!.isLike);

    final creator = snapshot['creator'] as Map<String, dynamic>;
    expect(creator['uid'], b.topicCreator!.uid);
    expect(creator['face'], b.topicCreator!.face);
    expect(creator['name'], b.topicCreator!.name);
  });

  test('推荐话题：缺 id 或 name 的条目两边都不出现', () {
    final data = read('topic_rcmd.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = readList('topic_rcmd.rust.json');
    final raw = data['topic_items'] as List<dynamic>;

    // 基线对缺 id/name 的条目会抛，所以「基线能解析出来的」= 两个字段都在
    // 的那些。迁移后不抛、整条丢掉，剩下的必须与之一一对应。
    final survivable = raw
        .cast<Map<String, dynamic>>()
        .where((e) => e['id'] != null && e['name'] != null)
        .toList();
    expect(snapshot.length, survivable.length);
    expect(snapshot.length, 3, reason: '样本里刻意放了两条残缺的');

    for (var i = 0; i < snapshot.length; i++) {
      final got = snapshot[i] as Map<String, dynamic>;
      final want = survivable[i];
      expect(got['id'], want['id']);
      expect(got['name'], want['name']);
      expect(got['description'], want['description']);
      expect(got['is_fav'], want['is_fav']);
      expect(got['is_like'], want['is_like']);
      // 四个计数：基线是 `?? 0`，迁移后同样落 0；第三条是字符串形态，
      // 基线会抛，迁移后放宽。
      for (final key in ['view', 'discuss', 'fav', 'like']) {
        final rawValue = want[key];
        final expected = switch (rawValue) {
          null => 0,
          final String s => int.parse(s),
          final int v => v,
          _ => fail('样本里不该有别的形态'),
        };
        expect(got[key], expected, reason: '$key 第 $i 条');
      }
    }
  });

  test('部落气泡：Rust 快照与基线逐字段一致', () {
    final data = read('bubble.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('bubble.rust.json');
    final b = _BaselineBubble.fromJson(data);

    // base_info：基线的 tribee_info 多套一层，迁移后摊平。
    final base = snapshot['base_info'] as Map<String, dynamic>;
    expect(base['is_joined'], b.baseInfo!['is_joined']);
    final tribe = b.baseInfo!['tribee_info'] as Map<String, dynamic>;
    expect(base['id'], tribe['id']);
    expect(base['title'], tribe['title']);
    expect(base['sub_title'], tribe['sub_title']);
    expect(base['face_url'], tribe['face_url']);
    expect(base['jump_uri'], tribe['jump_uri']);
    expect(base['summary'], tribe['summary']);

    final content = snapshot['content'] as Map<String, dynamic>;
    expect(content['count'], b.content!['count']);
    final items = content['items'] as List<dynamic>;
    final rawItems = b.content!['dyn_list'] as List<dynamic>;
    expect(items.length, rawItems.length);
    for (var i = 0; i < items.length; i++) {
      final got = items[i] as Map<String, dynamic>;
      final want = rawItems[i] as Map<String, dynamic>;
      expect(got['dyn_id'], want['dyn_id']);
      expect(got['title'], want['title']);
      // meta 摊平进条目；缺 meta 的那条四个字段全 null。
      final meta = want['meta'] as Map<String, dynamic>?;
      expect(got['author'], meta?['author']);
      expect(got['time_text'], meta?['time_text']);
      expect(got['reply_count'], meta?['reply_count']);
      expect(got['view_stat'], meta?['view_stat']);
    }

    final categories = snapshot['categories'] as List<dynamic>;
    final rawCategories = b.category!['category_list'] as List<dynamic>;
    expect(categories.length, rawCategories.length);
    for (var i = 0; i < categories.length; i++) {
      final got = categories[i] as Map<String, dynamic>;
      final want = rawCategories[i] as Map<String, dynamic>;
      expect(got['id'], want['id']);
      expect(got['name'], want['name']);
      expect(got['kind'], want['type'], reason: 'type 在 Rust 侧叫 kind');
    }

    final sort = snapshot['sort_info'] as Map<String, dynamic>;
    expect(sort['show_sort'], b.sortInfo!['show_sort']);
    expect(sort['cur_sort_type'], b.sortInfo!['cur_sort_type']);
    final sortItems = sort['items'] as List<dynamic>;
    final rawSortItems = b.sortInfo!['sort_items'] as List<dynamic>;
    expect(sortItems.length, rawSortItems.length);
    for (var i = 0; i < sortItems.length; i++) {
      final got = sortItems[i] as Map<String, dynamic>;
      final want = rawSortItems[i] as Map<String, dynamic>;
      expect(got['sort_type'], want['sort_type']);
      expect(got['text'], want['text']);
    }
  });
}

// ------------------------------------------------------------- oracle
//
// 以下是迁移前那几个 `fromJson` 的逐行转写。只为对照存在，不要在这里
// 「改好」任何东西。嵌套层里原本套 `Owner` / `Meta` 这类共享或琐碎模型
// 的，转写成 `Map`——它们的 `fromJson` 本身只是按键取值，再转写一遍
// 只会多出一处可能抄错的地方。

class _BaselineArticleInfo {
  _BaselineArticleInfo({
    this.favorite,
    this.stats,
    this.title,
    this.originImageUrls,
  });

  final bool? favorite;
  final _BaselineArticleStats? stats;
  final String? title;
  final List<String>? originImageUrls;

  factory _BaselineArticleInfo.fromJson(Map<String, dynamic> json) =>
      _BaselineArticleInfo(
        favorite: json['favorite'] as bool?,
        stats: json['stats'] == null
            ? null
            : _BaselineArticleStats.fromJson(
                json['stats'] as Map<String, dynamic>,
              ),
        title: json['title'] as String?,
        originImageUrls: (json['origin_image_urls'] as List?)?.cast<String>(),
      );
}

class _BaselineArticleStats {
  _BaselineArticleStats({this.favorite, this.like, this.reply, this.share});

  final int? favorite;
  final int? like;
  final int? reply;
  final int? share;

  /// `like` 刻意**不**在这里取：样本里它是字符串，基线的 `as int?` 会抛，
  /// 而本 oracle 要能跑完才能对照其余字段。那一处差异在断言里单独点名。
  factory _BaselineArticleStats.fromJson(Map<String, dynamic> json) =>
      _BaselineArticleStats(
        favorite: json['favorite'] as int?,
        like: null,
        reply: json['reply'] as int?,
        share: json['share'] as int?,
      );
}

class _BaselineArticleList {
  _BaselineArticleList({this.list, this.articles, this.author});

  final _BaselineArticleListInfo? list;
  final List<_BaselineArticleListItem>? articles;
  final Map<String, dynamic>? author;

  factory _BaselineArticleList.fromJson(Map<String, dynamic> json) =>
      _BaselineArticleList(
        list: json['list'] == null
            ? null
            : _BaselineArticleListInfo.fromJson(
                json['list'] as Map<String, dynamic>,
              ),
        articles: (json['articles'] as List<dynamic>?)
            ?.map(
              (e) =>
                  _BaselineArticleListItem.fromJson(e as Map<String, dynamic>),
            )
            .toList(),
        author: json['author'] as Map<String, dynamic>?,
      );
}

class _BaselineArticleListInfo {
  _BaselineArticleListInfo({
    this.id,
    this.name,
    this.imageUrl,
    this.updateTime,
    this.words,
    this.read,
    this.articlesCount,
  });

  final int? id;
  final String? name;
  final String? imageUrl;
  final int? updateTime;
  final int? words;
  final int? read;
  final int? articlesCount;

  /// `read` 同 [_BaselineArticleStats] 的 `like`：样本里是字符串。
  factory _BaselineArticleListInfo.fromJson(Map<String, dynamic> json) =>
      _BaselineArticleListInfo(
        id: json['id'] as int?,
        name: json['name'] as String?,
        imageUrl: json['image_url'] as String?,
        updateTime: json['update_time'] as int?,
        words: json['words'] as int?,
        read: null,
        articlesCount: json['articles_count'] as int?,
      );
}

class _BaselineArticleListItem {
  _BaselineArticleListItem({
    this.id,
    this.title,
    this.imageUrls,
    this.summary,
    this.dynIdStr,
    this.stats,
  });

  final int? id;
  final String? title;
  final List<String>? imageUrls;
  final String? summary;
  final String? dynIdStr;
  final Map<String, dynamic>? stats;

  factory _BaselineArticleListItem.fromJson(Map<String, dynamic> json) =>
      _BaselineArticleListItem(
        id: json['id'] as int?,
        title: json['title'] as String?,
        imageUrls: (json['image_urls'] as List?)?.cast<String>(),
        summary: json['summary'] as String?,
        dynIdStr: json['dyn_id_str'] as String?,
        stats: json['stats'] as Map<String, dynamic>?,
      );
}

class _BaselineTopDetails {
  _BaselineTopDetails({this.topicItem, this.topicCreator});

  final _BaselineTopicItem? topicItem;
  final _BaselineTopicCreator? topicCreator;

  factory _BaselineTopDetails.fromJson(Map<String, dynamic> json) =>
      _BaselineTopDetails(
        topicItem: json['topic_item'] == null
            ? null
            : _BaselineTopicItem.fromJson(
                json['topic_item'] as Map<String, dynamic>,
              ),
        topicCreator: json['topic_creator'] == null
            ? null
            : _BaselineTopicCreator.fromJson(
                json['topic_creator'] as Map<String, dynamic>,
              ),
      );
}

class _BaselineTopicItem {
  _BaselineTopicItem({
    required this.id,
    required this.name,
    required this.view,
    required this.discuss,
    required this.fav,
    required this.like,
    this.description,
    this.isFav,
    this.isLike,
  });

  final int id;
  final String name;
  final int view;
  final int discuss;
  final int fav;
  final int like;
  final String? description;
  final bool? isFav;
  final bool? isLike;

  /// `discuss` 同上：样本里是字符串，基线会抛。
  factory _BaselineTopicItem.fromJson(Map<String, dynamic> json) =>
      _BaselineTopicItem(
        id: json['id'],
        name: json['name'],
        view: json['view'] ?? 0,
        discuss: 0,
        fav: json['fav'] ?? 0,
        like: json['like'] ?? 0,
        description: json['description'] as String?,
        isFav: json['is_fav'] as bool?,
        isLike: json['is_like'] as bool?,
      );
}

class _BaselineTopicCreator {
  _BaselineTopicCreator({this.uid, this.face, this.name});

  final int? uid;
  final String? face;
  final String? name;

  factory _BaselineTopicCreator.fromJson(Map<String, dynamic> json) =>
      _BaselineTopicCreator(
        uid: json['uid'] as int?,
        face: json['face'] as String?,
        name: json['name'] as String?,
      );
}

class _BaselineBubble {
  _BaselineBubble({this.baseInfo, this.content, this.category, this.sortInfo});

  final Map<String, dynamic>? baseInfo;
  final Map<String, dynamic>? content;
  final Map<String, dynamic>? category;
  final Map<String, dynamic>? sortInfo;

  factory _BaselineBubble.fromJson(Map<String, dynamic> json) =>
      _BaselineBubble(
        baseInfo: json['base_info'] as Map<String, dynamic>?,
        content: json['content'] as Map<String, dynamic>?,
        category: json['category'] as Map<String, dynamic>?,
        sortInfo: json['sort_info'] as Map<String, dynamic>?,
      );
}
