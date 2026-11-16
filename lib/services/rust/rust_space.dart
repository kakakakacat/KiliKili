import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/rust/api/space.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/space.dart'
    show MemberCard, MemberInfo, MemberRelationStat, MemberUpStat;

/// 用户空间那一族的取数路径（S6 / issue #56）：别人的资料卡、关注关系
/// 计数、播放与获赞总数、用户名片，以及举报。
///
/// 基线对应的 `MemberHttp` 函数随本次迁移逐族删除，按 ADR-011 决策四
/// 不留降级开关。
///
/// ## 账号
///
/// 这一族在 `ApiType.apiTypeSet` 里都没有登记，与基线拦截器一样走 `main`。
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
    ...RustRequest.headersFor(account),
    ('origin', HttpString.spaceBaseUrl),
    ('referer', '${HttpString.spaceBaseUrl}/$mid/dynamic'),
    ('user-agent', BrowserUa.pc),
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
  }) async {
    final acc = Accounts.get(AccountType.main);
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

  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '请求失败', code: code) : Success(data);

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what 出错: $e\n$st');
    return Error('$e');
  }
}
