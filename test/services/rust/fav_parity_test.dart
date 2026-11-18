import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 收藏夹那一族的**一次性基线对照**（S8 #58，机制见 ADR-011 决策五）。
///
/// ## 为什么是逐行转写，而不是 import 基线模型
///
/// 这一族的基线模型在**本 PR 里已经删除**（`lib/models_new/fav/`、
/// `lib/models_new/sub/{sub,sub_detail}/`、
/// `lib/models_new/space/{space_fav,space_cheese}/`）——它们的生产者
/// `lib/http/fav.dart` 整个迁进了 Rust，照 ADR-011 决策四不留旧实现。
/// 被删掉的东西 import 不进来。
///
/// 下面的 `_Baseline*` 是那些 `fromJson` 的逐行转写，取自本 PR 的父提交，
/// 写下之后不再改，也不许顺手修正任何怪异之处——包括
/// `FavDetailItemModel` 里 `json['bvid'] ?? json['bv_id']` 的双拼法、
/// `FavPgcItemModel` 把空串 `progress` 归一成 null，以及
/// `SubItemModel` 把 `cnt_info.danmaku` 当「收藏数」读。
///
/// ## 这个测试的寿命
///
/// 一次性。基线实现已经不在仓库里，本文件证明的是「删掉它们的那一刻，
/// Rust 的输出与它们逐字段一致」。**下一个碰这一族的 PR 可以连同它一起
/// 删**，长期留在仓库里的是
/// `rust/crates/bili-api/tests/fixtures/fav/*.rust.json` 那几份快照。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/fav';

  dynamic read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync());

  test('收藏夹列表：Rust 快照与基线逐字段一致', () {
    final data = read('fav_folder_list.raw.json')['data'];
    final snapshot = read('fav_folder_list.rust.json') as Map<String, dynamic>;
    final baseline = _BaselineFavFolderData.fromJson(data);

    expect(snapshot['count'], baseline.count);
    expect(snapshot['has_more'], baseline.hasMore);
    final list = snapshot['list'] as List;
    expect(list.length, baseline.list!.length);
    for (var i = 0; i < list.length; i++) {
      _expectFolder(list[i] as Map<String, dynamic>, baseline.list![i], '第$i项');
    }
  });

  test('收藏夹信息：Rust 快照与基线逐字段一致', () {
    final data = read('fav_folder.raw.json')['data'];
    final snapshot = read('fav_folder.rust.json') as Map<String, dynamic>;
    _expectFolder(snapshot, _BaselineFavFolderInfo.fromJson(data), '收藏夹信息');
  });

  test('同一份收藏夹列表读成 SubItem：与基线的 SpaceFavItemModel 一致', () {
    // 个人空间收藏页在基线里正是拿 SpaceFavItemModel 解这条响应的。
    final list = read('fav_folder_list.raw.json')['data']['list'] as List;
    final snapshot = read('fav_folder_as_sub_list.rust.json');
    final got = snapshot['list'] as List;
    expect(got.length, list.length);
    for (var i = 0; i < list.length; i++) {
      final baseline = _BaselineSubItemModel.fromJson(
        list[i] as Map<String, dynamic>,
      );
      _expectSubItem(got[i] as Map<String, dynamic>, baseline, '第$i项');
    }
  });

  test('收藏夹详情：Rust 快照与基线逐字段一致', () {
    final data = read('fav_detail.raw.json')['data'];
    final snapshot = read('fav_detail.rust.json') as Map<String, dynamic>;
    final baseline = _BaselineFavDetailData.fromJson(data, pageDesc: false);

    expect(snapshot['has_more'], baseline.hasMore);
    _expectFolder(
      snapshot['info'] as Map<String, dynamic>,
      baseline.info!,
      '夹子信息',
    );

    final medias = snapshot['medias'] as List;
    expect(medias.length, baseline.medias!.length);
    for (var i = 0; i < medias.length; i++) {
      final got = medias[i] as Map<String, dynamic>;
      final want = baseline.medias![i];
      final at = '第$i项';
      expect(got['id'], want.id, reason: '$at id');
      expect(got['kind'], want.type, reason: '$at type');
      expect(got['title'], want.title, reason: '$at title');
      expect(got['cover'], want.cover, reason: '$at cover');
      expect(got['intro'], want.intro, reason: '$at intro');
      expect(got['duration'], want.duration, reason: '$at duration');
      expect(got['upper_mid'], want.upperMid, reason: '$at upper.mid');
      expect(got['upper_name'], want.upperName, reason: '$at upper.name');
      expect(got['upper_face'], want.upperFace, reason: '$at upper.face');
      expect(got['attr'], want.attr, reason: '$at attr');
      expect(got['play_count'], want.play, reason: '$at cnt_info.play');
      expect(got['danmaku_count'], want.danmaku, reason: '$at cnt_info.danmaku');
      expect(got['fav_time'], want.favTime, reason: '$at fav_time');
      expect(got['bvid'], want.bvid, reason: '$at bvid（两个拼法都要认）');
      expect(got['ogv_type_name'], want.ogvTypeName, reason: '$at ogv.type_name');
      expect(got['ogv_season_id'], want.ogvSeasonId, reason: '$at ogv.season_id');
      expect(got['ugc_first_cid'], want.ugcFirstCid, reason: '$at ugc.first_cid');
    }
  });

  test('收藏夹详情倒序：与基线 pageDesc 的 reversed 一致', () {
    final data = read('fav_detail.raw.json')['data'];
    final snapshot = read('fav_detail_desc.rust.json') as Map<String, dynamic>;
    final baseline = _BaselineFavDetailData.fromJson(data, pageDesc: true);
    final medias = snapshot['medias'] as List;
    expect(
      medias.map((e) => e['id']).toList(),
      baseline.medias!.map((e) => e.id).toList(),
    );
  });

  test('合集详情：Rust 快照与基线逐字段一致', () {
    final data = read('fav_season_detail.raw.json')['data'];
    final snapshot =
        read('fav_season_detail.rust.json') as Map<String, dynamic>;
    final baseline = _BaselineSubDetailData.fromJson(data, pageDesc: false);

    _expectSubItem(
      snapshot['info'] as Map<String, dynamic>,
      baseline.info!,
      '合集信息',
    );

    final medias = snapshot['medias'] as List;
    expect(medias.length, baseline.medias!.length);
    for (var i = 0; i < medias.length; i++) {
      final got = medias[i] as Map<String, dynamic>;
      final want = baseline.medias![i];
      final at = '第$i项';
      expect(got['id'], want.id, reason: '$at id');
      expect(got['title'], want.title, reason: '$at title');
      expect(got['cover'], want.cover, reason: '$at cover');
      expect(got['duration'], want.duration, reason: '$at duration');
      expect(got['pubtime'], want.pubtime, reason: '$at pubtime');
      expect(got['bvid'], want.bvid, reason: '$at bvid');
      expect(got['play_count'], want.play, reason: '$at cnt_info.play');
      expect(got['danmaku_count'], want.danmaku, reason: '$at cnt_info.danmaku');
    }
  });

  test('追番列表：Rust 快照与基线逐字段一致', () {
    final data = read('fav_pgc.raw.json')['data'];
    final snapshot = read('fav_pgc.rust.json') as Map<String, dynamic>;
    final baseline = _BaselineFavPgcData.fromJson(data);

    expect(snapshot['total'], baseline.total);
    final list = snapshot['list'] as List;
    expect(list.length, baseline.list!.length);
    for (var i = 0; i < list.length; i++) {
      final got = list[i] as Map<String, dynamic>;
      final want = baseline.list![i];
      final at = '第$i项';
      expect(got['season_id'], want.seasonId, reason: '$at season_id');
      expect(got['title'], want.title, reason: '$at title');
      expect(got['cover'], want.cover, reason: '$at cover');
      expect(got['is_finish'], want.isFinish, reason: '$at is_finish');
      expect(got['badge'], want.badge, reason: '$at badge');
      expect(
        got['new_ep_index_show'],
        want.indexShow,
        reason: '$at new_ep.index_show',
      );
      expect(got['renewal_time'], want.renewalTime, reason: '$at renewal_time');
      expect(got['progress'], want.progress, reason: '$at progress（空串归一）');
    }
  });

  test('课程收藏：Rust 快照与基线逐字段一致', () {
    final data = read('fav_pugv.raw.json')['data'];
    final snapshot = read('fav_pugv.rust.json') as Map<String, dynamic>;
    final baseline = _BaselineSpaceCheeseData.fromJson(data);

    expect(snapshot['has_next'], baseline.next, reason: 'page.next');
    final items = snapshot['items'] as List;
    expect(items.length, baseline.items!.length);
    for (var i = 0; i < items.length; i++) {
      final got = items[i] as Map<String, dynamic>;
      final want = baseline.items![i];
      final at = '第$i项';
      expect(got['cover'], want.cover, reason: '$at cover');
      // 基线 marks 为 null 时界面当空列表用；Rust 直接给空列表。
      expect(got['marks'], want.marks ?? const [], reason: '$at marks');
      expect(got['season_id'], want.seasonId, reason: '$at season_id');
      expect(got['status'], want.status, reason: '$at status');
      expect(got['title'], want.title, reason: '$at title');
      expect(got['ctime'], want.ctime, reason: '$at ctime');
    }
  });

  test('话题收藏：Rust 快照与基线逐字段一致', () {
    final data = read('fav_topic.raw.json')['data'];
    final snapshot = read('fav_topic.rust.json') as Map<String, dynamic>;
    final baseline = _BaselineFavTopicData.fromJson(data);

    expect(snapshot['total'], baseline.total, reason: 'page_info.total');
    final items = snapshot['items'] as List;
    expect(items.length, baseline.items!.length);
    for (var i = 0; i < items.length; i++) {
      expect(items[i]['id'], baseline.items![i].id, reason: '第$i项 id');
      expect(items[i]['name'], baseline.items![i].name, reason: '第$i项 name');
    }
  });

  test('专栏收藏：Rust 快照与基线逐字段一致', () {
    final data = read('fav_article.raw.json')['data'];
    final snapshot = read('fav_article.rust.json') as Map<String, dynamic>;
    final baseline = _BaselineFavArticleData.fromJson(data);

    expect(snapshot['has_more'], baseline.hasMore);
    final items = snapshot['items'] as List;
    expect(items.length, baseline.items!.length);
    for (var i = 0; i < items.length; i++) {
      final got = items[i] as Map<String, dynamic>;
      final want = baseline.items![i];
      final at = '第$i项';
      expect(got['opus_id'], want.opusId, reason: '$at opus_id');
      expect(got['content'], want.content, reason: '$at content');
      expect(got['author_name'], want.authorName, reason: '$at author.name');
      expect(got['cover_url'], want.coverUrl, reason: '$at cover.url');
      expect(got['like'], want.like, reason: '$at stat.like');
      expect(got['pub_time'], want.pubTime, reason: '$at pub_time');
    }
  });

  test('笔记列表：Rust 快照与基线逐字段一致，且 id 不丢精度', () {
    final list = read('fav_notes.raw.json')['data']['list'] as List;
    final snapshot = read('fav_notes.rust.json') as List;
    expect(snapshot.length, list.length);
    for (var i = 0; i < list.length; i++) {
      final want = _BaselineFavNoteItemModel.fromJson(
        list[i] as Map<String, dynamic>,
      );
      final got = snapshot[i] as Map<String, dynamic>;
      final at = '第$i项';
      expect(got['web_url'], want.webUrl, reason: '$at web_url');
      expect(got['title'], want.title, reason: '$at title');
      expect(got['summary'], want.summary, reason: '$at summary');
      expect(got['message'], want.message, reason: '$at message');
      expect(got['pic'], want.pic, reason: '$at arc.pic');
      // 基线是 dynamic 后直接 join(',')，发出去的就是 toString 的结果。
      expect(got['cvid'], want.cvid?.toString(), reason: '$at cvid');
      expect(got['note_id'], want.noteId?.toString(), reason: '$at note_id');
    }
    expect(
      snapshot[0]['note_id'],
      '9007199254740993',
      reason: '超出 JS 安全整数范围的 id 不能丢位',
    );
  });

  test('空间收藏页：Rust 快照与基线逐字段一致', () {
    final data = read('space_fav.raw.json')['data'] as List;
    final snapshot = read('space_fav.rust.json') as List;
    expect(snapshot.length, data.length);
    for (var g = 0; g < data.length; g++) {
      final rawGroup = data[g] as Map<String, dynamic>;
      final gotGroup = snapshot[g] as Map<String, dynamic>;
      expect(gotGroup['id'], rawGroup['id'], reason: '第$g组 id');
      expect(gotGroup['name'], rawGroup['name'], reason: '第$g组 name');
      final media = rawGroup['mediaListResponse'] as Map<String, dynamic>;
      expect(gotGroup['count'], media['count'], reason: '第$g组 count');

      final rawList = media['list'] as List;
      final gotList = gotGroup['list'] as List;
      expect(gotList.length, rawList.length);
      for (var i = 0; i < rawList.length; i++) {
        _expectSubItem(
          gotList[i] as Map<String, dynamic>,
          _BaselineSubItemModel.fromJson(rawList[i] as Map<String, dynamic>),
          '第$g组第$i项',
        );
      }
    }
  });
}

void _expectFolder(
  Map<String, dynamic> got,
  _BaselineFavFolderInfo want,
  String at,
) {
  expect(got['id'], want.id, reason: '$at id');
  expect(got['fid'], want.fid, reason: '$at fid');
  expect(got['mid'], want.mid, reason: '$at mid');
  expect(got['attr'], want.attr, reason: '$at attr');
  expect(got['title'], want.title, reason: '$at title');
  expect(got['cover'], want.cover, reason: '$at cover');
  expect(got['upper_mid'], want.upperMid, reason: '$at upper.mid');
  expect(got['upper_name'], want.upperName, reason: '$at upper.name');
  expect(got['upper_face'], want.upperFace, reason: '$at upper.face');
  expect(got['intro'], want.intro, reason: '$at intro');
  expect(got['fav_state'], want.favState, reason: '$at fav_state');
  expect(got['media_count'], want.mediaCount, reason: '$at media_count');
}

void _expectSubItem(
  Map<String, dynamic> got,
  _BaselineSubItemModel want,
  String at,
) {
  expect(got['id'], want.id, reason: '$at id');
  expect(got['fid'], want.fid, reason: '$at fid');
  expect(got['mid'], want.mid, reason: '$at mid');
  expect(got['attr'], want.attr, reason: '$at attr');
  expect(got['title'], want.title, reason: '$at title');
  expect(got['cover'], want.cover, reason: '$at cover');
  expect(got['upper_mid'], want.upperMid, reason: '$at upper.mid');
  expect(got['upper_name'], want.upperName, reason: '$at upper.name');
  expect(got['upper_face'], want.upperFace, reason: '$at upper.face');
  expect(got['cover_type'], want.coverType, reason: '$at cover_type');
  expect(got['intro'], want.intro, reason: '$at intro');
  expect(got['ctime'], want.ctime, reason: '$at ctime');
  expect(got['mtime'], want.mtime, reason: '$at mtime');
  expect(got['state'], want.state, reason: '$at state');
  expect(got['fav_state'], want.favState, reason: '$at fav_state');
  expect(got['media_count'], want.mediaCount, reason: '$at media_count');
  expect(got['view_count'], want.viewCount, reason: '$at view_count');
  expect(got['kind'], want.type, reason: '$at type');
  expect(got['play_count'], want.play, reason: '$at cnt_info.play');
  // 基线 SubItemModel 把 cnt_info.danmaku 当「收藏数」读，照搬。
  expect(got['collect_count'], want.danmaku, reason: '$at cnt_info.danmaku');
}

// --------------------------------------------------------------- 基线转写

class _BaselineFavFolderInfo {
  _BaselineFavFolderInfo.fromJson(Map<String, dynamic> json)
    : id = json['id'] as int? ?? 0,
      fid = json['fid'] as int?,
      mid = json['mid'] as int? ?? 0,
      attr = json['attr'] as int? ?? 0,
      title = json['title'] as String? ?? '',
      cover = json['cover'] as String? ?? '',
      upperMid = json['upper']?['mid'] as int?,
      upperName = json['upper']?['name'] as String?,
      upperFace = json['upper']?['face'] as String?,
      intro = json['intro'] as String?,
      favState = json['fav_state'] as int?,
      mediaCount = json['media_count'] as int? ?? 0;

  final int id;
  final int? fid;
  final int mid;
  final int attr;
  final String title;
  final String cover;
  final int? upperMid;
  final String? upperName;
  final String? upperFace;
  final String? intro;
  final int? favState;
  final int mediaCount;
}

class _BaselineFavFolderData {
  _BaselineFavFolderData.fromJson(Map<String, dynamic> json)
    : count = json['count'] as int?,
      list = (json['list'] as List<dynamic>?)
          ?.map(
            (e) => _BaselineFavFolderInfo.fromJson(e as Map<String, dynamic>),
          )
          .toList(),
      hasMore = json['has_more'] as bool?;

  final int? count;
  final List<_BaselineFavFolderInfo>? list;
  final bool? hasMore;
}

class _BaselineFavDetailItemModel {
  _BaselineFavDetailItemModel.fromJson(Map<String, dynamic> json)
    : id = json['id'] as int?,
      type = json['type'] as int?,
      title = json['title'] as String?,
      cover = json['cover'] as String?,
      intro = json['intro'] as String?,
      duration = json['duration'] as int?,
      upperMid = json['upper']?['mid'] as int?,
      upperName = json['upper']?['name'] as String?,
      upperFace = json['upper']?['face'] as String?,
      attr = json['attr'] as int?,
      play = json['cnt_info']?['play'] as int?,
      danmaku = json['cnt_info']?['danmaku'] as int?,
      favTime = json['fav_time'] as int?,
      bvid = json['bvid'] ?? json['bv_id'],
      ogvTypeName = json['ogv']?['type_name'],
      ogvSeasonId = json['ogv']?['season_id'],
      ugcFirstCid = json['ugc']?['first_cid'] as int?;

  final int? id;
  final int? type;
  final String? title;
  final String? cover;
  final String? intro;
  final int? duration;
  final int? upperMid;
  final String? upperName;
  final String? upperFace;
  final int? attr;
  final int? play;
  final int? danmaku;
  final int? favTime;
  final String? bvid;
  final String? ogvTypeName;
  final int? ogvSeasonId;
  final int? ugcFirstCid;
}

class _BaselineFavDetailData {
  _BaselineFavDetailData.fromJson(
    Map<String, dynamic> json, {
    bool pageDesc = false,
  }) : info = json['info'] == null
           ? null
           : _BaselineFavFolderInfo.fromJson(
               json['info'] as Map<String, dynamic>,
             ),
       medias =
           (pageDesc
                   ? (json['medias'] as List<dynamic>?)?.reversed
                   : (json['medias'] as List<dynamic>?))
               ?.map(
                 (e) => _BaselineFavDetailItemModel.fromJson(
                   e as Map<String, dynamic>,
                 ),
               )
               .toList(),
       hasMore = json['has_more'] as bool?;

  final _BaselineFavFolderInfo? info;
  final List<_BaselineFavDetailItemModel>? medias;
  final bool? hasMore;
}

class _BaselineSubItemModel {
  _BaselineSubItemModel.fromJson(Map<String, dynamic> json)
    : id = json['id'] as int?,
      fid = json['fid'] as int?,
      mid = json['mid'] as int?,
      attr = json['attr'] as int?,
      title = json['title'] as String?,
      cover = json['cover'] as String?,
      upperMid = json['upper']?['mid'] as int?,
      upperName = json['upper']?['name'] as String?,
      upperFace = json['upper']?['face'] as String?,
      coverType = json['cover_type'] as int?,
      intro = json['intro'] as String?,
      ctime = json['ctime'] as int?,
      mtime = json['mtime'] as int?,
      state = json['state'] as int?,
      favState = json['fav_state'] as int?,
      mediaCount = json['media_count'] as int?,
      viewCount = json['view_count'] as int?,
      type = json['type'] as int?,
      play = json['cnt_info']?['play'] as int?,
      danmaku = json['cnt_info']?['danmaku'] as int?;

  final int? id;
  final int? fid;
  final int? mid;
  final int? attr;
  final String? title;
  final String? cover;
  final int? upperMid;
  final String? upperName;
  final String? upperFace;
  final int? coverType;
  final String? intro;
  final int? ctime;
  final int? mtime;
  final int? state;
  final int? favState;
  final int? mediaCount;
  final int? viewCount;
  final int? type;
  final int? play;
  final int? danmaku;
}

class _BaselineSubDetailItemModel {
  _BaselineSubDetailItemModel.fromJson(Map<String, dynamic> json)
    : id = json['id'] as int?,
      title = json['title'] as String?,
      cover = json['cover'] as String?,
      duration = json['duration'] as int?,
      pubtime = json['pubtime'] as int?,
      bvid = json['bvid'] as String?,
      play = json['cnt_info']?['play'] as int?,
      danmaku = json['cnt_info']?['danmaku'] as int?;

  final int? id;
  final String? title;
  final String? cover;
  final int? duration;
  final int? pubtime;
  final String? bvid;
  final int? play;
  final int? danmaku;
}

class _BaselineSubDetailData {
  _BaselineSubDetailData.fromJson(
    Map<String, dynamic> json, {
    bool pageDesc = false,
  }) : info = json['info'] == null
           ? null
           : _BaselineSubItemModel.fromJson(json['info'] as Map<String, dynamic>),
       medias =
           (pageDesc
                   ? (json['medias'] as List<dynamic>?)?.reversed
                   : (json['medias'] as List<dynamic>?))
               ?.map(
                 (e) => _BaselineSubDetailItemModel.fromJson(
                   e as Map<String, dynamic>,
                 ),
               )
               .toList();

  final _BaselineSubItemModel? info;
  final List<_BaselineSubDetailItemModel>? medias;
}

class _BaselineFavPgcItemModel {
  _BaselineFavPgcItemModel.fromJson(Map<String, dynamic> json)
    : seasonId = json['season_id'] as int?,
      title = json['title'] as String?,
      cover = json['cover'] as String?,
      isFinish = json['is_finish'] as int?,
      badge = json['badge'] as String?,
      indexShow = json['new_ep']?['index_show'] as String?,
      renewalTime = json['renewal_time'] as String?,
      progress = json['progress'] == '' ? null : json['progress'];

  final int? seasonId;
  final String? title;
  final String? cover;
  final int? isFinish;
  final String? badge;
  final String? indexShow;
  final String? renewalTime;
  final String? progress;
}

class _BaselineFavPgcData {
  _BaselineFavPgcData.fromJson(Map<String, dynamic> json)
    : list = (json['list'] as List<dynamic>?)
          ?.map(
            (e) => _BaselineFavPgcItemModel.fromJson(e as Map<String, dynamic>),
          )
          .toList(),
      total = json['total'] as int?;

  final List<_BaselineFavPgcItemModel>? list;
  final int? total;
}

class _BaselineSpaceCheeseItem {
  _BaselineSpaceCheeseItem.fromJson(Map<String, dynamic> json)
    : cover = json['cover'] as String?,
      marks = (json['marks'] as List?)?.cast<String>(),
      seasonId = json['season_id'] as int?,
      status = json['status'] as String?,
      title = json['title'] as String?,
      ctime = json['ctime'] as String?;

  final String? cover;
  final List<String>? marks;
  final int? seasonId;
  final String? status;
  final String? title;
  final String? ctime;
}

class _BaselineSpaceCheeseData {
  _BaselineSpaceCheeseData.fromJson(Map<String, dynamic> json)
    : items = (json['items'] as List<dynamic>?)
          ?.map(
            (e) => _BaselineSpaceCheeseItem.fromJson(e as Map<String, dynamic>),
          )
          .toList(),
      next = json['page']?['next'] as bool?;

  final List<_BaselineSpaceCheeseItem>? items;
  final bool? next;
}

class _BaselineFavTopicItem {
  _BaselineFavTopicItem.fromJson(Map<String, dynamic> json)
    : id = json['id'] as int?,
      name = json['name'] as String?;

  final int? id;
  final String? name;
}

class _BaselineFavTopicData {
  _BaselineFavTopicData.fromJson(Map<String, dynamic> json)
    : items = (json['topic_list']?['topic_items'] as List<dynamic>?)
          ?.map(
            (e) => _BaselineFavTopicItem.fromJson(e as Map<String, dynamic>),
          )
          .toList(),
      total = json['topic_list']?['page_info']?['total'] as int?;

  final List<_BaselineFavTopicItem>? items;
  final int? total;
}

class _BaselineFavArticleItemModel {
  _BaselineFavArticleItemModel.fromJson(Map<String, dynamic> json)
    : opusId = json['opus_id'] as String?,
      content = json['content'] as String?,
      authorName = json['author']?['name'] as String?,
      coverUrl = json['cover']?['url'] as String?,
      like = json['stat']?['like'] as String?,
      pubTime = json['pub_time'] as String?;

  final String? opusId;
  final String? content;
  final String? authorName;
  final String? coverUrl;
  final String? like;
  final String? pubTime;
}

class _BaselineFavArticleData {
  _BaselineFavArticleData.fromJson(Map<String, dynamic> json)
    : items = (json['items'] as List<dynamic>?)
          ?.map(
            (e) => _BaselineFavArticleItemModel.fromJson(
              e as Map<String, dynamic>,
            ),
          )
          .toList(),
      hasMore = json['has_more'] as bool?;

  final List<_BaselineFavArticleItemModel>? items;
  final bool? hasMore;
}

class _BaselineFavNoteItemModel {
  _BaselineFavNoteItemModel.fromJson(Map json)
    : webUrl = json['web_url'],
      title = json['title'],
      summary = json['summary'],
      message = json['message'],
      pic = json['arc']?['pic'],
      cvid = json['cvid'],
      noteId = json['note_id'];

  final String? webUrl;
  final String? title;
  final String? summary;
  final String? message;
  final String? pic;
  final dynamic cvid;
  final dynamic noteId;
}
