import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/pgc_review_type.dart';
import 'package:kilikili/models_new/pgc/pgc_index_condition/data.dart';
import 'package:kilikili/models_new/pgc/pgc_index_result/data.dart';
import 'package:kilikili/models_new/pgc/pgc_review/data.dart';
import 'package:kilikili/rust/api/pgc.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/pgc.dart'
    show
        PgcConditionFilter,
        PgcConditionOrder,
        PgcConditionValue,
        PgcIndexItem,
        PgcRankItem,
        PgcReviewAuthor,
        PgcReviewItem,
        PgcBriefImage,
        PgcCooperator,
        PgcEpisode,
        PgcReviewStat,
        PgcSeasonBrief,
        PgcSeasonDetail,
        PgcSeasonStat,
        PgcSection,
        PgcUpInfo,
        PgcUserStatus,
        SeasonStatus,
        TimelineDay,
        TimelineEpisode;

/// PGC 的取数路径（S10 第一批 / issue #60）。**typed 直连**。
///
/// ## 覆盖范围
///
/// 索引结果、索引筛选条件、「番剧索引」入口、放送时间表、短评/长评列表、
/// 追番状态、两种排行榜。
///
/// 第二批补上了番剧与课堂详情（`pgcInfo` / `pugvInfo`）。
///
/// **不在这里**：短评的五个写操作仍走 `lib/http/pgc.dart`，归
/// ADR-013（#50）。
///
/// ## 没有降级开关
///
/// typed 直连删掉了 Dart 原实现（ADR-011 决策四已定为 A）。`RustCore`
/// 未就绪时这些页面整体不可用，表现为「取不到数据」而不是崩溃。
///
/// ## 账号按接口分两个桶
///
/// 索引与两种排行归 `AccountType.recommend`，时间表、评论、追番状态没有
/// 列进 `ApiType.apiTypeSet`、落到默认的 `main`。这里一律用
/// [RustRequest.accountFor] 按 path 查表，不硬编码——横跨两个桶时硬编码
/// 迟早错一个，而错了不会报错，只会悄悄换一批数据。
abstract final class RustPgc {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能不可用';

  /// 组装一次请求的上下文。`mixinKey` 为空串表示这条接口不签名。
  static Future<rust_api.PgcContext> _context({
    required String path,
    String mixinKey = '',
  }) async {
    final account = RustRequest.accountFor(path);
    return rust_api.PgcContext(
      baseUrl: HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(account, path),
      headers: RustRequest.headersFor(account),
      mixinKey: mixinKey,
      wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
    );
  }

  /// 失败一律转成 `Error`，不抛异常。
  ///
  /// `Error` 是 `LoadingState<Never>`，能赋给任何 `LoadingState<T>`。
  static Error _failure(String? detail, int? code) =>
      Error(detail ?? '请求失败', code: code);

  /// 取番剧或课堂详情。
  ///
  /// `pugv` 为真时走 `pugvInfo`（载荷在 `data`），否则走 `pgcInfo`
  /// （载荷在 `result`）。两条接口的响应结构相同，只有载荷键不同。
  ///
  /// `seasonId` 与 `epId` 至少要给一个，两个都缺时 Rust 侧不发请求，
  /// 直接回一个 protocol 错误。
  static Future<LoadingState<rust_api.PgcSeasonDetail>> seasonDetail({
    required bool pugv,
    int? seasonId,
    int? epId,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final path = pugv ? Api.pugvInfo : Api.pgcInfo;
    try {
      final context = await _context(path: path);
      final res = pugv
          ? await rust_api.fetchPugvSeasonDetail(
              context: context,
              seasonId: seasonId,
              epId: epId,
            )
          : await rust_api.fetchPgcSeasonDetail(
              context: context,
              seasonId: seasonId,
              epId: epId,
            );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取番剧详情出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取一页索引结果。
  ///
  /// `params` 是用户在筛选面板里选中的键值对，取值来自
  /// [indexCondition] 返回的 `keyword`，原样透传。
  static Future<LoadingState<PgcIndexResult>> indexResult({
    required int page,
    required Map<String, dynamic> params,
    Object? seasonType,
    Object? type,
    Object? indexType,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchPgcIndex(
        context: await _context(path: Api.pgcIndexResult),
        query: rust_api.PgcIndexQuery(
          page: page,
          // 基线把 params 整个摊进查询串，值可能是 int 也可能是 String。
          filters: params.entries.map((e) => (e.key, '${e.value}')).toList(),
          seasonType: _asInt(seasonType),
          type: _asInt(type),
          indexType: _asInt(indexType),
        ),
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(
        PgcIndexResult(hasNext: data.hasNext, list: data.list),
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取 PGC 索引出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取索引的筛选条件与排序方式。
  static Future<LoadingState<PgcIndexConditionData>> indexCondition({
    Object? seasonType,
    required Object type,
    Object? indexType,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchPgcIndexCondition(
        context: await _context(path: Api.pgcIndexCondition),
        seasonType: _asInt(seasonType),
        type: _asInt(type) ?? 1,
        indexType: _asInt(indexType),
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(
        PgcIndexConditionData(filter: data.filter, order: data.order),
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取 PGC 筛选条件出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 「番剧索引」入口。那十几个表示「不限」的 `-1` 由 Rust 侧补齐。
  static Future<LoadingState<List<rust_api.PgcIndexItem>?>> index({
    int? page,
    int? indexType,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchPgcDefaultIndex(
        context: await _context(path: Api.pgcIndexResult),
        page: page,
        indexType: indexType,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data.list);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取番剧索引出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取放送时间表。`types`：1 番剧 / 3 电影 / 4 国创。
  static Future<LoadingState<List<rust_api.TimelineDay>?>> timeline({
    int types = 1,
    required int before,
    required int after,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchPgcTimeline(
        context: await _context(path: Api.pgcTimeline),
        types: types,
        before: before,
        after: after,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取放送时间表出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取一页短评或长评。
  static Future<LoadingState<PgcReviewData>> review({
    required PgcReviewType type,
    required Object mediaId,
    int sort = 0,
    String? next,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchPgcReviews(
        context: await _context(path: type.api),
        long: type == PgcReviewType.long,
        mediaId: _asInt(mediaId) ?? 0,
        sort: sort,
        cursor: next,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(
        PgcReviewData(list: data.list, next: data.next, count: data.count),
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取 PGC 评论出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取追番状态。
  static Future<LoadingState<rust_api.SeasonStatus>> seasonStatus(
    Object seasonId,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchSeasonStatus(
        context: await _context(path: Api.seasonStatus),
        seasonId: _asInt(seasonId) ?? 0,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取追番状态出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取排行榜。`season` 为真走 season 变体，否则走 web 变体。
  ///
  /// 两条都要 WBI 签名，拿不到 key 就别发请求——基线会用空串继续签名并
  /// 换回一个业务错误码，那只是把失败推后，还更难诊断。
  static Future<LoadingState<List<rust_api.PgcRankItem>?>> rank({
    required bool season,
    int day = 3,
    required int seasonType,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final mixinKey = await WbiSign.getWbiKeys();
      if (mixinKey.isEmpty) return const Error('WBI 密钥获取失败，请稍后重试');

      final res = await rust_api.fetchPgcRank(
        context: await _context(
          path: season ? Api.pgcSeasonRank : Api.pgcRank,
          mixinKey: mixinKey,
        ),
        season: season,
        day: day,
        seasonType: seasonType,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取 PGC 排行出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 基线这几个参数的类型是 `dynamic`，实际传进来的有 `int` 也有
  /// `String`。统一折成 `int?`，折不出来按空处理——可选参数为空就是
  /// 「不传」，与基线 `'key': ?value` 的语义一致。
  static int? _asInt(Object? value) => switch (value) {
    int v => v,
    String v => int.tryParse(v),
    num v => v.toInt(),
    _ => null,
  };
}
