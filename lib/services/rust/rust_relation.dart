import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/rust/api/misc.dart' show FollowList;
import 'package:kilikili/rust/api/relation.dart' as rust_api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/relation.dart' show FollowTag, Relation;

/// 关注分组与关注关系的取数路径（S6 #56 / S7 #57）。
///
/// 基线对应的 `MemberHttp` 八个分组函数与 `UserHttp` 三个关系函数已随本次
/// 迁移删除，按 ADR-011 决策四不留降级开关。
///
/// ## 这一族为什么一次迁完
///
/// `FollowList` 有六个生产者，S15（#66）先迁走了关注与粉丝两条，剩下四条
/// 全在这里。四条一起迁完，基线的 `FollowData` / `FollowItemModel` 与过渡
/// 适配 `lib/services/rust/follow_compat.dart` 才一并删得掉——中途每单独
/// 迁一条，都要多留一层只为别人而活的适配层（issue #57 的表）。
///
/// 关注与粉丝两条仍在 [RustMisc]：挪过来只换到「按协议分类更整齐」，代价
/// 是整个生成产物重排与上游同步多一个冲突点（AGENTS.md §9）。
///
/// ## 账号
///
/// 这一族在 `ApiType.apiTypeSet` 里都没有登记，与基线拦截器一样走 `main`。
abstract final class RustRelation {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// 分组内列表的默认页长，与基线 `MemberHttp.followUpGroup` 一致。
  static const int groupPageSize = 20;

  /// [write] 为 true 时才带 csrf。注意「只读却要 csrf」的那两条
  /// （[followedUp] / [sameFollowing]）把 csrf 当普通查询参数显式传入，
  /// 不走这里——凭据少走一段就少一处泄漏面（ADR-013 决策四）。
  static Future<rust_api.RelationContext> _context({
    required String path,
    String mixinKey = '',
    bool write = false,
  }) async {
    final acc = Accounts.get(AccountType.main);
    return rust_api.RelationContext(
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

  /// 关注分组清单。
  static Future<LoadingState<List<rust_api.FollowTag>>> followUpTags() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchFollowUpTags(
        context: await _context(path: Api.followUpTag),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('关注分组', e, st);
    }
  }

  /// 某个分组下的一页人。
  ///
  /// [mid] 省略表示看自己的分组，[tagid] 省略表示「全部关注」那一栏；
  /// [orderType] 空串是最近关注，`attention` 是最常访问。
  ///
  /// 返回的 `total` 恒为 null——这条接口的 `data` 是裸数组，不给总数，
  /// 翻页靠「这一页还满不满」判断，与基线一致。
  static Future<LoadingState<FollowList>> followUpGroup({
    int? mid,
    int? tagid,
    int? pn,
    int ps = groupPageSize,
    String orderType = '',
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchFollowUpGroup(
        context: await _context(path: Api.followUpGroup),
        mid: mid,
        tagid: tagid,
        page: pn,
        pageSize: ps,
        orderType: orderType,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('分组内关注', e, st);
    }
  }

  /// 在自己的关注里按昵称搜索。WBI 签名。
  static Future<LoadingState<FollowList>> followSearch({
    required int mid,
    required int pn,
    required int ps,
    required String name,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchFollowSearch(
        context: await _context(path: Api.followSearch, mixinKey: mixinKey),
        vmid: mid,
        page: pn,
        pageSize: ps,
        name: name,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('关注搜索', e, st);
    }
  }

  /// 「他关注的 up 主」一页。只读接口却要 csrf，是服务端的要求。
  static Future<LoadingState<FollowList>> followedUp({
    required int mid,
    required int pn,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchFollowedUp(
        context: await _context(path: Api.followedUp),
        csrf: Accounts.main.csrf,
        vmid: mid,
        page: pn,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('他关注的 up 主', e, st);
    }
  }

  /// 「你们共同关注」一页。[pn] 省略时服务端给第一页。
  static Future<LoadingState<FollowList>> sameFollowing({
    required int mid,
    int? pn,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSameFollowing(
        context: await _context(path: Api.sameFollowing),
        csrf: Accounts.main.csrf,
        vmid: mid,
        page: pn,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('共同关注', e, st);
    }
  }

  /// 与某个人的关系。
  static Future<LoadingState<rust_api.Relation>> userRelation(int mid) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchRelation(
        context: await _context(path: Api.relation),
        fid: mid,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('关注关系', e, st);
    }
  }

  /// 把若干人设置到若干分组。两个参数都是逗号分隔的 id 串。
  ///
  /// 服务端语义是「这些人的分组**置为**这一组」，所以 Rust 侧定级为幂等。
  static Future<LoadingState<void>> addUsers(
    String fids,
    String tagids,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.addUsersToTags(
          context: await _context(path: Api.addUsers, write: true),
          fids: fids,
          tagids: tagids,
        ),
      );
    } catch (e, st) {
      return _crashed('设置分组', e, st);
    }
  }

  /// 新建分组，成功时返回服务端分配的 `tagid`。
  ///
  /// 失败**不自动重发**：重发会多建一个同名分组，而服务端允许重名，事后
  /// 从响应看不出哪个是这次建的（ADR-013 决策二，Rust 侧定级 Unsafe）。
  static Future<LoadingState<int>> createFollowTag(String tagName) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.createFollowTag(
        context: await _context(path: Api.createFollowTag, write: true),
        tag: tagName,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('新建分组', e, st);
    }
  }

  /// 给分组改名。
  static Future<LoadingState<void>> updateFollowTag(
    int tagid,
    String name,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.updateFollowTag(
          context: await _context(path: Api.updateFollowTag, write: true),
          tagid: tagid,
          name: name,
        ),
      );
    } catch (e, st) {
      return _crashed('分组改名', e, st);
    }
  }

  /// 删除分组。
  static Future<LoadingState<void>> delFollowTag(int tagid) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.delFollowTag(
          context: await _context(path: Api.delFollowTag, write: true),
          tagid: tagid,
        ),
      );
    } catch (e, st) {
      return _crashed('删除分组', e, st);
    }
  }

  /// 加/取消特别关注。[isAdd] 为 false 时打的是取消那条路径。
  static Future<LoadingState<void>> specialAction({
    required int fid,
    bool isAdd = true,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.specialAction(
          context: await _context(
            path: isAdd ? Api.addSpecial : Api.delSpecial,
            write: true,
          ),
          fid: fid,
          add: isAdd,
        ),
      );
    } catch (e, st) {
      return _crashed('特别关注', e, st);
    }
  }

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
