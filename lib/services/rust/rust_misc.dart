import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/rust/api/misc.dart' as rust_api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/misc.dart'
    show
        Artist,
        BgmRecommend,
        BlackList,
        BlackListItem,
        DanmakuFilter,
        FilterRule,
        HotSongHeat,
        MatchContest,
        MatchSeason,
        MatchTeam,
        MusicComment,
        MusicDetail,
        SongHeat;

/// 杂项只读接口的取数路径：音乐页、黑名单、电竞赛事、云端弹幕屏蔽规则
/// （S15 / issue #66）。
///
/// 基线对应的**整个** `MusicHttp`（`lib/http/music.dart`）、整个
/// `DanmakuFilterHttp`（`lib/http/danmaku_block.dart`）、`BlackHttp.blackList`、
/// `MatchHttp.matchInfo` 与 `FollowHttp.sortFollowTag` 都已随迁移删除，
/// 按 ADR-011 决策四不留降级开关。写操作尤其不能留：只读降级只是多发一次
/// 请求，写操作降级意味着**可能重复提交**（ADR-013 决策五）。
///
/// `FollowHttp.followings` 与 `FanHttp.fans` 还在 Dart：它们共用 `FollowData`
/// 模型，按 ADR-011 决策六要两个生产者一起迁，归 #66 的下一批。
///
/// 这几条接口在 `ApiType.apiTypeSet` 里都没有登记，因此与基线拦截器一样
/// 走 `main` 账号。
abstract final class RustMisc {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// 基线黑名单的固定页长。
  static const int blackListPageSize = 50;

  /// [write] 为 true 时带上 csrf。只读路径一律不带——凭据少走一段就少
  /// 一处泄漏面，而且「这条是不是写操作」在调用点一眼看得见
  /// （ADR-013 决策四）。
  static Future<rust_api.MiscContext> _context({
    required String path,
    String mixinKey = '',
    bool write = false,
  }) async {
    final acc = Accounts.get(AccountType.main);
    return rust_api.MiscContext(
      baseUrl: HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(acc, path),
      csrf: write ? RustRequest.csrfFor(acc) : null,
      headers: RustRequest.headersFor(acc),
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
      mixinKey: mixinKey,
      wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
    );
  }

  /// 取 WBI mixin key。拿不到就别发请求——基线会用空串继续签名并换回一个
  /// 业务错误码，那只是把失败推后，还更难诊断。
  static Future<String?> _mixinKey() async {
    final key = await WbiSign.getWbiKeys();
    return key.isEmpty ? null : key;
  }

  /// BGM 详情。WBI 签名。
  static Future<LoadingState<rust_api.MusicDetail>> bgmDetail(
    String musicId,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchBgmDetail(
        context: await _context(path: Api.bgmDetail, mixinKey: mixinKey),
        musicId: musicId,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('BGM 详情', e, st);
    }
  }

  /// BGM 的相关推荐。
  static Future<LoadingState<List<rust_api.BgmRecommend>?>> bgmRecommend(
    String musicId,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchBgmRecommend(
        context: await _context(path: Api.bgmRecommend),
        musicId: musicId,
      );
      return _unwrap<List<rust_api.BgmRecommend>?>(
        result.data,
        result.errorDetail,
        result.errorCode,
      );
    } catch (e, st) {
      return _crashed('相关推荐', e, st);
    }
  }

  /// 一页黑名单。只读接口却要 csrf，是服务端的要求；真相源在 Dart 的
  /// 账号体系。
  static Future<LoadingState<rust_api.BlackList>> blackList({
    required int pn,
    int ps = blackListPageSize,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchBlackList(
        context: await _context(path: Api.blackList),
        page: pn,
        pageSize: ps,
        csrf: Accounts.main.csrf,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('黑名单', e, st);
    }
  }

  /// 一场比赛的详情。请求成功但服务端没给比赛时返回 `Success(null)`，
  /// 与基线一致——界面显示空态，不是错误。
  static Future<LoadingState<rust_api.MatchContest?>> matchInfo(
    int contestId,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchMatchInfo(
        context: await _context(path: Api.matchInfo),
        contestId: contestId,
      );
      final data = result.data;
      if (data == null) {
        return Error(result.errorDetail ?? '请求失败', code: result.errorCode);
      }
      return Success(data.contest);
    } catch (e, st) {
      return _crashed('赛事详情', e, st);
    }
  }

  /// 云端弹幕屏蔽规则，已按类型分好组。
  static Future<LoadingState<rust_api.DanmakuFilter>> danmakuFilter() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchDanmakuFilter(
        context: await _context(path: Api.danmakuFilter),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('弹幕屏蔽规则', e, st);
    }
  }

  /// 新增一条云端弹幕屏蔽规则。`type`：0 关键词 / 1 正则 / 2 用户。
  ///
  /// 返回的是 **Rust 侧的 typed 结构**：规则只剩 `FilterRule` 这一个类型，
  /// 读路径已经迁走，这里再留一个等价的 Dart 模型只会让两边各自漂移。
  ///
  /// 失败**不自动重发**。这条在 Rust 侧定级为「可判定」：重发会多一条，
  /// 但 [danmakuFilter] 查得到上一次生没生效（ADR-013 决策一/三）。界面
  /// 现在的做法是把失败提示给用户、由用户决定要不要再按一次，这就是那个
  /// 「查过之后再决定」——由人来查。
  static Future<LoadingState<rust_api.FilterRule>> danmakuFilterAdd({
    required String filter,
    required int type,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.addDanmakuFilter(
        context: await _context(path: Api.danmakuFilterAdd, write: true),
        filter: filter,
        kind: type,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('新增弹幕屏蔽规则', e, st);
    }
  }

  /// 删除一条云端弹幕屏蔽规则。
  ///
  /// 成功时没有载荷——`errorKind == null` 就是全部结论，所以这里返回
  /// `LoadingState<void>` 而不是去解一个不存在的 `data`。
  static Future<LoadingState<void>> danmakuFilterDel({required int ids}) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.delDanmakuFilter(
          context: await _context(path: Api.danmakuFilterDel, write: true),
          ids: ids,
        ),
      );
    } catch (e, st) {
      return _crashed('删除弹幕屏蔽规则', e, st);
    }
  }

  /// 把一首 BGM 标记为「想听」或取消（#66）。
  ///
  /// `wish` 是**目标状态**不是「切换一下」——基线发的是 `state: 2/1`。
  /// 因此 Rust 侧定级为幂等，传输层在「请求没发出去」时会重发。
  static Future<LoadingState<void>> wishUpdate(
    String musicId,
    bool wish,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.updateWish(
          context: await _context(path: Api.wishUpdate, write: true),
          musicId: musicId,
          wish: wish,
        ),
      );
    } catch (e, st) {
      return _crashed('「想听」', e, st);
    }
  }

  /// 提交关注分组的新顺序（#66）。
  ///
  /// [tagids] 是逗号分隔的分组 id，顺序即目标顺序。整串是置为语义，所以
  /// 定级为幂等。
  static Future<LoadingState<void>> sortFollowTag({
    required String tagids,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.sortFollowTag(
          context: await _context(path: Api.sortFollowTag, write: true),
          tagids: tagids,
        ),
      );
    } catch (e, st) {
      return _crashed('关注分组排序', e, st);
    }
  }

  /// 写操作的结果没有载荷：`errorKind` 为空就是成功。
  static LoadingState<void> _unwrapWrite(WriteResult result) =>
      result.errorKind == null
      ? const Success(null)
      : Error(result.errorDetail ?? '请求失败', code: result.errorCode);

  /// Rust 侧的失败一律是值：`data` 为空就说明这次没拿到。
  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '请求失败', code: code) : Success(data);

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] 取$what出错: $e\n$st');
    return Error('$e');
  }
}
