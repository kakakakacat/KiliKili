import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/rust/api/user.dart' as rust_api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/user.dart'
    show
        CoinLogItem,
        LevelInfo,
        LoginLogItem,
        PrivacySetting,
        RejectPage,
        SubItem,
        SubList,
        UserInfo,
        UserRealName,
        UserStat,
        VideoTag;

/// 「我的」那一族的取数路径（S7 / issue #57）：登录用户信息、三个计数、
/// 硬币数与三份流水、实名、空间隐私设置、订阅、视频标签，以及四条写操作。
///
/// 基线对应的 `UserHttp` 十四个函数已随本次迁移删除，按 ADR-011 决策四
/// 不留降级开关。
///
/// ## 登录信息的本地缓存改存 JSON
///
/// 基线把 `UserInfoData` 整个对象用 Hive 适配器存盘。typed 直连之后导出
/// 结构不可变，适配器无从生成，缓存因此改成一段由 Rust 序列化的 JSON
/// （[cacheUserInfo] / [cachedUserInfo]）。老版本留下的二进制帧解不出来，
/// 按「没有缓存」处理——首屏空一下，等 nav 回来，这是换格式一次性的代价。
///
/// 「等 nav 回来」成立的前提是**登录态不依赖这份缓存**（issue #161）：
/// 它只是头像昵称的本地副本，不是判断有没有登录的依据。
///
/// ## 账号
///
/// 这一族在 `ApiType.apiTypeSet` 里都没有登记，与基线拦截器一样走 `main`。
abstract final class RustUser {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// [write] 为 true 时才带 csrf（ADR-013 决策四）。
  ///
  /// [baseUrl] 只有硬币数那条要换——它挂在 `account.bilibili.com`。
  static Future<rust_api.UserContext> _context({
    required String path,
    String? baseUrl,
    bool write = false,
  }) async {
    final acc = Accounts.get(AccountType.main);
    return rust_api.UserContext(
      baseUrl: baseUrl ?? HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(acc, path),
      csrf: write ? RustRequest.csrfFor(acc) : null,
      headers: RustRequest.headersFor(acc),
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
      ts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
    );
  }

  /// 登录用户信息。
  static Future<LoadingState<rust_api.UserInfo>> userInfo() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchUserInfo(
        context: await _context(path: Api.userInfo),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('登录用户信息', e, st);
    }
  }

  /// 「我的」页三个计数。
  static Future<LoadingState<rust_api.UserStat>> userStatOwner() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchUserStat(
        context: await _context(path: Api.userStatOwner),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('账号计数', e, st);
    }
  }

  /// 硬币数。
  ///
  /// 请求成功但服务端没给时返回 `Success(null)`，与基线一致——界面显示
  /// 「-」，不是错误。
  static Future<LoadingState<double?>> getCoin() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchCoin(
        // 这条不在 api 域上。
        context: await _context(
          path: Api.getCoin,
          baseUrl: HttpString.accountBaseUrl,
        ),
      );
      if (result.errorKind != null) {
        return Error(result.errorDetail ?? '请求失败', code: result.errorCode);
      }
      return Success(result.data);
    } catch (e, st) {
      return _crashed('硬币数', e, st);
    }
  }

  /// 硬币流水。
  static Future<LoadingState<List<rust_api.CoinLogItem>>> coinLog() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchCoinLog(
        context: await _context(path: Api.coinLog),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('硬币流水', e, st);
    }
  }

  /// 经验流水。与硬币流水同形，只换路径。
  static Future<LoadingState<List<rust_api.CoinLogItem>>> expLog() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchExpLog(
        context: await _context(path: Api.expLog),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('经验流水', e, st);
    }
  }

  /// 登录日志。
  static Future<LoadingState<List<rust_api.LoginLogItem>>> loginLog() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchLoginLog(
        context: await _context(path: Api.loginLog),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('登录日志', e, st);
    }
  }

  /// up 主实名信息。APP 签名，不带 csrf。
  static Future<LoadingState<rust_api.UserRealName>> getUserRealName(
    int mid,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final accessKey = Accounts.main.accessKey;
    if (accessKey == null) return const Error('账号未登录');
    try {
      final result = await rust_api.fetchUserRealName(
        context: await _context(path: Api.userRealName),
        accessKey: accessKey,
        upMid: mid,
        appkey: Constants.appKey,
        appsec: Constants.appSec,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('实名信息', e, st);
    }
  }

  /// 空间隐私设置。返回十五项，顺序与 `group` 即界面上的分组顺序；
  /// 中文标题由界面按 `key` 查 [privacyLabels]。
  static Future<LoadingState<List<rust_api.PrivacySetting>>>
  spaceSetting() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSpaceSetting(
        context: await _context(path: Api.spaceSetting),
        mid: Accounts.main.mid,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('隐私设置', e, st);
    }
  }

  /// 隐私设置每一项的中文标题。
  ///
  /// 文案留在宿主、`key` 与「是否反相」在 Rust，是 AGENTS.md §2 那条分界：
  /// 哪个开关是反的是协议语义（读错就把设置显示反），标题只是 i18n。
  static const Map<String, String> privacyLabels = {
    'fav_video': '公开我的收藏',
    'bangumi': '公开我的追番追剧',
    'comic': '公开我的追漫',
    'coins_video': '公开最近投币的视频',
    'likes_video': '公开最近点赞的视频',
    'played_game': '公开最近玩过的游戏',
    'dress_up': '公开拥有的粉丝装扮',
    'disable_following': '公开我的关注列表',
    'disable_show_fans': '公开我的粉丝列表',
    'close_space_medal': '公开佩戴的粉丝勋章',
    'only_show_wearing': '勋章墙公开显示所有粉丝勋章',
    'disable_show_school': '公开学校信息',
    'live_playback': '投稿视频列表中展现直播回放',
    'charge_video': '投稿视频列表中展现包月充电专属视频',
    'lesson_video': '投稿视频列表中展现课堂视频',
  };

  /// 订阅的收藏夹/合集一页。
  static Future<LoadingState<rust_api.SubList>> userSubFolder({
    required int mid,
    required int pn,
    required int ps,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSubList(
        context: await _context(path: Api.userSubFolder),
        upMid: mid,
        page: pn,
        pageSize: ps,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('订阅', e, st);
    }
  }

  /// 视频标签。没有标签的稿件返回空列表，不是失败。
  static Future<LoadingState<List<rust_api.VideoTag>>> videoTags({
    required String bvid,
    int? cid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchVideoTags(
        context: await _context(path: Api.videoTags),
        bvid: bvid,
        cid: cid,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('视频标签', e, st);
    }
  }

  /// 领大会员经验。按天只记一次，重发落到同一状态。
  static Future<LoadingState<void>> vipExpAdd() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.vipExpAdd(
          context: await _context(path: Api.vipExpAdd, write: true),
          mid: Accounts.main.mid,
        ),
      );
    } catch (e, st) {
      return _crashed('领取大会员经验', e, st);
    }
  }

  /// 批量改空间隐私设置。[values] 是「key → 值」，整批是置为语义。
  static Future<LoadingState<void>> spaceSettingMod(
    Map<String, dynamic> values,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.spaceSettingMod(
          context: await _context(path: Api.spaceSettingMod, write: true),
          values: values.entries
              .map((e) => (e.key, e.value.toString()))
              .toList(),
        ),
      );
    } catch (e, st) {
      return _crashed('修改隐私设置', e, st);
    }
  }

  /// 举报一条动态。
  ///
  /// 失败**不自动重发**：重发会多提交一次举报，事后也查不到上一次是否已
  /// 受理（ADR-013 决策二，Rust 侧定级 Unsafe）。
  static Future<LoadingState<void>> dynamicReport({
    required int mid,
    required String dynId,
    required int reasonType,
    String? reasonDesc,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.dynamicReport(
          context: await _context(path: Api.dynamicReport, write: true),
          mid: mid,
          dynId: dynId,
          reasonType: reasonType,
          reasonDesc: reasonDesc,
        ),
      );
    } catch (e, st) {
      return _crashed('举报动态', e, st);
    }
  }

  /// 预约/取消预约一个空间活动。[isFollow] 为真表示当前已预约，这次是取消。
  static Future<LoadingState<void>> spaceReserve({
    required int sid,
    required bool isFollow,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.spaceReserve(
          context: await _context(
            path: isFollow ? Api.spaceReserveCancel : Api.spaceReserve,
            write: true,
          ),
          sid: sid,
          reserve: !isFollow,
        ),
      );
    } catch (e, st) {
      return _crashed('预约', e, st);
    }
  }

  /// 把登录用户信息序列化成缓存串。序列化失败时返回 null，调用方不写盘。
  static String? cacheUserInfo(rust_api.UserInfo info) =>
      rust_api.serializeUserInfo(info: info);

  /// 把缓存串解回登录用户信息。解不出来（含老版本的 Hive 帧）返回 null。
  static rust_api.UserInfo? cachedUserInfo(String? json) =>
      json == null ? null : rust_api.parseUserInfoCache(json: json);

  /// 改名/换头像之后的缓存更新：只改两个字段，其余原样。
  static rust_api.UserInfo withProfile(
    rust_api.UserInfo info, {
    String? uname,
    String? face,
  }) => rust_api.userInfoWithProfile(info: info, uname: uname, face: face);

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
