import 'dart:convert';

import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/rust/api/msg.dart' as rust_api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/msg.dart'
    show
        AtFeed,
        AtFeedContent,
        AtFeedItem,
        FeedBadgeSelection,
        FeedCursor,
        FeedUser,
        ImUserInfo,
        LikeDetail,
        LikeDetailCard,
        LikeDetailItem,
        LikeDetailPage,
        LikeFeed,
        LikeFeedContent,
        LikeFeedItem,
        LikeLatest,
        LikeTotal,
        MsgDndSetting,
        MsgFeedUnread,
        ReplyFeed,
        ReplyFeedContent,
        ReplyFeedItem,
        SessionSs,
        SingleUnread,
        SysMsgItem;

/// 消息中心与私信设置的取数路径（S12 / issue #63）。
///
/// 基线 `MsgHttp`（`lib/http/msg.dart`）里属于消息域的 19 条接口已随本片
/// 删除（十条只读 + 九条写），按 ADR-011 决策四不留降级开关。写操作尤其不能留：只读降级只是多
/// 发一次请求，写操作降级意味着**可能重复提交**（ADR-013 决策五）。
///
/// ## 三个域名
///
/// 这一片横跨 `api.bilibili.com`、`api.vc.bilibili.com` 与
/// `message.bilibili.com`。基线把域名拼进了 `Api.*` 常量，这里按接口分别
/// 传 `baseUrl`——**Cookie 也按域名取**（`RustRequest.cookieFor` 要的是
/// 完整 URL），传错域名的话请求会以「未登录」失败，而不是 404。
///
/// ## 账号
///
/// 这些接口在 `ApiType.apiTypeSet` 里都没有登记，因此与基线拦截器一样走
/// `main` 账号。
///
/// ## 刻意留在 Dart 的四条
///
/// `uploadImage` / `uploadBfs`（multipart 上传 + 取消）属于一项还没有决策的
/// 传输能力，见 **issue #193**；`removeDynamic` 属于动态域，归 S5 续单
/// #150。三条仍在 `MsgHttp` 里。
abstract final class RustMsg {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// 系统通知的默认页长，与基线一致。
  static const int sysNotifyPageSize = 20;

  /// [write] 为 true 时带上 csrf，交给 Rust 的 `send_write` 统一注入。
  ///
  /// **带 WBI 签名的两条写操作（[ackSessionMsg]、[removeMsg]）不走这里**：
  /// 它们的签名覆盖 csrf，必须在签名前就进参数表，所以 csrf 走函数参数。
  /// 那是 ADR-013 决策四的一处显式例外，理由写在 Rust 侧。
  static Future<rust_api.MsgContext> _context({
    required String baseUrl,
    required String endpoint,
    String mixinKey = '',
    bool write = false,
  }) async {
    final acc = Accounts.get(AccountType.main);
    return rust_api.MsgContext(
      baseUrl: baseUrl,
      // [endpoint] 是 `Api.*` 常量本身：相对路径由 cookieFor 补上
      // api.bilibili.com，已经带域名的（tUrl / messageBaseUrl 那些）原样用。
      // cookie jar 按域名存，取错域名的表现是「未登录」而不是 404。
      cookie: await RustRequest.cookieFor(acc, endpoint),
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

  /// 当前账号的 csrf。未登录时为 `null`。
  static String? get _csrf => RustRequest.csrfFor(Accounts.get(AccountType.main));

  // ------------------------------------------------------------ 三条 feed

  /// 「回复我的」一页。
  static Future<LoadingState<rust_api.ReplyFeed>> msgFeedReplyMe({
    int? cursor,
    int? cursorTime,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchReplyFeed(
        context: await _context(
          baseUrl: HttpString.apiBaseUrl,
          endpoint: Api.msgFeedReply,
        ),
        cursor: cursor,
        cursorTime: cursorTime,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('「回复我的」', e, st);
    }
  }

  /// 「@我的」一页。
  static Future<LoadingState<rust_api.AtFeed>> msgFeedAtMe({
    int? cursor,
    int? cursorTime,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchAtFeed(
        context: await _context(
          baseUrl: HttpString.apiBaseUrl,
          endpoint: Api.msgFeedAt,
        ),
        cursor: cursor,
        cursorTime: cursorTime,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('「@我的」', e, st);
    }
  }

  /// 「收到的赞」一页。
  static Future<LoadingState<rust_api.LikeFeed>> msgFeedLikeMe({
    int? cursor,
    int? cursorTime,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchLikeFeed(
        context: await _context(
          baseUrl: HttpString.apiBaseUrl,
          endpoint: Api.msgFeedLike,
        ),
        cursor: cursor,
        cursorTime: cursorTime,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('「收到的赞」', e, st);
    }
  }

  /// 某条赞的明细。
  ///
  /// [cardId] 与 [lastMid] 按字符串传：服务端对它们既可能回整数也可能回
  /// 字符串（同类问题见 #166），基线把参数声明成 `Object` 正是这个原因。
  static Future<LoadingState<rust_api.LikeDetail>> msgLikeDetail({
    required String cardId,
    required int pn,
    String lastMid = '0',
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchLikeDetail(
        context: await _context(
          baseUrl: HttpString.apiBaseUrl,
          endpoint: Api.msgLikeDetail,
        ),
        cardId: cardId,
        pn: pn,
        lastMid: lastMid,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('赞的明细', e, st);
    }
  }

  // ---------------------------------------------------------- 系统通知

  /// 系统通知列表。走 `message.bilibili.com`。
  static Future<LoadingState<List<rust_api.SysMsgItem>>> msgFeedNotify({
    int? cursor,
    int pageSize = sysNotifyPageSize,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSysNotify(
        context: await _context(
          baseUrl: HttpString.messageBaseUrl,
          endpoint: Api.msgSysNotify,
        ),
        cursor: cursor,
        pageSize: pageSize,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('系统通知', e, st);
    }
  }

  /// 更新系统通知的已读游标。
  ///
  /// 基线发的是 GET，照搬。`cursor` 是位点不是增量，重发安全。
  static Future<LoadingState<void>> msgSysUpdateCursor(int cursor) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final csrf = _csrf;
    if (csrf == null) return const Error('未登录');
    try {
      return _unwrapWrite(
        await rust_api.updateSysCursor(
          context: await _context(
            baseUrl: HttpString.messageBaseUrl,
            endpoint: Api.msgSysUpdateCursor,
          ),
          cursor: cursor,
          csrf: csrf,
        ),
      );
    } catch (e, st) {
      return _crashed('更新系统通知游标', e, st);
    }
  }

  /// 删除一条系统通知。
  static Future<LoadingState<void>> delSysMsg(String id) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.delSysMsg(
          context: await _context(
            baseUrl: HttpString.messageBaseUrl,
            endpoint: HttpString.messageBaseUrl + Api.delSysMsg,
            write: true,
          ),
          id: id,
        ),
      );
    } catch (e, st) {
      return _crashed('删除系统通知', e, st);
    }
  }

  // ------------------------------------------------------------ 未读数

  /// 私信未读数。返回值里的 `badge` 是算好的红点数字。
  static Future<LoadingState<rust_api.SingleUnread>> msgUnread() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchSingleUnread(
        context: await _context(
          baseUrl: HttpString.tUrl,
          endpoint: Api.msgUnread,
        ),
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('私信未读数', e, st);
    }
  }

  /// 消息中心未读数。
  ///
  /// [selection] 是用户在设置页勾选的那几项——算哪几个 Tab 的规则在 Rust，
  /// 勾了什么在 Dart，返回值里的 `badge` 已经算完。
  static Future<LoadingState<rust_api.MsgFeedUnread>> msgFeedUnread(
    rust_api.FeedBadgeSelection selection,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchFeedUnread(
        context: await _context(
          baseUrl: HttpString.apiBaseUrl,
          endpoint: Api.msgFeedUnread,
        ),
        pick: selection,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('消息中心未读数', e, st);
    }
  }

  // ------------------------------------------------------ 私信会话设置

  /// IM 用户资料。[uids] 是逗号分隔的 mid 串。
  ///
  /// 这条是 GET 却要 csrf：`api.vc` 的 `im`/`link_setting` 两组接口即使
  /// 只读也校验，不带就回 -111。
  static Future<LoadingState<List<rust_api.ImUserInfo>>> imUserInfos({
    required String uids,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final csrf = _csrf;
    if (csrf == null) return const Error('未登录');
    try {
      final result = await rust_api.fetchImUserInfos(
        context: await _context(
          baseUrl: HttpString.tUrl,
          endpoint: Api.imUserInfos,
        ),
        uids: uids,
        csrf: csrf,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('IM 用户资料', e, st);
    }
  }

  /// 与某人的会话设置。
  static Future<LoadingState<rust_api.SessionSs>> getSessionSs({
    required int talkerUid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final csrf = _csrf;
    if (csrf == null) return const Error('未登录');
    try {
      final result = await rust_api.fetchSessionSs(
        context: await _context(
          baseUrl: HttpString.tUrl,
          endpoint: Api.getSessionSs,
        ),
        talkerUid: talkerUid,
        csrf: csrf,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('会话设置', e, st);
    }
  }

  /// 免打扰设置。[uidsStr] 是要查的那些人，`own_uid` 由这里补上自己的 mid。
  static Future<LoadingState<List<rust_api.MsgDndSetting>>> getMsgDnd({
    required String uidsStr,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final csrf = _csrf;
    if (csrf == null) return const Error('未登录');
    try {
      final result = await rust_api.fetchMsgDnd(
        context: await _context(
          baseUrl: HttpString.tUrl,
          endpoint: Api.getMsgDnd,
        ),
        ownUid: Accounts.main.mid,
        uidsStr: uidsStr,
        csrf: csrf,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('免打扰设置', e, st);
    }
  }

  // ------------------------------------------------------------ 写操作

  /// 删除消息中心里的一条。[tp]：0 赞、1 回复、2 @。
  static Future<LoadingState<void>> delMsgfeed(int tp, String id) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.delMsgFeed(
          context: await _context(
            baseUrl: HttpString.apiBaseUrl,
            endpoint: Api.delMsgfeed,
            write: true,
          ),
          tp: tp,
          id: id,
        ),
      );
    } catch (e, st) {
      return _crashed('删除消息', e, st);
    }
  }

  /// 开关某条赞的通知。[noticeState] 是目标状态，不是切换。
  static Future<LoadingState<void>> msgSetNotice({
    required String id,
    required int noticeState,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.setNotice(
          context: await _context(
            baseUrl: HttpString.apiBaseUrl,
            endpoint: Api.msgSetNotice,
            write: true,
          ),
          id: id,
          noticeState: noticeState,
        ),
      );
    } catch (e, st) {
      return _crashed('设置通知', e, st);
    }
  }

  /// 设置免打扰。[uid] 是自己的 mid，[dndUid] 是对方的。
  static Future<LoadingState<void>> setMsgDnd({
    required int uid,
    required int setting,
    required int dndUid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.setMsgDnd(
          context: await _context(
            baseUrl: HttpString.tUrl,
            endpoint: Api.setMsgDnd,
            write: true,
          ),
          uid: uid,
          setting: setting,
          dndUid: dndUid,
        ),
      );
    } catch (e, st) {
      return _crashed('免打扰设置', e, st);
    }
  }

  /// 设置内容推送开关。
  static Future<LoadingState<void>> setPushSs({
    required int setting,
    required int talkerUid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.setPushSs(
          context: await _context(
            baseUrl: HttpString.tUrl,
            endpoint: Api.setPushSs,
            write: true,
          ),
          setting: setting,
          talkerUid: talkerUid,
        ),
      );
    } catch (e, st) {
      return _crashed('内容推送设置', e, st);
    }
  }

  /// 标记私信已读。WBI 签名，签名覆盖 csrf，所以 csrf 走参数。
  static Future<LoadingState<void>> ackSessionMsg({
    required int talkerId,
    required int ackSeqno,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final csrf = _csrf;
    if (csrf == null) return const Error('未登录');
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      return _unwrapWrite(
        await rust_api.ackSession(
          context: await _context(
            baseUrl: HttpString.tUrl,
            endpoint: Api.ackSessionMsg,
            mixinKey: mixinKey,
          ),
          talkerId: talkerId,
          ackSeqno: ackSeqno,
          csrf: csrf,
        ),
      );
    } catch (e, st) {
      return _crashed('标记私信已读', e, st);
    }
  }

  /// 删除一个私信会话。WBI 签名，理由同 [ackSessionMsg]。
  static Future<LoadingState<void>> removeMsg(int talkerId) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final csrf = _csrf;
    if (csrf == null) return const Error('未登录');
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      return _unwrapWrite(
        await rust_api.removeSession(
          context: await _context(
            baseUrl: HttpString.tUrl,
            endpoint: HttpString.tUrl + Api.removeMsg,
            mixinKey: mixinKey,
          ),
          talkerId: talkerId,
          csrf: csrf,
        ),
      );
    } catch (e, st) {
      return _crashed('删除会话', e, st);
    }
  }

  /// 举报一位私信对象。
  ///
  /// **这条一次都不会重发**：没有幂等键，服务端每收一次立一条工单，而且
  /// 查不到上一次立没立（ADR-013 决策一）。超时后要不要再举报一次由用户
  /// 决定——所以失败就是失败，这里不做任何自动重试。
  static Future<LoadingState<void>> imMsgReport({
    required int accusedUid,
    required int reasonType,
    required String reasonDesc,
    required Map<dynamic, dynamic> comment,
    required Map<dynamic, dynamic> extra,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.imReport(
          context: await _context(
            baseUrl: HttpString.tUrl,
            endpoint: Api.imMsgReport,
            write: true,
          ),
          accusedUid: accusedUid,
          reasonType: reasonType,
          reasonDesc: reasonDesc,
          // 这两个字段的结构由举报弹窗定，Rust 不该替它定义形状，所以
          // 在这里编码成字符串再过去——基线同样是 jsonEncode 后放进表单。
          commentJson: jsonEncode(comment),
          extraJson: jsonEncode(extra),
        ),
      );
    } catch (e, st) {
      return _crashed('举报', e, st);
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
    if (kDebugMode) debugPrint('[rust] $what 出错: $e\n$st');
    return Error('$e');
  }
}
