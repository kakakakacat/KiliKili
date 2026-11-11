import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/live.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/live.dart'
    show
        LiveArea,
        LiveAreaGroup,
        LiveFeedback,
        LiveFeedbackReason,
        LiveFollowItem,
        LiveFollowSection,
        LiveFollowingPage,
        LiveIndexPage,
        LiveRoomCard,
        LiveRoomH5,
        LiveSecondPage,
        LiveSecondTag;

/// 直播只读接口的取数路径（S11a 第一批 / issue #61）。**typed 直连**。
///
/// ## 覆盖范围
///
/// 全部分区、某一级分区下的二级分区、收藏分区、首页推荐、分区二级列表、
/// 关注的直播、H5 房间信息。
///
/// **不在这里**：`liveRoomInfo`（取流）仍走 `lib/http/live.dart`，它要
/// 产出共享的 `LiveQuality`，留到第二批；弹幕长连接归 S11b（#62）；
/// 全部写操作归 ADR-013（#50）。
///
/// ## 没有降级开关
///
/// typed 直连删掉了 Dart 原实现（ADR-011 决策四已定为 A）。`RustCore`
/// 未就绪时这些页面整体不可用，表现为「取不到数据」而不是崩溃。
///
/// ## 两个与别处不同的地方
///
/// 1. **主机不是 `api.bilibili.com`**。直播接口挂在
///    `HttpString.liveBaseUrl`（`api.live.bilibili.com`），`Api.live*`
///    常量因此是完整 URL 而非相对路径——账号与 Cookie 查表用那个完整
///    常量，传给 Rust 的则是 base_url 与路径两段。
/// 2. **有 APP 签名**。分区三条与两条 feed 接口要带 `appkey`/`ts`/`sign`，
///    算法在 Rust 侧；本文件只负责把身份值（appkey、UA、buvid、
///    access_key…）递过去——它们的真相源在 Dart，还有若干未迁接口在用。
abstract final class RustLive {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能不可用';

  /// 基线各调用点写死的 app 版本号。与 [Constants.userAgentApp] 和
  /// [Constants.statisticsApp] 里的 `8.43.0` 是同一套，改一处要一起改。
  static const int _build = 8430300;
  static const String _version = '8.43.0';

  /// 组装一次请求的公共参数。
  ///
  /// [endpoint] 用 `Api.live*` 那个完整 URL：账号归属与 Cookie 都按它查。
  static Future<rust_api.LiveContext> _context(String endpoint) async {
    final account = RustRequest.accountFor(endpoint);
    return rust_api.LiveContext(
      baseUrl: HttpString.liveBaseUrl,
      cookie: await RustRequest.cookieFor(account, endpoint),
      headers: RustRequest.headersFor(account),
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
    );
  }

  /// 身份值。按接口所属的账号取 `access_key` 与 `buvid`——两者必须来自
  /// 同一个账号，混用会让服务端看到一个对不上的组合。
  static rust_api.LiveAppIdentity _identity(Account account) =>
      rust_api.LiveAppIdentity(
        appkey: Constants.appKey,
        appsec: Constants.appSec,
        build: _build,
        version: _version,
        statistics: Constants.statisticsApp,
        userAgent: Constants.userAgentApp,
        buvid: account.buvid,
        traceId: Constants.traceId,
        accessKey: account.accessKey,
        loggedIn: account.isLogin,
        ts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      );

  /// 失败一律转成 `Error`，不抛异常。
  ///
  /// `Error` 是 `LoadingState<Never>`，能赋给任何 `LoadingState<T>`。
  static Error _failure(String? detail, int? code) =>
      Error(detail ?? '请求失败', code: code);

  /// 取全部分区（按一级分区分组）。
  static Future<LoadingState<List<rust_api.LiveAreaGroup>?>> areaList() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchLiveAreaList(
        context: await _context(Api.liveAreaList),
        identity: _identity(RustRequest.accountFor(Api.liveAreaList)),
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取直播分区列表出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取收藏的分区。基线在未收藏时返回空列表而非 null，这里照搬。
  static Future<LoadingState<List<rust_api.LiveArea>>> favTags() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchLiveFavTags(
        context: await _context(Api.getLiveFavTag),
        identity: _identity(RustRequest.accountFor(Api.getLiveFavTag)),
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取收藏分区出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取某一级分区下的二级分区。
  static Future<LoadingState<List<rust_api.LiveArea>?>> roomAreaList({
    required int parentId,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchLiveRoomAreaList(
        context: await _context(Api.liveRoomAreaList),
        identity: _identity(RustRequest.accountFor(Api.liveRoomAreaList)),
        parentId: parentId,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取二级分区出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取首页推荐的一屏。
  ///
  /// `moduleSelect` 只在刷新顶部区块（关注 + 分区入口）时为真。
  static Future<LoadingState<rust_api.LiveIndexPage>> feedIndex({
    required int pn,
    bool moduleSelect = false,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchLiveIndex(
        context: await _context(Api.liveFeedIndex),
        identity: _identity(RustRequest.accountFor(Api.liveFeedIndex)),
        page: pn,
        moduleSelect: moduleSelect,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取直播首页推荐出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取分区二级列表的一屏。
  static Future<LoadingState<rust_api.LiveSecondPage>> secondList({
    required int pn,
    int? areaId,
    int? parentAreaId,
    String? sortType,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchLiveSecondList(
        context: await _context(Api.liveSecondList),
        identity: _identity(RustRequest.accountFor(Api.liveSecondList)),
        page: pn,
        areaId: areaId,
        parentAreaId: parentAreaId,
        sortType: sortType,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取直播分区列表出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取关注的直播。只回正在直播的那些。
  static Future<LoadingState<rust_api.LiveFollowingPage>> follow(
    int page,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchLiveFollow(
        context: await _context(Api.liveFollow),
        page: page,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取关注的直播出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取 H5 房间信息（房间 + 主播 + 看过人数）。
  static Future<LoadingState<rust_api.LiveRoomH5>> roomInfoH5({
    required int roomId,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchLiveRoomH5(
        context: await _context(Api.liveRoomInfoH5),
        roomId: roomId,
      );
      final data = res.data;
      if (data == null) return _failure(res.errorDetail, res.errorCode);
      return Success(data);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取直播间信息出错: $e\n$st');
      return Error('$e');
    }
  }
}
