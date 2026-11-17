import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/history.dart' as rust_api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/history.dart'
    show
        HistoryItem,
        HistoryPage,
        HistoryRef,
        HistoryTab,
        LaterItem,
        LaterPage,
        MediaList,
        MediaListItem,
        MediaListParams;

/// 观看历史、稍后再看与播放列表的取数路径（S7 / issue #57）。
///
/// 基线对应的 `UserHttp` 剩下十一个函数已随本次迁移删除，按 ADR-011
/// 决策四不留降级开关。
///
/// ## 账号：历史那几条走 `Accounts.history`
///
/// 基线对历史相关接口**显式**传 `Accounts.history`，而不是按路径查
/// `ApiType.apiTypeSet`。这是迁移时最容易丢的一条（AGENTS.md §3），所以
/// 这里把账号做成每个方法的可选参数，默认值就是 `Accounts.history`，与
/// 基线一字对应；稍后再看与播放列表则走 `main`。
///
/// ## 角标与进度条不在这里
///
/// 基线把它们做成模型上的 getter（返回 Widget）。渲染留在宿主，Rust 只给
/// 判据所需的原始值——`isFav` / `badge` / `liveStatus` / `progress`。
abstract final class RustHistory {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// 历史与稍后再看的默认页长，与基线一致。
  static const int pageSize = 20;

  static Future<rust_api.HistoryContext> _context({
    required String path,
    required Account account,
    String mixinKey = '',
    bool write = false,
  }) async => rust_api.HistoryContext(
    baseUrl: HttpString.apiBaseUrl,
    cookie: await RustRequest.cookieFor(account, path),
    csrf: write ? RustRequest.csrfFor(account) : null,
    headers: RustRequest.headersFor(account),
    timeoutMs: BigInt.from(_timeoutMs),
    transport: RustTransport.current(),
    mixinKey: mixinKey,
    wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
  );

  /// 取 WBI mixin key。拿不到就别发请求——基线会用空串继续签名并换回一个
  /// 业务错误码，那只是把失败推后，还更难诊断。
  static Future<String?> _mixinKey() async {
    final key = await WbiSign.getWbiKeys();
    return key.isEmpty ? null : key;
  }

  /// 一页观看历史。[max] / [viewAt] 是游标，首页都传 null。
  static Future<LoadingState<rust_api.HistoryPage>> historyList({
    required String type,
    int? max,
    int? viewAt,
    Account? account,
    int ps = pageSize,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchHistoryList(
        context: await _context(
          path: Api.historyList,
          account: account ?? Accounts.history,
        ),
        kind: type,
        pageSize: ps,
        max: max,
        viewAt: viewAt,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('观看历史', e, st);
    }
  }

  /// 搜索历史记录。响应与 [historyList] 同形。
  static Future<LoadingState<rust_api.HistoryPage>> searchHistory({
    required int pn,
    required String keyword,
    Account? account,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSearchHistory(
        context: await _context(
          path: Api.searchHistory,
          account: account ?? Accounts.history,
        ),
        page: pn,
        keyword: keyword,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('搜索历史', e, st);
    }
  }

  /// 历史记录是否已暂停。
  static Future<LoadingState<bool>> historyStatus({Account? account}) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchHistoryStatus(
        context: await _context(
          path: Api.historyStatus,
          account: account ?? Accounts.history,
        ),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('历史记录开关', e, st);
    }
  }

  /// 暂停/恢复记录历史。[switchStatus] 为 true 表示暂停。
  static Future<LoadingState<void>> pauseHistory(
    bool switchStatus, {
    Account? account,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.pauseHistory(
          context: await _context(
            path: Api.pauseHistory,
            account: account ?? Accounts.history,
            write: true,
          ),
          paused: switchStatus,
        ),
      );
    } catch (e, st) {
      return _crashed('暂停历史记录', e, st);
    }
  }

  /// 清空历史记录。
  ///
  /// 失败**不自动重发**：这条不可逆，自动重发意味着「用户以为失败、实际
  /// 已经清空」（ADR-013，Rust 侧定级 Unsafe）。
  static Future<LoadingState<void>> clearHistory({Account? account}) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.clearHistory(
          context: await _context(
            path: Api.clearHistory,
            account: account ?? Accounts.history,
            write: true,
          ),
        ),
      );
    } catch (e, st) {
      return _crashed('清空历史记录', e, st);
    }
  }

  /// 删除一条历史记录。[kid] 形如 `archive_123456`。
  static Future<LoadingState<void>> delHistory(
    String kid, {
    Account? account,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.delHistory(
          context: await _context(
            path: Api.delHistory,
            account: account ?? Accounts.history,
            write: true,
          ),
          kid: kid,
        ),
      );
    } catch (e, st) {
      return _crashed('删除历史记录', e, st);
    }
  }

  /// 一页稍后再看。WBI 签名，走 `main` 账号。
  static Future<LoadingState<rust_api.LaterPage>> seeYouLater({
    required int page,
    int viewed = 0,
    String keyword = '',
    bool asc = false,
    int ps = pageSize,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchSeeYouLater(
        context: await _context(
          path: Api.seeYouLater,
          account: Accounts.main,
          mixinKey: mixinKey,
        ),
        page: page,
        pageSize: ps,
        viewed: viewed,
        keyword: keyword,
        asc: asc,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('稍后再看', e, st);
    }
  }

  /// 加入稍后再看。[aid] 与 [bvid] 至少给一个。
  static Future<LoadingState<void>> toViewLater({
    String? bvid,
    int? aid,
  }) async {
    assert(aid != null || bvid != null);
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.toViewLater(
          context: await _context(
            path: Api.toViewLater,
            account: Accounts.main,
            write: true,
          ),
          aid: aid,
          bvid: bvid,
        ),
      );
    } catch (e, st) {
      return _crashed('稍后再看', e, st);
    }
  }

  /// 从稍后再看移除。[aids] 是逗号分隔的 aid 串。
  static Future<LoadingState<void>> toViewDel({required String aids}) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.toViewDel(
          context: await _context(
            path: Api.toViewDel,
            account: Accounts.main,
            write: true,
          ),
          resources: aids,
        ),
      );
    } catch (e, st) {
      return _crashed('移除稍后再看', e, st);
    }
  }

  /// 清空稍后再看。[cleanType]：null 全部，1 失效的，2 已看过的。
  ///
  /// 失败**不自动重发**，同 [clearHistory]。
  static Future<LoadingState<void>> toViewClear([int? cleanType]) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.toViewClear(
          context: await _context(
            path: Api.toViewClear,
            account: Accounts.main,
            write: true,
          ),
          cleanType: cleanType,
        ),
      );
    } catch (e, st) {
      return _crashed('清空稍后再看', e, st);
    }
  }

  /// 播放列表一页。
  static Future<LoadingState<rust_api.MediaList>> getMediaList({
    required int type,
    required int bizId,
    required int ps,
    int? oid,
    int? otype,
    bool withCurrent = false,
    bool desc = true,
    int sortField = 1,
    bool direction = false,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchMediaList(
        context: await _context(path: Api.mediaList, account: Accounts.main),
        params: rust_api.MediaListParams(
          kind: type,
          bizId: bizId,
          oid: oid,
          otype: otype,
          pageSize: ps,
          direction: direction,
          desc: desc,
          sortField: sortField,
          withCurrent: withCurrent,
        ),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('播放列表', e, st);
    }
  }

  /// 续播位置，**毫秒**。历史接口以秒计，且 `-1`（已看完）要从头播。
  static int? playbackProgressMs(int? progress) =>
      rust_api.playbackProgressMs(progress: progress);

  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '请求失败', code: code) : Success(data);

  /// 写操作的结果没有载荷：`errorKind` 为空就是成功。
  static LoadingState<void> _unwrapWrite(WriteResult result) =>
      result.errorKind == null
      ? const Success(null)
      : Error(result.errorDetail ?? '请求失败', code: result.errorCode);

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what 出错: $e\n$st');
    return Error('$e');
  }
}
