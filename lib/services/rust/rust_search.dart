import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/models/common/search/search_type.dart';
import 'package:kilikili/models/search/result.dart';
import 'package:kilikili/models_new/search/search_rcmd/data.dart';
import 'package:kilikili/models_new/search/search_trending/data.dart';
import 'package:kilikili/rust/api/search.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust/search_compat.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/global_data.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/search.dart'
    show
        OfficialVerify,
        SearchActivityItem,
        SearchArticleItem,
        SearchLiveItem,
        SearchOwner,
        SearchPgcItem,
        SearchStat,
        SearchTrendingItem,
        SearchUserCard,
        SearchUserItem,
        SearchVideoItem,
        TitleSpan;

/// 搜索的取数路径（S9 / issue #59）。**typed 直连**，照 ADR-011 的样板写。
///
/// ## 没有降级开关
///
/// 与 `RustVideoDetail` 那种「失败回落 Dart 原实现」不同：typed 直连
/// **删掉了 Dart 原实现**，`lib/http/search.dart` 里的 `searchByType` /
/// `searchTrending` / `searchRecommend` 已随本次迁移删除，降级无处可去。
///
/// 代价写在明处：`RustCore` 未就绪（原生库没打进产物）时搜索整体不可用，
/// 表现为「搜不到结果」而不是崩溃——各入口统一返回
/// `error_kind: storage` 的失败。这是 ADR-011 决策四的取舍。
///
/// ## 凭据与账号
///
/// Cookie 与账号头按请求从 Dart 的账号体系取出传给 Rust，Rust 不保存。
/// 账号必须取 `AccountType.recommend`：`lib/utils/accounts/api_type.dart`
/// 把四个搜索接口都绑在这个账号上，注释写着「不同账号搜索结果可能不一样」。
/// 取错账号不会报错，只会悄悄换一批结果。
abstract final class RustSearch {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，搜索不可用';

  /// 搜索接口统一走 recommend 账号，见类文档。
  static Account get _account => Accounts.get(AccountType.recommend);

  /// 热搜不签名，搜索发现走 App 签名，两者的 `mixinKey` 都留空。
  static Future<rust_api.SearchContext> _context({
    required String path,
    String mixinKey = '',
    bool app = false,
    List<(String, String)> extraHeaders = const [],
  }) async {
    final account = _account;
    return rust_api.SearchContext(
      baseUrl: app ? HttpString.appBaseUrl : HttpString.apiBaseUrl,
      // App 请求沿用原拦截器的 access_key 身份，不混入 Web Cookie。
      cookie: app ? null : await RustRequest.cookieFor(account, path),
      headers: [
        ...RustRequest.headersFor(account),
        ...extraHeaders,
      ],
      mixinKey: mixinKey,
      wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
    );
  }

  /// 搜索结果页特有的请求头。
  ///
  /// 基线显式设置了 origin / referer / 浏览器 UA（`lib/http/search.dart`）。
  /// 这三条是风控的输入，少一条就可能被判成非浏览器请求——因此原样搬过来，
  /// 并且**放在账号头之后**，确保覆盖拦截器补的那条 referer。
  static List<(String, String)> _searchHeaders(
    SearchType searchType,
    String keyword,
  ) => [
    ('origin', 'https://search.bilibili.com'),
    (
      'referer',
      'https://search.bilibili.com/${searchType.name}'
          '?keyword=${Uri.encodeFull(keyword)}',
    ),
    ('user-agent', BrowserUa.pc),
  ];

  /// 分类型搜索。`R` 由 `searchType` 决定，与基线
  /// `SearchHttp.searchByType` 的契约一致。
  ///
  /// 风控拦截（服务端下发 `v_voucher`）时返回
  /// `Error('触发风控，需要完成验证后重试')`——基线在这里也是直接判失败，
  /// 其后的验证流程是注释掉的。
  static Future<LoadingState<R>> searchByType<R extends SearchNumData>({
    required SearchType searchType,
    required String keyword,
    required int page,
    String? order,
    int? duration,
    int? tids,
    int? orderSort,
    int? userType,
    int? categoryId,
    int? pubBegin,
    int? pubEnd,
    String? gaiaVtoken,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);

    try {
      final mixinKey = await WbiSign.getWbiKeys();
      if (mixinKey.isEmpty) {
        // 基线会用空串继续签名，拿回一个业务错误码。那只是把「取 key 失败」
        // 伪装成「服务端拒绝」，这里直接说实话。
        return const Error('WBI 密钥获取失败，请稍后重试');
      }

      final context = await _context(
        path: searchType == SearchType.all ? Api.searchAll : Api.searchByType,
        mixinKey: mixinKey,
        extraHeaders: [
          ..._searchHeaders(searchType, keyword),
          // 基线把风控票据同时放进查询参数与 Cookie。
          if (gaiaVtoken != null) ('cookie', 'x-bili-gaia-vtoken=$gaiaVtoken'),
        ],
      );

      final query = rust_api.SearchQuery(
        searchType: searchType.name,
        keyword: keyword,
        page: page,
        order: order,
        duration: duration,
        tids: tids,
        orderSort: orderSort,
        userType: userType,
        categoryId: categoryId,
        pubBegin: pubBegin,
        pubEnd: pubEnd,
        gaiaVtoken: gaiaVtoken,
      );

      return await switch (searchType) {
        SearchType.all => _all(context, query) as Future<LoadingState<R>>,
        SearchType.video => _video(context, query) as Future<LoadingState<R>>,
        SearchType.media_bangumi ||
        SearchType.media_ft => _pgc(context, query) as Future<LoadingState<R>>,
        SearchType.live_room =>
          _live(context, query) as Future<LoadingState<R>>,
        SearchType.bili_user =>
          _user(context, query) as Future<LoadingState<R>>,
        SearchType.article =>
          _article(context, query) as Future<LoadingState<R>>,
      };
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 搜索出错: $e\n$st');
      return Error('$e');
    }
  }

  static Future<LoadingState<SearchVideoData>> _all(
    rust_api.SearchContext context,
    rust_api.SearchQuery query,
  ) async {
    final res = await rust_api.searchAll(context: context, query: query);
    final data = res.data;
    if (data == null) return _failure(res.errorDetail, res.errorCode);

    return Success(
      SearchVideoData(
        numResults: res.numResults,
        // 黑名单过滤留在 Dart：它来自本地设置（GlobalData().blackMids），
        // 不是协议的一部分，没有理由跨 FFI 传一份过去。
        list: data.videos.notBlocked((e) => e.owner.mid).toItemModels(),
        searchUser: data.users.notBlocked((e) => e.mid).toList(),
        searchMedia: data.medias,
        // 「只显示直播活动」是界面决策（App 只能打开那种页面），所以过滤
        // 在这里而不在 Rust。
        searchActivity: data.activities
            .where((e) => e.url?.startsWith(HttpString.liveUrl) ?? false)
            .toList(),
        searchEsports: data.esports,
      ),
    );
  }

  static Future<LoadingState<SearchVideoData>> _video(
    rust_api.SearchContext context,
    rust_api.SearchQuery query,
  ) async {
    final res = await rust_api.searchVideo(context: context, query: query);
    final data = res.data;
    if (data == null) return _failure(res.errorDetail, res.errorCode);
    return Success(
      SearchVideoData(
        numResults: res.numResults,
        list: data.notBlocked((e) => e.owner.mid).toItemModels(),
      ),
    );
  }

  static Future<LoadingState<SearchUserData>> _user(
    rust_api.SearchContext context,
    rust_api.SearchQuery query,
  ) async {
    final res = await rust_api.searchUser(context: context, query: query);
    final data = res.data;
    if (data == null) return _failure(res.errorDetail, res.errorCode);
    return Success(
      SearchUserData(
        numResults: res.numResults,
        list: data.notBlocked((e) => e.mid).toList(),
      ),
    );
  }

  static Future<LoadingState<SearchLiveData>> _live(
    rust_api.SearchContext context,
    rust_api.SearchQuery query,
  ) async {
    final res = await rust_api.searchLive(context: context, query: query);
    final data = res.data;
    if (data == null) return _failure(res.errorDetail, res.errorCode);
    return Success(
      SearchLiveData(
        numResults: res.numResults,
        // 基线这里按 uid 过滤，不是 mid。
        list: data.notBlocked((e) => e.uid).toList(),
      ),
    );
  }

  static Future<LoadingState<SearchPgcData>> _pgc(
    rust_api.SearchContext context,
    rust_api.SearchQuery query,
  ) async {
    final res = await rust_api.searchPgc(context: context, query: query);
    final data = res.data;
    if (data == null) return _failure(res.errorDetail, res.errorCode);
    // 基线对番剧结果不做黑名单过滤（番剧没有 UP 主 mid）。
    return Success(SearchPgcData(numResults: res.numResults, list: data));
  }

  static Future<LoadingState<SearchArticleData>> _article(
    rust_api.SearchContext context,
    rust_api.SearchQuery query,
  ) async {
    final res = await rust_api.searchArticle(context: context, query: query);
    final data = res.data;
    if (data == null) return _failure(res.errorDetail, res.errorCode);
    return Success(
      SearchArticleData(
        numResults: res.numResults,
        list: data.notBlocked((e) => e.mid).toList(),
      ),
    );
  }

  /// 热搜榜。`needsTop` 为真时置顶项已拼在列表前面，条数由 `numResults`
  /// 带回。
  static Future<LoadingState<SearchTrendingData>> searchTrending({
    int limit = 30,
    bool needsTop = false,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final context = await _context(path: Api.searchTrending);
      final res = await rust_api.searchTrending(
        context: context,
        limit: limit,
        needsTop: needsTop,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(
        SearchTrendingData(list: data, topCount: res.numResults ?? 0),
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取热搜出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 搜索页默认词。
  static Future<LoadingState<SearchRcmdData>> searchRecommend() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final context = await _context(path: Api.searchRecommend, app: true);
      final res = await rust_api.searchRecommend(
        context: context,
        appkey: Constants.appKey,
        appsec: Constants.appSec,
        accessKey: _account.accessKey,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(SearchRcmdData(list: data));
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取默认词出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 失败一律转成 `Error`，不抛异常——调用点照常走失败分支。
  ///
  /// `Error` 是 `LoadingState<Never>`，能赋给任何 `LoadingState<T>`，
  /// 所以这里不需要（也不能）带类型参数。
  static Error _failure(String? detail, int? code) =>
      Error(detail ?? '搜索失败', code: code);
}

/// 本地黑名单过滤。
///
/// 留在 Dart 而不是传进 Rust：黑名单是本地设置（`GlobalData().blackMids`，
/// 由 `lib/pages/blacklist/` 维护），不是协议的一部分。跨 FFI 传一份过去
/// 等于让 Rust 多知道一件与协议无关的事，还得处理它什么时候失效。
extension _NotBlocked<E> on List<E> {
  Iterable<E> notBlocked(Object? Function(E element) mid) =>
      where((e) => !GlobalData().blackMids.contains(mid(e)));
}
