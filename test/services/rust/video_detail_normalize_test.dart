import 'dart:convert';
import 'dart:io';

import 'package:PiliPlus/models/model_owner.dart';
import 'package:PiliPlus/models_new/video/video_detail/arc.dart';
import 'package:PiliPlus/models_new/video/video_detail/data.dart';
import 'package:PiliPlus/models_new/video/video_detail/dimension.dart';
import 'package:PiliPlus/models_new/video/video_detail/page.dart';
import 'package:PiliPlus/models_new/video/video_detail/stat.dart';
import 'package:PiliPlus/services/rust/video_detail_normalize.dart';
import 'package:flutter_test/flutter_test.dart';

/// `video_detail_normalize.dart` 是 `VideoDetailData.fromJson` 那套解析规则
/// 的转写（转写的理由见该文件的文档注释）。转写会脱节，本测试是兜底：
/// 用**真正的模型**解析同一份 fixture，再逐字段核对规范化器的输出。
///
/// 下面的 `_fromModel` 读的是**模型实例的字段**，不是 JSON。因此模型改了
/// 解析行为（比如某个默认值、某次宽高交换）而规范化器没跟上时，两边的值
/// 会对不上，这个测试会红。
void main() {
  const fixtureDir = 'rust/crates/bili-api/tests/fixtures/video_detail';

  late List<File> raws;

  setUpAll(() {
    raws =
        Directory(fixtureDir)
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.raw.json'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
  });

  test('fixture 目录非空，否则本测试形同虚设', () {
    expect(raws, isNotEmpty);
  });

  test('规范化结果与 VideoDetailData.fromJson 一致', () {
    for (final raw in raws) {
      final body = jsonDecode(raw.readAsStringSync()) as Map<String, dynamic>;
      final data = body['data'] as Map<String, dynamic>;

      final fromModel = _fromModel(VideoDetailData.fromJson(data));
      final fromNormalizer = normalizeVideoDetail(data);

      expect(
        fromNormalizer,
        fromModel,
        reason:
            '${raw.path}: 规范化器与基线模型脱节了。'
            '改过 lib/models_new/video/video_detail/ 之后，'
            '需同步 lib/services/rust/video_detail_normalize.dart '
            '并重新生成 fixture。',
      );
    }
  });
}

Map<String, dynamic>? _dimension(Dimension? d) =>
    d == null ? null : {'width': d.width, 'height': d.height};

Map<String, dynamic>? _owner(Owner? o) =>
    o == null ? null : {'mid': o.mid, 'name': o.name, 'face': o.face};

Map<String, dynamic>? _stat(VideoStat? s) => s == null
    ? null
    : {
        'view': s.view,
        'danmaku': s.danmaku,
        'reply': s.reply,
        'favorite': s.favorite,
        'coin': s.coin.toInt(),
        'share': s.share,
        'like': s.like,
      };

Map<String, dynamic>? _part(Part? p) => p == null
    ? null
    : {
        'cid': p.cid,
        'page': p.page,
        'from': p.from,
        'part': p.part,
        'duration': p.duration,
        'vid': p.vid,
        'dimension': _dimension(p.dimension),
        'ctime': p.ctime,
        'first_frame': p.firstFrame,
        'badge': p.badge,
      };

Map<String, dynamic>? _arc(Arc? a) => a == null
    ? null
    : {
        'aid': a.aid,
        'pic': a.pic,
        'title': a.title,
        'pubdate': a.pubdate,
        'duration': a.duration,
        'author': _owner(a.author),
        'stat': _stat(a.stat),
        'dimension': _dimension(a.dimension),
      };

Map<String, dynamic> _fromModel(VideoDetailData d) => {
  'bvid': d.bvid,
  'aid': d.aid,
  'videos': d.videos,
  'copyright': d.copyright,
  'pic': d.pic,
  'title': d.title,
  'pubdate': d.pubdate,
  'ctime': d.ctime,
  'desc': d.desc,
  'desc_v2': d.descV2
      ?.map((e) => {'raw_text': e.rawText, 'type': e.type, 'biz_id': e.bizId})
      .toList(),
  'duration': d.duration,
  'is_stein_gate': d.rights?.isSteinGate,
  'owner': _owner(d.owner),
  'stat': _stat(d.stat),
  'argue_msg': d.argueInfo?.argueMsg,
  'cid': d.cid,
  'dimension': _dimension(d.dimension),
  'season_id': d.seasonId,
  'is_upower_exclusive': d.isUpowerExclusive,
  'pages': d.pages?.map(_part).toList(),
  'ugc_season': d.ugcSeason == null
      ? null
      : {
          'id': d.ugcSeason!.id,
          'title': d.ugcSeason!.title,
          'cover': d.ugcSeason!.cover,
          'mid': d.ugcSeason!.mid,
          'sections': d.ugcSeason!.sections
              ?.map(
                (s) => {
                  'season_id': s.seasonId,
                  'id': s.id,
                  'title': s.title,
                  'type': s.type,
                  'episodes': s.episodes
                      ?.map(
                        (e) => {
                          'season_id': e.seasonId,
                          'section_id': e.sectionId,
                          'id': e.id,
                          'aid': e.aid,
                          'bvid': e.bvid,
                          'cid': e.cid,
                          'title': e.title,
                          'attribute': e.attribute,
                          'badge': e.badge,
                          'arc': _arc(e.arc),
                          'page': _part(e.page),
                          'pages': e.pages?.map(_part).toList(),
                        },
                      )
                      .toList(),
                },
              )
              .toList(),
        },
  'staff': d.staff
      ?.map(
        (s) => {
          // 模型里 mid 是 dynamic，规范化时收敛为 int
          'mid': s.mid is int ? s.mid : int.tryParse('${s.mid}'),
          'title': s.title,
          'name': s.name,
          'face': s.face,
          'vip_type': s.vip?.type,
          'vip_status': s.vip?.status ?? 0,
          'official_type': s.official?.type,
          'official_desc': s.official?.desc,
        },
      )
      .toList(),
  'redirect_url': d.redirectUrl,
};
