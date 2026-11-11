import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 番剧/课堂详情迁移的**基线对照**（S10 第二批 / issue #60，机制见
/// ADR-011 决策五）。
///
/// 被取代的 `fromJson` 随本 PR 从 `lib/` 删除，借不到活着的基线，所以
/// 下面 `_Baseline` 里是迁移前那一版的**逐字拷贝**：
///
/// - `lib/models_new/pgc/pgc_info_model/result.dart`（`PgcInfoModel`）
/// - 同目录的 `area` / `brief` / `cooperator` / `new_ep` / `publish` /
///   `rating` / `season` / `section` / `up_info` / `user_status` /
///   `user_progress`
/// - `lib/models_new/video/video_detail/dimension.dart` 的 `fromJson`
///   （`rotate == 1` 时换宽高）
/// - `lib/models_new/pgc/pgc_info_model/stat.dart` 的字段映射
///
/// `episode.dart` 与 `stat.dart` 这两个类**没有删**：它们继承共享的
/// `BaseEpisodeItem` / `StatDetail`，那两个基类还有未迁的生产者，按
/// ADR-011 决策六留了单向适配 `lib/services/rust/pgc_detail_compat.dart`。
/// 所以这份 parity 测试在 `BaseEpisodeItem` 的最后一个生产者迁完之前
/// 不会整个删掉，但它仍然照决策五的规则写：不 import `lib/`，写下之后
/// 不再改。
///
/// ## 有意与基线不同的地方，这里断言的是差异本身
///
/// 1. **五个单字段壳摊平**：`Area{name}` → `List<String>`，
///    `NewEp`/`Publish`/`Rating`/`Brief` 各自摊成标量或列表，
///    `user_status.progress.last_ep_id` 提到上一层。
/// 2. **`type` 改名 `kind`**：Rust 的关键字，值不变。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/pgc';

  Map<String, dynamic> readRaw(String name) =>
      jsonDecode(File('$dir/$name.raw.json').readAsStringSync())
          as Map<String, dynamic>;

  Map<String, dynamic> readSnapshot(String name) =>
      jsonDecode(File('$dir/$name.rust.json').readAsStringSync())
          as Map<String, dynamic>;

  void expectEpisode(
    Map<String, dynamic> rust,
    Map<String, dynamic> baseline,
    String where,
  ) {
    expect(rust['id'], baseline['id'], reason: '$where: id');
    expect(rust['aid'], baseline['aid'], reason: '$where: aid');
    expect(rust['cid'], baseline['cid'], reason: '$where: cid');
    expect(rust['ep_id'], baseline['epId'], reason: '$where: epId');
    expect(rust['bvid'], baseline['bvid'], reason: '$where: bvid');
    expect(rust['badge'], baseline['badge'], reason: '$where: badge');
    expect(rust['title'], baseline['title'], reason: '$where: 标题');
    expect(rust['cover'], baseline['cover'], reason: '$where: 封面');
    // 基线把 rotate 消化在 Dimension.fromJson 里，Rust 在解析层做，
    // 两边都应得到已校正的宽高。
    expect(rust['width'], baseline['width'], reason: '$where: width');
    expect(rust['height'], baseline['height'], reason: '$where: height');
    expect(rust['duration'], baseline['duration'], reason: '$where: 时长');
    expect(rust['from'], baseline['from'], reason: '$where: from');
    expect(rust['link'], baseline['link'], reason: '$where: link');
    expect(rust['long_title'], baseline['longTitle'], reason: '$where: 长标题');
    expect(rust['pub_time'], baseline['pubTime'], reason: '$where: 发布时间');
    expect(rust['share_copy'], baseline['shareCopy'], reason: '$where: 分享文案');
    expect(rust['share_url'], baseline['shareUrl'], reason: '$where: 分享链接');
    expect(rust['show_title'], baseline['showTitle'], reason: '$where: 展示标题');
    expect(rust['play'], baseline['play'], reason: '$where: 播放量');
  }

  void expectDetail(String stem, String payloadKey) {
    final raw = readRaw(stem);
    final snap = readSnapshot(stem);
    final baseline = _Baseline.pgcInfo(raw[payloadKey] as Map<String, dynamic>);

    expect(snap['actors'], baseline['actors'], reason: 'actors');
    expect(snap['cover'], baseline['cover'], reason: '封面');
    expect(snap['evaluate'], baseline['evaluate'], reason: '简介');
    expect(snap['media_id'], baseline['mediaId'], reason: 'mediaId');
    expect(snap['season_id'], baseline['seasonId'], reason: 'seasonId');
    expect(
      snap['season_title'],
      baseline['seasonTitle'],
      reason: 'seasonTitle',
    );
    expect(snap['subtitle'], baseline['subtitle'], reason: 'subtitle');
    expect(snap['title'], baseline['title'], reason: '标题');
    // 有意差异 2：type → kind，值不变。
    expect(snap['kind'], baseline['type'], reason: 'type/kind');

    // 有意差异 1：五个单字段壳摊平。
    final areas = (baseline['areas'] as List?) ?? const [];
    expect(
      snap['areas'],
      areas
          .map((e) => (e as Map<String, dynamic>)['name'])
          .where((e) => e != null)
          .toList(),
      reason: 'areas 摊平成字符串列表',
    );
    expect(
      snap['new_ep_desc'],
      (baseline['newEp'] as Map<String, dynamic>?)?['desc'],
      reason: 'newEp.desc',
    );
    expect(
      snap['new_ep_title'],
      (baseline['newEp'] as Map<String, dynamic>?)?['title'],
      reason: 'newEp.title',
    );
    expect(
      snap['pub_time_show'],
      (baseline['publish'] as Map<String, dynamic>?)?['pubTimeShow'],
      reason: 'publish.pubTimeShow',
    );
    expect(
      snap['rating_score'],
      (baseline['rating'] as Map<String, dynamic>?)?['score'],
      reason: 'rating.score',
    );

    final briefImg =
        ((baseline['brief'] as Map<String, dynamic>?)?['img'] as List?) ??
        const [];
    final rustBrief = snap['brief_images'] as List;
    expect(rustBrief.length, briefImg.length, reason: '简介图数');
    for (var i = 0; i < briefImg.length; i++) {
      final b = briefImg[i] as Map<String, dynamic>;
      final r = rustBrief[i] as Map<String, dynamic>;
      expect(r['url'], b['url'], reason: '简介图 $i url');
      expect(
        r['aspect_ratio'],
        (b['aspectRatio'] as num).toDouble(),
        reason: '简介图 $i 宽高比（缺失取 1）',
      );
    }

    final us = baseline['userStatus'] as Map<String, dynamic>?;
    final rustUs = snap['user_status'] as Map<String, dynamic>?;
    expect(rustUs != null, us != null, reason: 'user_status 是否存在');
    if (us != null && rustUs != null) {
      expect(rustUs['favored'], us['favored'], reason: 'favored');
      expect(
        rustUs['last_ep_id'],
        (us['progress'] as Map<String, dynamic>?)?['lastEpId'],
        reason: 'progress.lastEpId 提到上一层',
      );
    }

    final stat = baseline['stat'] as Map<String, dynamic>?;
    final rustStat = snap['stat'] as Map<String, dynamic>?;
    expect(rustStat != null, stat != null, reason: 'stat 是否存在');
    if (stat != null && rustStat != null) {
      expect(rustStat['coin'], stat['coin'], reason: 'coin');
      expect(rustStat['favorite'], stat['favorite'], reason: 'favorite');
      expect(rustStat['like'], stat['like'], reason: 'like');
      expect(rustStat['danmaku'], stat['danmaku'], reason: 'danmaku');
      expect(rustStat['reply'], stat['reply'], reason: 'reply');
      expect(rustStat['share'], stat['share'], reason: 'share');
      expect(rustStat['view'], stat['view'], reason: 'view');
    }

    final up = baseline['upInfo'] as Map<String, dynamic>?;
    final rustUp = snap['up'] as Map<String, dynamic>?;
    expect(rustUp != null, up != null, reason: 'up_info 是否存在');
    if (up != null && rustUp != null) {
      expect(rustUp['mid'], up['mid'], reason: 'up.mid');
      expect(rustUp['avatar'], up['avatar'], reason: 'up.avatar');
      expect(rustUp['uname'], up['uname'], reason: 'up.uname');
    }

    final coop = (baseline['cooperators'] as List?) ?? const [];
    final rustCoop = snap['cooperators'] as List;
    expect(rustCoop.length, coop.length, reason: '创作者数');
    for (var i = 0; i < coop.length; i++) {
      final b = coop[i] as Map<String, dynamic>;
      final r = rustCoop[i] as Map<String, dynamic>;
      expect(r['mid'], b['mid'], reason: '创作者 $i mid');
      expect(r['avatar'], b['avatar'], reason: '创作者 $i 头像');
      expect(r['nick_name'], b['nickName'], reason: '创作者 $i 昵称');
      expect(r['role'], b['role'], reason: '创作者 $i 角色');
    }

    final seasons = (baseline['seasons'] as List?) ?? const [];
    final rustSeasons = snap['seasons'] as List;
    expect(rustSeasons.length, seasons.length, reason: '季度数');
    for (var i = 0; i < seasons.length; i++) {
      final b = seasons[i] as Map<String, dynamic>;
      final r = rustSeasons[i] as Map<String, dynamic>;
      expect(r['media_id'], b['mediaId'], reason: '季 $i mediaId');
      expect(r['season_id'], b['seasonId'], reason: '季 $i seasonId');
      expect(r['season_title'], b['seasonTitle'], reason: '季 $i 标题');
    }

    final eps = (baseline['episodes'] as List?) ?? const [];
    final rustEps = snap['episodes'] as List;
    expect(rustEps.length, eps.length, reason: '正片集数');
    for (var i = 0; i < eps.length; i++) {
      expectEpisode(
        rustEps[i] as Map<String, dynamic>,
        eps[i] as Map<String, dynamic>,
        '正片第 $i 集',
      );
    }

    final sections = (baseline['sections'] as List?) ?? const [];
    final rustSections = snap['sections'] as List;
    expect(rustSections.length, sections.length, reason: '花絮组数');
    for (var i = 0; i < sections.length; i++) {
      final b = sections[i] as Map<String, dynamic>;
      final r = rustSections[i] as Map<String, dynamic>;
      expect(r['title'], b['title'], reason: '花絮组 $i 标题');
      final bEps = (b['episodes'] as List?) ?? const [];
      final rEps = r['episodes'] as List;
      expect(rEps.length, bEps.length, reason: '花絮组 $i 集数');
      for (var j = 0; j < bEps.length; j++) {
        expectEpisode(
          rEps[j] as Map<String, dynamic>,
          bEps[j] as Map<String, dynamic>,
          '花絮组 $i 第 $j 集',
        );
      }
    }
  }

  group('番剧详情', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      expectDetail('season_detail', 'result');
    });

    test('rotate == 1 的那一集两侧都换过宽高', () {
      final raw = readRaw('season_detail');
      final eps = (raw['result'] as Map<String, dynamic>)['episodes'] as List;
      final rotated = eps.firstWhere(
        (e) =>
            ((e as Map<String, dynamic>)['dimension']
                as Map<String, dynamic>?)?['rotate'] ==
            1,
      );
      final dim =
          (rotated as Map<String, dynamic>)['dimension']
              as Map<String, dynamic>;
      // 样本里原始值是 1920x1080，换过之后应是 1080x1920。
      expect(dim['width'], 1920);
      expect(dim['height'], 1080);

      final snapEp = (readSnapshot('season_detail')['episodes'] as List)
          .firstWhere(
            (e) => (e as Map<String, dynamic>)['ep_id'] == rotated['ep_id'],
          );
      expect((snapEp as Map<String, dynamic>)['width'], 1080);
      expect(snapEp['height'], 1920);
    });

    test('缺 pub_time 的那一集回落到 release_date', () {
      final raw = readRaw('season_detail');
      final eps = (raw['result'] as Map<String, dynamic>)['episodes'] as List;
      final fallback = eps.firstWhere(
        (e) =>
            (e as Map<String, dynamic>)['pub_time'] == null &&
            e['release_date'] != null,
      ) as Map<String, dynamic>;

      final snapEp =
          (readSnapshot('season_detail')['episodes'] as List).firstWhere(
            (e) => (e as Map<String, dynamic>)['ep_id'] == fallback['ep_id'],
          ) as Map<String, dynamic>;
      expect(snapEp['pub_time'], fallback['release_date']);
    });
  });

  group('课堂详情', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      expectDetail('pugv_season_detail', 'data');
    });

    test('stat 只给了 views 时，另外三个取 0、其余保持 null', () {
      final snap = readSnapshot('pugv_season_detail');
      final stat = snap['stat'] as Map<String, dynamic>;
      expect(stat['view'], 8888);
      expect((stat['coin'], stat['favorite'], stat['like']), (0, 0, 0));
      expect(stat['danmaku'], isNull);
      expect(stat['reply'], isNull);
      expect(stat['share'], isNull);
    });
  });
}

/// 迁移前那一版 `fromJson` 的逐字拷贝。产出 Map 而非对象，免得把被删掉的
/// 模型类再写一遍；字段名沿用基线的属性名。
abstract final class _Baseline {
  /// `video_detail/dimension.dart`：`rotate == 1` 时换宽高。
  static (int?, int?) dimension(Map<String, dynamic>? json) {
    if (json == null) return (null, null);
    if (json['rotate'] == 1) {
      return (json['height'] as int?, json['width'] as int?);
    }
    return (json['width'] as int?, json['height'] as int?);
  }

  /// `pgc_info_model/episode.dart`
  static Map<String, dynamic> episode(Map<String, dynamic> json) {
    final (width, height) = dimension(
      json['dimension'] as Map<String, dynamic>?,
    );
    return {
      'aid': json['aid'] as int?,
      'badge': json['badge'] as String?,
      'bvid': json['bvid'] as String?,
      'cid': json['cid'] as int?,
      'cover': json['cover'] as String?,
      'width': width,
      'height': height,
      'duration': json['duration'] as int?,
      'epId': json['ep_id'] as int?,
      'from': json['from'] as String?,
      'id': json['id'] as int?,
      'link': json['link'] as String?,
      'longTitle': json['long_title'] as String?,
      'pubTime': json['pub_time'] ?? json['release_date'],
      'shareCopy': json['share_copy'] as String?,
      'shareUrl': json['share_url'] as String?,
      'showTitle': json['show_title'] as String?,
      'title': json['title'] as String?,
      'play': json['play'] as int?,
    };
  }

  /// `pgc_info_model/stat.dart`
  static Map<String, dynamic> stat(Map<String, dynamic> json) => {
    'coin': json['coins'] ?? 0,
    'danmaku': json['danmakus'],
    'favorite': json['favorite'] ?? 0,
    'like': json['likes'] ?? 0,
    'reply': json['reply'],
    'share': json['share'],
    'view': json['views'],
  };

  /// `pgc_info_model/result.dart`
  static Map<String, dynamic> pgcInfo(Map<String, dynamic> json) => {
    'actors': json['actors'] as String?,
    'areas': (json['areas'] as List<dynamic>?)
        ?.map((e) => {'name': (e as Map<String, dynamic>)['name'] as String?})
        .toList(),
    'cover': json['cover'] as String?,
    'episodes': (json['episodes'] as List<dynamic>?)
        ?.map((e) => episode(e as Map<String, dynamic>))
        .toList(),
    'evaluate': json['evaluate'] as String?,
    'mediaId': json['media_id'] as int?,
    'newEp': json['new_ep'] == null
        ? null
        : {
            'desc': (json['new_ep'] as Map<String, dynamic>)['desc'] as String?,
            'title':
                (json['new_ep'] as Map<String, dynamic>)['title'] as String?,
          },
    'publish': json['publish'] == null
        ? null
        : {
            'pubTimeShow':
                (json['publish'] as Map<String, dynamic>)['pub_time_show']
                    as String?,
          },
    'rating': json['rating'] == null
        ? null
        : {
            'score': ((json['rating'] as Map<String, dynamic>)['score'] as num?)
                ?.toDouble(),
          },
    'seasonId': json['season_id'] as int?,
    'seasonTitle': json['season_title'] as String?,
    'sections': (json['section'] as List<dynamic>?)
        ?.map(
          (e) => {
            'title': (e as Map<String, dynamic>)['title'],
            'episodes': (e['episodes'] as List<dynamic>?)
                ?.map((x) => episode(x as Map<String, dynamic>))
                .toList(),
          },
        )
        .toList(),
    'stat': json['stat'] == null
        ? null
        : stat(json['stat'] as Map<String, dynamic>),
    'subtitle': json['subtitle'] as String?,
    'title': json['title'] as String?,
    'type': json['type'] as int?,
    'upInfo': json['up_info'] == null
        ? null
        : {
            'avatar':
                (json['up_info'] as Map<String, dynamic>)['avatar'] as String?,
            'mid': (json['up_info'] as Map<String, dynamic>)['mid'] as int?,
            'uname':
                (json['up_info'] as Map<String, dynamic>)['uname'] as String?,
          },
    'userStatus': json['user_status'] == null
        ? null
        : {
            'progress':
                (json['user_status'] as Map<String, dynamic>)['progress'] ==
                    null
                ? null
                : {
                    'lastEpId':
                        ((json['user_status']
                                as Map<String, dynamic>)['progress']
                            as Map<String, dynamic>)['last_ep_id'],
                  },
            'favored':
                (json['user_status'] as Map<String, dynamic>)['favored']
                    as int?,
          },
    'cooperators': (json['cooperators'] as List?)
        ?.map(
          (e) => {
            'mid': (e as Map<String, dynamic>)['mid'] as int?,
            'avatar': e['avatar'] as String?,
            'nickName': e['nick_name'] as String?,
            'role': e['role'] as String?,
          },
        )
        .toList(),
    'brief': json['brief'] == null
        ? null
        : {
            'img': ((json['brief'] as Map<String, dynamic>)['img'] as List?)
                ?.map(
                  (e) => {
                    'aspectRatio':
                        (e as Map<String, dynamic>)['aspect_ratio'] ?? 1,
                    'url': e['url'] as String?,
                  },
                )
                .toList(),
          },
    'seasons': (json['seasons'] as List<dynamic>?)
        ?.map(
          (e) => {
            'mediaId': (e as Map<String, dynamic>)['media_id'] as int?,
            'seasonId': e['season_id'] as int?,
            'seasonTitle': e['season_title'] as String?,
          },
        )
        .toList(),
  };
}
