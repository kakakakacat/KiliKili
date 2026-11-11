import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/search/suggest.dart';
import 'package:kilikili/models_new/dynamic/dyn_topic_pub_search/data.dart';
import 'package:kilikili/models_new/pagelist/page_item.dart';
import 'package:kilikili/models_new/pgc/pgc_info_model/result.dart';
import 'package:kilikili/services/rust/rust_search_suggest.dart';
import 'package:kilikili/services/rust/search_suggest_result.dart';
import 'package:kilikili/utils/extension/iterable_ext.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

/// 搜索结果、热搜与默认词已经迁到 Rust 的 typed 直连（S9 / issue #59）：
/// `searchByType`、`searchTrending`、`searchRecommend` 现在在
/// `lib/services/rust/rust_search.dart`。
///
/// 本文件剩下的五个函数分两类，都刻意留在原地：
///
/// **1. `searchSuggest`——已迁但形态不同，本切片不动它。**
/// 它走的是更早的「Rust 解析 → JSON 字符串 → Dart 模型」往返形态
/// （`bili-api/src/search_suggest.rs`）。EPIC #67 定下：已迁的七条往返
/// 路径暂不改造成 typed 直连，留到 Phase 2 统一评估。顺手改掉会让本 PR
/// 多出一份与 S9 无关的回归面，而那条路径现在是好的。
///
/// 评估结论记在这里：改造它是**划算**的——它只有一个调用点、模型只有
/// 23 行（`lib/models/search/suggest.dart`）、没有共享运行时模型，是七条
/// 里最便宜的一条。建议在 Phase 2 第一个做。
///
/// **2. 另外四个不属于搜索域，各有归属：**
///
/// - `ab2c` / `ab2cWithDimension`：取分 P 列表，属视频页（S3 / #53）
/// - `pgcInfo` / `pugvInfo`：番剧与课堂详情，属 PGC（S10 / #60）
/// - `topicPubSearch`：话题发布页的搜索，属动态（S5 / #55）
///
/// 它们住在 `search.dart` 里只是历史归档位置的问题。顺手一起迁会把三个
/// 切片的模型拖进搜索的 PR，与 ADR-011 决策六「不要为共享模型把别的切片
/// 拖进自己的 PR」是同一个理由。本文件在最后一个切片迁完后删除。
abstract final class SearchHttp {
  // 获取搜索建议。仍是 JSON 往返形态，见类文档第 1 条。
  @pragma('vm:notify-debugger-on-exception')
  static Future<LoadingState<SearchSuggestModel>> searchSuggest({
    required String term,
  }) async {
    final res = await Request().get(
      Api.searchSuggest,
      queryParameters: await WbiSign.makSign({
        'term': term,
        'highlight': 0,
        'spmid': 333.1365,
        'web_location': 333.1365,
      }),
    );
    final resData = res.data;
    if (resData is Map && resData['code'] == 0) {
      try {
        final body = Map<String, dynamic>.from(resData);
        final rustJson = await RustSearchSuggest.parse(body);
        return Success(searchSuggestFromRustOrDart(body, rustJson));
      } catch (_) {
        if (kDebugMode) rethrow;
      }
    }
    return const Error(null);
  }

  static Future<int?> ab2c({dynamic aid, dynamic bvid, int? part}) async {
    return (await ab2cWithDimension(aid: aid, bvid: bvid, part: part))?.cid;
  }

  static Future<PageItem?> ab2cWithDimension({
    dynamic aid,
    dynamic bvid,
    int? part,
  }) async {
    final res = await Request().get(
      Api.ab2c,
      queryParameters: {'aid': ?aid, 'bvid': ?bvid},
    );
    if (res.data['code'] == 0) {
      if (res.data['data'] case List list) {
        final target = part != null
            ? (list.getOrNull(part - 1) ?? list.firstOrNull)
            : list.firstOrNull;
        if (target != null) {
          return PageItem.fromJson(target);
        }
      }
      return null;
    } else {
      SmartDialog.showToast("ab2c error: ${res.data['message']}");
      return null;
    }
  }

  static Future<LoadingState<PgcInfoModel>> pgcInfo({
    dynamic seasonId,
    dynamic epId,
  }) async {
    final res = await Request().get(
      Api.pgcInfo,
      queryParameters: {
        'season_id': ?seasonId,
        'ep_id': ?epId,
      },
    );
    if (res.data['code'] == 0) {
      return Success(PgcInfoModel.fromJson(res.data['result']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<PgcInfoModel>> pugvInfo({
    dynamic seasonId,
    dynamic epId,
  }) async {
    final res = await Request().get(
      Api.pugvInfo,
      queryParameters: {
        'season_id': ?seasonId,
        'ep_id': ?epId,
      },
    );
    if (res.data['code'] == 0) {
      return Success(PgcInfoModel.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<TopicPubSearchData>> topicPubSearch({
    required String keywords,
    String content = '',
    required int pageNum,
  }) async {
    final res = await Request().get(
      Api.topicPubSearch,
      queryParameters: {
        'keywords': keywords,
        'content': content,
        if (pageNum == 1) ...{
          'page_size': 20,
          'page_num': 1,
        } else
          'offset': 20 * (pageNum - 1),
        'web_location': 333.1365,
      },
    );
    if (res.data['code'] == 0) {
      return Success(TopicPubSearchData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }
}
