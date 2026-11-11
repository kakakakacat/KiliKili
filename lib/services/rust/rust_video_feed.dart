import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/models/model_hot_video_item.dart';
import 'package:kilikili/rust/api/video_feed.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/global_data.dart';
import 'package:kilikili/utils/recommend_filter.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;

export 'package:kilikili/rust/api/video_feed.dart'
    show
        FeedVideoItem,
        PopularPrecious,
        PopularSeriesConfig,
        PopularSeriesListItem,
        PopularSeriesOne;

/// 视频流五条只读接口的取数路径：请求由 Rust 发出、Rust 解析并过滤
/// （S3 / issue #53）。
///
/// 热门、排行、相关推荐、每周必看、入站必刷。基线对应的
/// `VideoHttp.hotVideoList` 等六个函数已随本次迁移删除。
///
/// ## 没有降级开关
///
/// typed 直连删掉了 Dart 原实现，降级无处可去（ADR-011 决策四，仓库所有者
/// 2026-11-11 确认）。`RustCore` 未就绪时这几个列表整体不可用，表现为一条
/// 明确的错误而不是崩溃。
///
/// ## 过滤规则
///
/// 规则本身在 Rust（`bili_domain::video_feed`），取值每次从这里传入：
/// 用户随时能在设置页改，缓存一份必然漂移。哪几条接口过滤、哪几条不过滤
/// 严格照基线，见 Rust 侧模块文档的那张表。
///
/// 屏蔽词正则在两端语义不同（如 `\d`）或 Rust 编不过（如 lookahead）时，
/// 整条请求失败且 `errorKind == 'unsupported'`——忽略规则意味着用户明确
/// 屏蔽过的内容照常出现且没有提示，那比空列表更糟。
abstract final class RustVideoFeed {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该列表暂不可用';

  /// 每周必看与入站必刷在基线里额外带的桌面端请求头：不带会被风控拦。
  static List<(String, String)> _webHeaders(String referer) => [
    ('user-agent', BrowserUa.pc),
    ('origin', HttpString.baseUrl),
    ('referer', referer),
  ];

  static rust_api.FeedFilterSettings _filter() => rust_api.FeedFilterSettings(
    blockedMids: Int64List.fromList(GlobalData().blackMids.toList()),
    // 读 RecommendFilter 的静态字段而不是 Pref：设置页改完会就地更新它们，
    // Pref 要等下次启动才一致。
    banWordRcmd: RecommendFilter.enableFilter
        ? RecommendFilter.rcmdRegExp.pattern
        : '',
    banWordZone: RecommendFilter.enableZoneFilter
        ? RecommendFilter.zoneRegExp.pattern
        : '',
    minDuration: RecommendFilter.minDurationForRcmd,
    minPlay: RecommendFilter.minPlayForRcmd,
    minLikeRatio: RecommendFilter.minLikeRatioForRecommend,
    applyToRelated: RecommendFilter.applyFilterToRelatedVideos,
  );

  /// 组装一次请求的上下文。
  ///
  /// `path` 决定用哪个账号——必须与 `ApiType.apiTypeSet` 的归类一致，否则
  /// 两条路径用的是不同账号。这六条接口都归 `AccountType.recommend`。
  ///
  /// `mixinKey` 为空串表示这条接口不签名（热门、相关推荐）。
  static Future<rust_api.FeedContext> _context({
    required String path,
    required String mixinKey,
    List<(String, String)>? extraHeaders,
  }) async {
    final account = Accounts.get(AccountType.recommend);
    return rust_api.FeedContext(
      baseUrl: HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(account, path),
      headers: [
        ...RustRequest.headersFor(account),
        ...?extraHeaders,
      ],
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
      mixinKey: mixinKey,
      wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      filter: _filter(),
    );
  }

  /// 取 WBI mixin key。拿不到就别发请求：基线会用空串继续签名并换回一个
  /// 业务错误码，那只是把失败推后，还更难诊断。
  static Future<String?> _mixinKey() async {
    final key = await WbiSign.getWbiKeys();
    return key.isEmpty ? null : key;
  }

  /// 热门列表。
  static Future<LoadingState<List<HotVideoItemModel>>> hot({
    required int pn,
    required int ps,
  }) => _list(
    what: '热门视频',
    run: () async => rust_api.fetchHotVideos(
      context: await _context(path: Api.hotList, mixinKey: ''),
      pn: pn,
      ps: ps,
    ),
  );

  /// 分区排行。`rid` 为分区 id，0 是全站。
  static Future<LoadingState<List<HotVideoItemModel>>> rank(int rid) async {
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    return _list(
      what: '排行榜',
      run: () async => rust_api.fetchRankVideos(
        context: await _context(path: Api.getRankApi, mixinKey: mixinKey),
        rid: rid,
      ),
    );
  }

  /// 相关推荐。
  static Future<LoadingState<List<HotVideoItemModel>?>> related({
    required String bvid,
  }) => _list(
    what: '相关视频',
    run: () async => rust_api.fetchRelatedVideos(
      context: await _context(path: Api.relatedList, mixinKey: ''),
      bvid: bvid,
    ),
  );

  /// 每周必看的期号列表。
  static Future<LoadingState<List<rust_api.PopularSeriesListItem>?>>
  seriesList() async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchPopularSeriesList(
        context: await _context(
          path: Api.popularSeriesList,
          mixinKey: mixinKey,
          extraHeaders: _webHeaders(
            'https://www.bilibili.com/v/popular/weekly',
          ),
        ),
      );
      final data = result.data;
      if (data == null) return Error(_message(result.errorDetail));
      return Success(data);
    } catch (e, st) {
      return _crashed('每周必看期号列表', e, st);
    }
  }

  /// 每周必看的某一期。
  static Future<LoadingState<rust_api.PopularSeriesOne>> seriesOne({
    required int number,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchPopularSeriesOne(
        context: await _context(
          path: Api.popularSeriesOne,
          mixinKey: mixinKey,
          extraHeaders: _webHeaders(
            'https://www.bilibili.com/v/popular/weekly?num=$number',
          ),
        ),
        number: number,
      );
      final data = result.data;
      if (data == null) return Error(_message(result.errorDetail));
      return Success(data);
    } catch (e, st) {
      return _crashed('每周必看', e, st);
    }
  }

  /// 入站必刷。
  static Future<LoadingState<rust_api.PopularPrecious>> precious({
    required int page,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchPopularPrecious(
        context: await _context(
          path: Api.popularPrecious,
          mixinKey: mixinKey,
          extraHeaders: _webHeaders(
            'https://www.bilibili.com/v/popular/history',
          ),
        ),
        page: page,
      );
      final data = result.data;
      if (data == null) return Error(_message(result.errorDetail));
      return Success(data);
    } catch (e, st) {
      return _crashed('入站必刷', e, st);
    }
  }

  /// 三条列表接口共用的壳：就绪判断、异常兜底、结果转换。
  ///
  /// 返回 `LoadingState<List<HotVideoItemModel>>`，相关推荐那条声明的是
  /// 可空列表——Dart 的泛型是协变的，直接可用。
  static Future<LoadingState<List<HotVideoItemModel>>> _list({
    required String what,
    required Future<rust_api.FeedListResult> Function() run,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await run();
      final data = result.data;
      if (data == null) return Error(_message(result.errorDetail));
      return Success(data.map(HotVideoItemModel.fromRust).toList());
    } catch (e, st) {
      return _crashed(what, e, st);
    }
  }

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] 取$what出错: $e\n$st');
    return Error('$e');
  }

  /// Rust 侧的 `errorDetail` 已经是可直接展示的文案；兜底一句免得空白。
  static String _message(String? detail) => detail ?? '请求失败';
}
