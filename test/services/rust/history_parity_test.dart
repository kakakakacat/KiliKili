import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 观看历史 / 稍后再看 / 播放列表的**一次性基线对照**
/// （S7 #57，机制见 ADR-011 决策五）。
///
/// ## 为什么是逐行转写，而不是 import 基线模型
///
/// 这一族的基线模型在**本 PR 里已经删除**（`lib/models_new/history/`、
/// `lib/models_new/later/`、`lib/models_new/media_list/data.dart`）——它们的
/// 生产者全部迁进了 Rust，照 ADR-011 决策四不留旧实现。被删掉的东西
/// import 不进来。
///
/// 下面的 `_Baseline*` 是那几个 `fromJson` 的逐行转写，取自本 PR 的父提交，
/// 写下之后不再改，也不许顺手修正任何怪异之处——包括：
/// - `History.fromJson` 里 `cid == 0 ? null : cid`；
/// - `LaterItemModel.fromJson` 里 `subtitle` 只看 `bangumi` 在不在，
///   **里面的 `season.title` 不存在就会抛**（基线如此，不是本 PR 的事）；
/// - `isCharging` 判的是 `charging_pay?['level'] != null`；
/// - `Dimension.fromJson` 的 `rotate == 1` 宽高对调。
///
/// `MediaListItemModel` 只转写 `fromJson` 里取值的那部分：它 `extends
/// BaseEpisodeItem`，而那个基类还活着（见
/// `lib/services/rust/media_list_compat.dart`），整条继承链搬进测试没有意义。
///
/// ## 这个测试的寿命
///
/// 一次性。基线实现已经不在仓库里，本文件证明的是「删掉它们的那一刻，
/// Rust 的输出与它们逐字段一致」。**下一个碰这一族的 PR 可以连同它一起
/// 删**，长期留在仓库里的是
/// `rust/crates/bili-api/tests/fixtures/history/*.rust.json` 那几份快照。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/history';

  dynamic read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync());

  test('观看历史：Rust 快照与基线逐字段一致', () {
    final data = read('history_page.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('history_page.rust.json') as Map<String, dynamic>;

    final baselineTabs = (data['tab'] as List)
        .map((e) => _BaselineTab.fromJson(e as Map<String, dynamic>))
        .toList();
    final tabs = snapshot['tabs'] as List;
    expect(tabs.length, baselineTabs.length, reason: '分页签数量');
    for (var i = 0; i < tabs.length; i++) {
      final tab = tabs[i] as Map<String, dynamic>;
      expect(tab['kind'], baselineTabs[i].type, reason: '第 $i 个分页签 type');
      expect(tab['name'], baselineTabs[i].name, reason: '第 $i 个分页签 name');
    }

    final baseline = (data['list'] as List)
        .map((e) => _BaselineHistoryItem.fromJson(e as Map<String, dynamic>))
        .toList();
    final list = snapshot['list'] as List;
    expect(list.length, baseline.length, reason: '条目数量');
    for (var i = 0; i < list.length; i++) {
      final got = list[i] as Map<String, dynamic>;
      final want = baseline[i];
      expect(got['title'], want.title, reason: '第 $i 条 title');
      expect(got['cover'], want.cover, reason: '第 $i 条 cover');
      // Rust 把「没有 covers」摊平成空列表：Dart 侧判空的写法是
      // `covers?.isNotEmpty == true`，空列表与 null 走同一条分支。
      expect(got['covers'], want.covers ?? const [], reason: '第 $i 条 covers');
      expect(got['uri'], want.uri, reason: '第 $i 条 uri');
      expect(got['videos'], want.videos, reason: '第 $i 条 videos');
      expect(got['author_name'], want.authorName, reason: '第 $i 条 authorName');
      expect(got['author_mid'], want.authorMid, reason: '第 $i 条 authorMid');
      expect(got['view_at'], want.viewAt, reason: '第 $i 条 viewAt');
      expect(got['progress'], want.progress, reason: '第 $i 条 progress');
      expect(got['badge'], want.badge, reason: '第 $i 条 badge');
      expect(got['show_title'], want.showTitle, reason: '第 $i 条 showTitle');
      expect(got['duration'], want.duration, reason: '第 $i 条 duration');
      expect(got['is_fav'], want.isFav, reason: '第 $i 条 isFav');
      expect(got['kid'], want.kid, reason: '第 $i 条 kid');
      expect(got['tag_name'], want.tagName, reason: '第 $i 条 tagName');
      expect(got['live_status'], want.liveStatus, reason: '第 $i 条 liveStatus');

      final history = got['history'] as Map<String, dynamic>;
      expect(history['oid'], want.oid, reason: '第 $i 条 history.oid');
      expect(history['epid'], want.epid, reason: '第 $i 条 history.epid');
      expect(history['bvid'], want.bvid, reason: '第 $i 条 history.bvid');
      expect(history['page'], want.page, reason: '第 $i 条 history.page');
      expect(history['cid'], want.cid, reason: '第 $i 条 history.cid');
      expect(history['business'], want.business, reason: '第 $i 条 business');
    }
  });

  test('播放进度毫秒：Rust 的规则与基线的 playbackProgress 一致', () {
    final data = read('history_page.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('history_page.rust.json') as Map<String, dynamic>;
    final baseline = (data['list'] as List)
        .map((e) => _BaselineHistoryItem.fromJson(e as Map<String, dynamic>))
        .toList();
    final list = snapshot['list'] as List;

    for (var i = 0; i < list.length; i++) {
      final progress = (list[i] as Map<String, dynamic>)['progress'] as int?;
      final rust = progress == null
          ? null
          : progress == -1
          ? 0
          : progress * 1000;
      expect(rust, baseline[i].playbackProgress, reason: '第 $i 条播放进度');
    }
  });

  test('观看记录开关：data 直接就是布尔值', () {
    expect(read('history_status.rust.json'), read('history_status.raw.json')['data']);
  });

  test('稍后再看：Rust 快照与基线逐字段一致', () {
    final data = read('later_page.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('later_page.rust.json') as Map<String, dynamic>;

    expect(snapshot['count'], data['count'], reason: 'count');

    final baseline = (data['list'] as List)
        .map((e) => _BaselineLaterItem.fromJson(e as Map<String, dynamic>))
        .toList();
    final list = snapshot['list'] as List;
    expect(list.length, baseline.length, reason: '条目数量');
    for (var i = 0; i < list.length; i++) {
      final got = list[i] as Map<String, dynamic>;
      final want = baseline[i];
      expect(got['aid'], want.aid, reason: '第 $i 条 aid');
      expect(got['bvid'], want.bvid, reason: '第 $i 条 bvid');
      expect(got['pic'], want.pic, reason: '第 $i 条 pic');
      expect(got['title'], want.title, reason: '第 $i 条 title');
      expect(got['subtitle'], want.subtitle, reason: '第 $i 条 subtitle');
      expect(got['pubdate'], want.pubdate, reason: '第 $i 条 pubdate');
      expect(got['duration'], want.duration, reason: '第 $i 条 duration');
      expect(got['redirect_url'], want.redirectUrl, reason: '第 $i 条 redirectUrl');
      expect(got['is_cooperation'], want.isCooperation, reason: '第 $i 条 联合投稿');
      expect(got['owner_mid'], want.ownerMid, reason: '第 $i 条 owner.mid');
      expect(got['owner_name'], want.ownerName, reason: '第 $i 条 owner.name');
      expect(got['owner_face'], want.ownerFace, reason: '第 $i 条 owner.face');
      expect(got['view'], want.view, reason: '第 $i 条 stat.view');
      expect(got['danmaku'], want.danmaku, reason: '第 $i 条 stat.danmaku');
      expect(got['ep_id'], want.epId, reason: '第 $i 条 bangumi.ep_id');
      expect(got['season_title'], want.seasonTitle, reason: '第 $i 条 季名');
      expect(got['cid'], want.cid, reason: '第 $i 条 cid');
      expect(got['progress'], want.progress, reason: '第 $i 条 progress');
      expect(got['is_pgc'], want.isPgc, reason: '第 $i 条 isPgc');
      expect(got['pgc_label'], want.pgcLabel, reason: '第 $i 条 pgcLabel');
      expect(got['is_pugv'], want.isPugv, reason: '第 $i 条 isPugv');
      expect(got['is_charging'], want.isCharging, reason: '第 $i 条 isCharging');
      expect(got['width'], want.width, reason: '第 $i 条 宽（已按 rotate 摆正）');
      expect(got['height'], want.height, reason: '第 $i 条 高（已按 rotate 摆正）');
    }
  });

  test('播放列表：Rust 快照与基线逐字段一致', () {
    final data = read('media_list.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('media_list.rust.json') as Map<String, dynamic>;

    final baseline = (data['media_list'] as List)
        .map((e) => _BaselineMediaListItem.fromJson(e as Map<String, dynamic>))
        .toList();
    final list = snapshot['list'] as List;
    expect(list.length, baseline.length, reason: '条目数量');
    for (var i = 0; i < list.length; i++) {
      final got = list[i] as Map<String, dynamic>;
      final want = baseline[i];
      expect(got['aid'], want.aid, reason: '第 $i 条 aid（协议里叫 id）');
      expect(got['bvid'], want.bvid, reason: '第 $i 条 bvid（协议里叫 bv_id）');
      expect(got['title'], want.title, reason: '第 $i 条 title');
      expect(got['intro'], want.intro, reason: '第 $i 条 intro');
      expect(got['cover'], want.cover, reason: '第 $i 条 cover');
      expect(got['duration'], want.duration, reason: '第 $i 条 duration');
      expect(got['badge'], want.badge, reason: '第 $i 条 badge.text');
      expect(got['kind'], want.type, reason: '第 $i 条 type');
      expect(got['upper_mid'], want.upperMid, reason: '第 $i 条 upper.mid');
      expect(got['upper_name'], want.upperName, reason: '第 $i 条 upper.name');
      expect(got['upper_face'], want.upperFace, reason: '第 $i 条 upper.face');
      expect(got['play'], want.play, reason: '第 $i 条 cnt_info.play');
      expect(got['danmaku'], want.danmaku, reason: '第 $i 条 cnt_info.danmaku');
      expect(got['first_page_cid'], want.cid, reason: '第 $i 条 cid（pages 第一项）');
    }
  });
}

// --------------------------------------------------------- 基线逐行转写

class _BaselineTab {
  _BaselineTab.fromJson(Map<String, dynamic> json)
    : type = json['type'] as String?,
      name = json['name'] as String?;

  final String? type;
  final String? name;
}

class _BaselineHistoryItem {
  _BaselineHistoryItem.fromJson(Map<String, dynamic> json)
    : title = json['title'] as String?,
      cover = json['cover'] as String?,
      covers = (json['covers'] as List?)?.cast<String>(),
      uri = json['uri'] as String?,
      videos = json['videos'] as int?,
      authorName = json['author_name'] as String?,
      authorMid = json['author_mid'] as int?,
      viewAt = json['view_at'] as int?,
      progress = json['progress'] as int?,
      badge = json['badge'] as String?,
      showTitle = json['show_title'] as String?,
      duration = json['duration'] as int?,
      isFav = json['is_fav'] as int?,
      kid = json['kid'] as int?,
      tagName = json['tag_name'] as String?,
      liveStatus = json['live_status'] as int?,
      oid = json['history']?['oid'],
      epid = json['history']?['epid'],
      bvid = json['history']?['bvid'],
      page = json['history']?['page'],
      // 基线的 `History.fromJson`：0 不是合法 cid。
      cid = json['history']?['cid'] == 0 ? null : json['history']?['cid'],
      business = json['history']?['business'];

  final String? title;
  final String? cover;
  final List<String>? covers;
  final String? uri;
  final int? videos;
  final String? authorName;
  final int? authorMid;
  final int? viewAt;
  final int? progress;
  final String? badge;
  final String? showTitle;
  final int? duration;
  final int? isFav;
  final int? kid;
  final String? tagName;
  final int? liveStatus;
  final int? oid;
  final int? epid;
  final String? bvid;
  final int? page;
  final int? cid;
  final String? business;

  int? get playbackProgress {
    final progress = this.progress;
    if (progress == null) return null;
    return progress == -1 ? 0 : progress * Duration.millisecondsPerSecond;
  }
}

class _BaselineLaterItem {
  _BaselineLaterItem.fromJson(Map<String, dynamic> json)
    : aid = json['aid'] as int?,
      pic = json['pic'] as String?,
      title = json['title'] as String?,
      pubdate = json['pubdate'] as int?,
      duration = json['duration'] as int?,
      redirectUrl = json['redirect_url'] as String?,
      isCooperation = json['rights']?['is_cooperation'],
      ownerMid = json['owner']?['mid'],
      ownerName = json['owner']?['name'],
      ownerFace = json['owner']?['face'],
      view = json['stat']?['view'],
      danmaku = json['stat']?['danmaku'],
      epId = json['bangumi']?['ep_id'],
      seasonTitle = json['bangumi']?['season']?['title'],
      // 基线只看 `bangumi` 在不在，里面缺 `season.title` 会直接抛。
      subtitle = json['bangumi'] == null
          ? null
          : (json['title'] as String).replaceFirst(
              '${json['bangumi']['season']['title']} ',
              '',
            ),
      cid = json['cid'] as int?,
      progress = json['progress'] as int?,
      bvid = json['bvid'] as String?,
      isPgc = json['is_pgc'] as bool?,
      pgcLabel = json['pgc_label'] == '' ? null : json['pgc_label'],
      isPugv = json['is_pugv'] as bool?,
      isCharging = json['charging_pay']?['level'] != null,
      // 基线的 `Dimension.fromJson`。
      width = json['dimension'] == null
          ? null
          : json['dimension']['rotate'] == 1
          ? json['dimension']['height'] as int?
          : json['dimension']['width'] as int?,
      height = json['dimension'] == null
          ? null
          : json['dimension']['rotate'] == 1
          ? json['dimension']['width'] as int?
          : json['dimension']['height'] as int?;

  final int? aid;
  final String? pic;
  final String? title;
  final int? pubdate;
  final int? duration;
  final String? redirectUrl;
  final int? isCooperation;
  final int? ownerMid;
  final String? ownerName;
  final String? ownerFace;
  final int? view;
  final int? danmaku;
  final int? epId;
  final String? seasonTitle;
  final String? subtitle;
  final int? cid;
  final int? progress;
  final String? bvid;
  final bool? isPgc;
  final String? pgcLabel;
  final bool? isPugv;
  final bool isCharging;
  final int? width;
  final int? height;
}

class _BaselineMediaListItem {
  _BaselineMediaListItem.fromJson(Map<String, dynamic> json)
    : aid = json['id'] as int?,
      intro = json['intro'] as String?,
      play = json['cnt_info']?['play'],
      danmaku = json['cnt_info']?['danmaku'],
      cover = json['cover'] as String?,
      duration = json['duration'] as int?,
      // 基线的 `cid` 是 `pages?.firstOrNull?.id` 这个 getter。
      cid = (json['pages'] as List?)?.firstOrNull?['id'],
      title = json['title'] as String?,
      type = json['type'] as int?,
      upperMid = json['upper']?['mid'],
      upperName = json['upper']?['name'],
      upperFace = json['upper']?['face'],
      bvid = json['bv_id'] as String?,
      badge = json['badge']?['text'];

  final int? aid;
  final String? intro;
  final int? play;
  final int? danmaku;
  final String? cover;
  final int? duration;
  final int? cid;
  final String? title;
  final int? type;
  final int? upperMid;
  final String? upperName;
  final String? upperFace;
  final String? bvid;
  final String? badge;
}
