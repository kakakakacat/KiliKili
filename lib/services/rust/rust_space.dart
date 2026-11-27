import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/rust/api/space.dart' as rust_api;
import 'package:kilikili/services/rust/rust_grpc.dart';
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/space.dart'
    show
        CoinLikeArcItem,
        CoinLikeArcPage,
        GuardItem,
        MemberCard,
        MemberGuardPage,
        MemberInfo,
        MemberRelationStat,
        MemberUpStat,
        SpaceArchiveHistory,
        SpaceArchiveItem,
        SpaceArchiveKind,
        SpaceArchiveOrder,
        SpaceArchivePage,
        SpaceArchiveSearchOrder,
        SpaceArchiveSearchPage,
        SpaceArchiveSort,
        SpaceArchiveTag,
        spaceArticleIsEnd,
        spaceAudioIsEnd,
        SpaceArticlePage,
        SpaceAudioPage,
        SpaceCheesePage,
        SpaceCheeseItem,
        SpaceShopPage,
        SpaceShopItem,
        SpaceOpusFlowPage,
        SpaceOpusFlowItem,
        SpaceOpusCover,
        SpaceOpusCollectionItem,
        SpaceOpusCollectionSection,
        SpaceArticleBlock,
        SpaceArticleItem,
        SpaceAudioBlock,
        SpaceAudioItem,
        SpaceCard,
        SpaceCardTag,
        SpaceElec,
        SpaceEpisodicButton,
        SpaceFavBlock,
        SpaceFavItem,
        SpaceFollowedBy,
        SpaceGuard,
        SpaceImages,
        SpaceLive,
        SpaceLiveMedal,
        SpacePage,
        SpacePrInfo,
        SpaceRelation,
        SpaceReservation,
        SpaceReservationLottery,
        SpaceSearchArchiveItem,
        SpaceSearchArchivePage,
        SpaceSeasonSeries,
        SpaceSeasonSeriesPage,
        SpaceSeasonWebKind,
        SpaceSeasonWebPage,
        SpaceSetting,
        SpaceSimpleUser,
        SpaceTab2,
        SpaceTab2Item,
        SpaceTabFilter,
        SpaceTopImage,
        SpaceTopTitle,
        SpaceVideoBlock,
        SpaceWebBadge,
        SpaceWebVideo,
        UpowerLevel,
        UpowerRank,
        UpowerRankItem;

/// 用户空间那一族的取数路径（S6 / issue #56）：别人的资料卡、关注关系
/// 计数、播放与获赞总数、用户名片、举报，以及合集与列表、投币/点赞过的
/// 稿件、大航海名单、充电排行。
///
/// ## 五个不同的域
///
/// 这一族横跨五个主机：多数在 `api.bilibili.com`，举报在
/// `space.bilibili.com`，投币/点赞列表在 `app.bilibili.com`，大航海名单在
/// `live.bilibili.com`，会员购小店在 `mall.bilibili.com`。base_url 由各方法按条传。
///
/// 基线对应的 `MemberHttp` 函数随本次迁移逐族删除，按 ADR-011 决策四
/// 不留降级开关。
///
/// ## 账号
///
/// 多数接口在 `ApiType.apiTypeSet` 里**没有登记**，与基线拦截器一样走
/// `main`。两个例外必须照抄，否则换成另一个账号发出去，服务端给的是
/// 另一份结果：
///
/// - 投稿搜索（`Api.searchArchive`）登记在 `recommend` 下——注释写着
///   「不同账号搜索结果可能不一样」；
/// - 合集 / 列表（`Api.seasonArchives` / `Api.seriesArchives`）登记在
///   `heartbeat` 下。
abstract final class RustSpace {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// 空间页专用的请求头。
  ///
  /// 基线对资料卡那条显式设置了 origin / referer / 浏览器 UA
  /// （`lib/http/member.dart`）。这里照抄，并且**放在账号头之后**，
  /// 确保覆盖拦截器补的那条 referer——同 `rust_search.dart` 的做法。
  ///
  /// referer 带上具体 mid：服务端把它当「从谁的空间页点进来的」，
  /// 换成空间站根路径与基线不是同一个请求。
  static List<(String, String)> _spaceHeaders(Account account, int mid) => [
    ...RustRequest.headersFor(
      account,
      referer: '${HttpString.spaceBaseUrl}/$mid/dynamic',
      userAgent: BrowserUa.pc,
    ),
    ('origin', HttpString.spaceBaseUrl),
  ];

  /// [write] 为 true 时才带 csrf（ADR-013 决策四）。
  ///
  /// [baseUrl] 只有举报那条要换——它挂在 `space.bilibili.com`。
  static Future<rust_api.SpaceContext> _context({
    required String path,
    String? baseUrl,
    String mixinKey = '',
    List<(String, String)>? headers,
    bool write = false,
    AccountType accountType = AccountType.main,
  }) async {
    final acc = Accounts.get(accountType);
    return rust_api.SpaceContext(
      baseUrl: baseUrl ?? HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(acc, path),
      csrf: write ? RustRequest.csrfFor(acc) : null,
      headers: headers ?? RustRequest.headersFor(acc),
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
      mixinKey: mixinKey,
      ts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
    );
  }

  /// 取 WBI mixin key。拿不到就别发请求——基线会用空串继续签名并换回一个
  /// 业务错误码，那只是把失败推后，还更难诊断。
  static Future<String?> _mixinKey() async {
    final key = await WbiSign.getWbiKeys();
    return key.isEmpty ? null : key;
  }

  /// 别人的资料。WBI 签名 + 浏览器 UA。
  static Future<LoadingState<rust_api.MemberInfo>> memberInfo({
    required int mid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchMemberInfo(
        context: await _context(
          path: Api.memberInfo,
          mixinKey: mixinKey,
          headers: _spaceHeaders(Accounts.get(AccountType.main), mid),
        ),
        mid: mid,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('用户资料', e, st);
    }
  }

  /// 关注关系计数（关注数 / 粉丝数）。
  static Future<LoadingState<rust_api.MemberRelationStat>> memberRelationStat({
    required int mid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchMemberRelationStat(
        context: await _context(path: Api.userStat),
        mid: mid,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('关注计数', e, st);
    }
  }

  /// 投稿播放与获赞总数。需要登录，调用方先判 `isLogin`。
  static Future<LoadingState<rust_api.MemberUpStat>> memberUpStat({
    required int mid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchMemberUpStat(
        context: await _context(path: Api.getMemberViewApi),
        mid: mid,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('播放与获赞', e, st);
    }
  }

  /// 用户名片。只想要一个昵称时用这条——不需要签名，也不需要浏览器 UA。
  static Future<LoadingState<rust_api.MemberCard>> memberCard({
    required int mid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchMemberCard(
        context: await _context(path: Api.memberCardInfo),
        mid: mid,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('用户名片', e, st);
    }
  }

  /// 一页「合集和列表」。
  ///
  /// 合集与列表分两个数组回来，拼成一条列表是界面的事——它们跳转到的是
  /// 不同的页面，区分靠 `seasonId` / `seriesId` 哪个有值。
  static Future<LoadingState<rust_api.SpaceSeasonSeriesPage>> seasonSeries({
    required int mid,
    required int page,
    int pageSize = 10,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSeasonSeries(
        context: await _context(path: Api.seasonSeries),
        mid: mid,
        page: page,
        pageSize: pageSize,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('合集和列表', e, st);
    }
  }

  /// 一页投币过/点赞过的稿件。[coin] 为真取投币列表。
  ///
  /// 两条接口的参数与响应形状完全一样，只换路径，所以共用一个方法。
  static Future<LoadingState<rust_api.CoinLikeArcPage>> coinLikeArc({
    required bool coin,
    required int mid,
    required int page,
    int pageSize = 20,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchCoinLikeArc(
        context: await _context(
          // 这两条挂在 app 域上。
          path: coin ? Api.coinArc : Api.likeArc,
          baseUrl: HttpString.appBaseUrl,
        ),
        coin: coin,
        mid: mid,
        page: page,
        pageSize: pageSize,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed(coin ? '投币过的视频' : '点赞过的视频', e, st);
    }
  }

  /// 一页大航海名单。
  static Future<LoadingState<rust_api.MemberGuardPage>> memberGuard({
    required int ruid,
    required int page,
    int pageSize = 20,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchMemberGuard(
        context: await _context(
          // 这条挂在直播域上。
          path: Api.memberGuard,
          baseUrl: HttpString.liveBaseUrl,
        ),
        ruid: ruid,
        page: page,
        pageSize: pageSize,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('大航海名单', e, st);
    }
  }

  /// 充电排行。[privilegeType] 省掉表示「全部档位」，此时响应里才带各档位
  /// 的名字与人数。
  static Future<LoadingState<rust_api.UpowerRank>> upowerRank({
    required int upMid,
    required int page,
    int pageSize = 100,
    int? privilegeType,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchUpowerRank(
        context: await _context(path: Api.upowerRank),
        upMid: upMid,
        page: page,
        pageSize: pageSize,
        privilegeType: privilegeType,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('充电排行', e, st);
    }
  }

  /// 举报用户。
  ///
  /// 返回的是**服务端是否受理**：这条接口不走标准信封，受理失败时 HTTP
  /// 仍是 200，响应里只有一个 `"status": false`。所以「请求失败」与
  /// 「没受理」是两种结果，由 `Error` 与 `Success(false)` 分别表达。
  static Future<LoadingState<bool>> reportMember({
    required int mid,
    required String reason,
    int? reasonV2,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.reportMember(
        context: await _context(
          path: Api.reportMember,
          baseUrl: HttpString.spaceBaseUrl,
          write: true,
        ),
        mid: mid,
        reason: reason,
        reasonV2: reasonV2,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('举报', e, st);
    }
  }

  /// 空间主页。
  ///
  /// [fromViewAid] 是从视频页点进来时带的稿件 id，服务端据此把那条稿件
  /// 排到投稿列表前面。
  static Future<LoadingState<rust_api.SpacePage>> spacePage({
    required int mid,
    String? fromViewAid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSpacePage(
        context: await _context(
          // 这条挂在 app 域上。
          path: Api.space,
          baseUrl: HttpString.appBaseUrl,
        ),
        mid: mid,
        fromViewAid: fromViewAid,
        appkey: Constants.appKey,
        appsec: Constants.appSec,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('空间主页', e, st);
    }
  }

  /// 一页投稿。六个 tab（视频 / 充电专属 / 合集 / 列表 / 追番 / 漫画）
  /// 共用这一个方法，差别由 [kind] 决定。
  ///
  /// 省掉的参数**不会发出去**，不是发一个空值——这一点有 Rust 侧的测试
  /// 钉着。
  static Future<LoadingState<rust_api.SpaceArchivePage>> spaceArchive({
    required rust_api.SpaceArchiveKind kind,
    int? mid,
    String? aid,
    rust_api.SpaceArchiveOrder? order,
    rust_api.SpaceArchiveSort? sort,
    int? page,
    int? next,
    int? seasonId,
    int? seriesId,
    bool? includeCursor,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSpaceArchive(
        context: await _context(
          path: Api.spaceArchive,
          baseUrl: HttpString.appBaseUrl,
        ),
        kind: kind,
        mid: mid,
        aid: aid,
        order: order,
        sort: sort,
        page: page,
        next: next,
        seasonId: seasonId,
        seriesId: seriesId,
        includeCursor: includeCursor,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('投稿列表', e, st);
    }
  }

  /// 一页投稿搜索（网页端）。WBI 签名 + 浏览器 UA。
  ///
  /// [specialType]（如 `charging`）与 [tid] 二选一：标签条上点到专题时发
  /// 前者，点到分区时发后者。
  static Future<LoadingState<rust_api.SpaceArchiveSearchPage>> archiveSearch({
    required int mid,
    int tid = 0,
    required int page,
    int pageSize = 30,
    String? keyword,
    String? specialType,
    rust_api.SpaceArchiveSearchOrder order =
        rust_api.SpaceArchiveSearchOrder.pubdate,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchSpaceArchiveSearch(
        context: await _context(
          path: Api.searchArchive,
          mixinKey: mixinKey,
          // 这条登记在 `recommend` 下，不是 `main`。
          accountType: AccountType.recommend,
          headers: _spaceHeaders(Accounts.get(AccountType.recommend), mid),
        ),
        mid: mid,
        tid: tid,
        page: page,
        pageSize: pageSize,
        keyword: keyword,
        specialType: specialType,
        order: order,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('投稿搜索', e, st);
    }
  }

  /// 一页合集或列表（网页端）。
  ///
  /// 两条接口的参数名不同，由 Rust 按 [kind] 分开拼；这里只表达「正序还是
  /// 倒序」。
  static Future<LoadingState<rust_api.SpaceSeasonWebPage>> seasonWeb({
    required rust_api.SpaceSeasonWebKind kind,
    required int mid,
    required int id,
    required int page,
    int pageSize = 30,
    bool ascending = false,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSpaceSeasonWeb(
        context: await _context(
          path: kind == rust_api.SpaceSeasonWebKind.season
              ? Api.seasonArchives
              : Api.seriesArchives,
          // 这两条登记在 `heartbeat` 下，不是 `main`。
          accountType: AccountType.heartbeat,
          headers: _spaceHeaders(Accounts.get(AccountType.heartbeat), mid),
        ),
        kind: kind,
        mid: mid,
        id: id,
        page: page,
        pageSize: pageSize,
        ascending: ascending,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('合集视频', e, st);
    }
  }

  /// 一页 gRPC 投稿搜索。
  ///
  /// 这条走 gRPC 网关，上下文与弹幕那几条共用（`RustGrpc.context()`）：
  /// 不带 cookie，身份在 metadata 头里。
  static Future<LoadingState<rust_api.SpaceSearchArchivePage>>
  searchArchiveGrpc({
    required int mid,
    required String keyword,
    required int page,
    int pageSize = 20,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSpaceSearchArchive(
        context: RustGrpc.context(),
        mid: mid,
        keyword: keyword,
        page: page,
        pageSize: pageSize,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('投稿搜索', e, st);
    }
  }

  static Future<LoadingState<rust_api.SpaceArticlePage>> article({
    required int mid,
    required int page,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final r = await rust_api.fetchSpaceArticle(
        context: await _context(
          path: Api.spaceArticle,
          baseUrl: HttpString.appBaseUrl,
          headers: [
            ...RustRequest.headersFor(
              Accounts.main,
              userAgent: Constants.userAgentApp,
            ),
            ('bili-http-engine', 'cronet'),
          ],
        ),
        mid: mid,
        page: page,
        pageSize: 10,
      );
      return _unwrap(r.data, r.errorDetail, r.errorCode);
    } catch (e, st) {
      return _crashed('Article', e, st);
    }
  }

  static Future<LoadingState<rust_api.SpaceAudioPage>> audio({
    required int mid,
    required int page,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final r = await rust_api.fetchSpaceAudio(
        context: await _context(path: Api.spaceAudio),
        mid: mid,
        page: page,
        pageSize: 20,
      );
      return _unwrap(r.data, r.errorDetail, r.errorCode);
    } catch (e, st) {
      return _crashed('Audio', e, st);
    }
  }

  static Future<LoadingState<rust_api.SpaceCheesePage>> cheese({
    required int mid,
    required int page,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final r = await rust_api.fetchSpaceCheese(
        context: await _context(path: Api.spaceCheese),
        mid: mid,
        page: page,
        pageSize: 30,
      );
      return _unwrap(r.data, r.errorDetail, r.errorCode);
    } catch (e, st) {
      return _crashed('Cheese', e, st);
    }
  }

  static Future<LoadingState<rust_api.SpaceShopPage>> shop({
    required int mid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final r = await rust_api.fetchSpaceShop(
        context: await _context(
          path: Api.spaceShop,
          baseUrl: HttpString.mallBaseUrl,
        ),
        mid: mid,
        pageSize: 8,
        accessKey: Accounts.main.accessKey,
        appkey: Constants.appKey,
        appsec: Constants.appSec,
      );
      return _unwrap(r.data, r.errorDetail, r.errorCode);
    } catch (e, st) {
      return _crashed('会员购', e, st);
    }
  }

  static Future<LoadingState<rust_api.SpaceOpusFlowPage>> opusFlow({
    required int hostMid,
    String? next,
    required String filterType,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final r = await rust_api.fetchSpaceOpusFlow(
        context: RustGrpc.context(),
        hostMid: hostMid,
        next: next,
        filterType: filterType,
        pageSize: 20,
      );
      return _unwrap(r.data, r.errorDetail, r.errorCode);
    } catch (e, st) {
      return _crashed('图文', e, st);
    }
  }

  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '请求失败', code: code) : Success(data);

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what 出错: $e\n$st');
    return Error('$e');
  }
}
