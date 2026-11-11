import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/rust/api/reply.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/reply.dart'
    show
        Emote,
        EmoteMeta,
        EmotePackage,
        InteractStatus,
        ReplyCursor,
        ReplyInteract,
        ReplyItem,
        ReplyList,
        ReplyMember,
        ReplyPage,
        ReplyReplyList;

/// 评论 REST 只读路径的取数：根评论列表、楼中楼、评论区开关、表情包面板
/// （S1 / issue #51）。
///
/// 基线对应的 `ReplyHttp.replyList` / `replyReplyList` / `replyInteraction`
/// / `getEmoteList` 已随本次迁移删除，按 ADR-011 决策四不留降级开关。写操作
/// （点赞/点踩/置顶/举报/开关评论区）仍在 `ReplyHttp`，等 #50。
///
/// ## 评论区 UI 走的是 gRPC，不是这里
///
/// 本文件迁的是 REST 这条路径。它在当前代码里的唯一消费者是
/// `ReplyUtils`（评论是否被 shadow ban 的自检），表情包面板则服务于评论
/// 发布框。评论列表本身由 `lib/grpc/` 的 protobuf 类型驱动（S2 / #52）。
///
/// ## 账号归属
///
/// `replyList` 与 `replyReplyList` 在 `ApiType.apiTypeSet` 里登记为
/// `heartbeat`，其余走 `main`。必须与基线拦截器的选择一致，否则两条路径
/// 用的是不同账号。
abstract final class RustReply {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// 评论区已关闭 / 评论不可见。自检靠它把 shadow ban 与普通失败分开。
  static const int codeReplyInvisible = 12022;

  static Future<rust_api.ReplyContext> _context({
    required AccountType account,
    required String path,
  }) async {
    final acc = Accounts.get(account);
    return rust_api.ReplyContext(
      baseUrl: HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(acc, path),
      headers: RustRequest.headersFor(acc),
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
    );
  }

  /// **刻意匿名**的请求上下文：不带 Cookie，也不带任何账号头。
  ///
  /// 对应基线 `ReplyHttp.options`（`cookie: ''` + `NoAccount`）：带上
  /// `NoAccount` 时拦截器会整段跳过（`account_mgr.dart` 的 `onRequest`
  /// 第一行就 return），所以连它平时补的 `referer` 都不会有。这里照抄
  /// 那个结果——匿名视角是 shadow ban 自检的前提，多一个头都可能让服务端
  /// 把请求当成「有身份」。
  ///
  /// 放在本文件而不是 `RustRequest`：那是各切片共用的小文件，按并行约定
  /// 只做追加；而「刻意不带账号」目前只有评论这一处需要。
  static rust_api.ReplyContext _anonymousContext() => rust_api.ReplyContext(
    baseUrl: HttpString.apiBaseUrl,
    cookie: null,
    headers: [
      // 基线 BaseOptions 的两条（Http2Adapter 不会自动补 UA）。
      ('user-agent', 'Dart/3.6 (dart:io)'),
      ('accept-encoding', 'br,gzip'),
      // 基线 ReplyHttp.options 带的就是这一组，取同一个常量而不是抄值。
      for (final e in Constants.baseHeaders.entries) (e.key, e.value),
    ],
    timeoutMs: BigInt.from(_timeoutMs),
    transport: RustTransport.current(),
  );

  /// 一页根评论（匿名形态，`/x/v2/reply/main`）。
  ///
  /// [sort] 用基线 `ReplySortType` 的序号（0 时间、1 热度）；服务端的
  /// `mode` 由 Rust 侧换算。[nextOffset] 是上一页 `cursor.nextOffset`，
  /// 首页传空串。
  static Future<LoadingState<rust_api.ReplyList>> replyListAnonymous({
    required int oid,
    required int type,
    required String nextOffset,
    int sort = 1,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchReplyListAnonymous(
        context: _anonymousContext(),
        oid: oid,
        kind: type,
        offset: nextOffset,
        sort: sort,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('评论列表', e, st);
    }
  }

  /// 一页根评论（登录形态，`/x/v2/reply`，分页在 `page` 而非 `cursor`）。
  ///
  /// 迁移时保留了这条形态：它是 REST 评论列表的另一半定义，请求形状与
  /// 响应形状都已被 fixture 锁死。当前**没有调用点**——基线里这个分支
  /// 同样没有（`ReplyUtils` 只用匿名形态），评论区 UI 走 gRPC。
  static Future<LoadingState<rust_api.ReplyList>> replyListPaged({
    required int oid,
    required int type,
    required int page,
    int sort = 1,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchReplyListPaged(
        context: await _context(
          account: AccountType.heartbeat,
          path: Api.replyList,
        ),
        oid: oid,
        kind: type,
        sort: sort,
        page: page,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('评论列表', e, st);
    }
  }

  /// 一页楼中楼。
  ///
  /// [isLogin] 决定两件事，缺一不可：带不带 Cookie，以及带不带 csrf。
  /// 自检正是靠「同一楼，登录看得到而匿名看不到」判断 shadow ban。
  ///
  /// 失败时 `code` 是业务码，[codeReplyInvisible] 表示评论不可见。
  static Future<LoadingState<rust_api.ReplyReplyList>> replyReplyList({
    required bool isLogin,
    required int oid,
    required int root,
    required int type,
    required int pageNum,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchReplyReplyList(
        context: isLogin
            ? await _context(
                account: AccountType.heartbeat,
                path: Api.replyReplyList,
              )
            : _anonymousContext(),
        oid: oid,
        root: root,
        kind: type,
        page: pageNum,
        csrf: isLogin ? Accounts.main.csrf : null,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('楼中楼', e, st);
    }
  }

  /// UP 主自己稿件的评论区开关状态（精选评论、评论区开关）。要登录态。
  static Future<LoadingState<rust_api.ReplyInteract>> replyInteraction({
    required int oid,
    required int type,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchReplyInteraction(
        context: await _context(
          account: AccountType.main,
          path: Api.replyInteraction,
        ),
        oid: oid,
        kind: type,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('评论区开关状态', e, st);
    }
  }

  /// 表情包面板。未登录时服务端给的是空列表，不是错误。
  ///
  /// 返回类型里的 `?` 是迁就调用点：`CommonListController` 的 `R` 必须与
  /// 它内部的 `LoadingState<List<T>?>` 对齐。Rust 侧成功时一定给列表。
  static Future<LoadingState<List<rust_api.EmotePackage>?>> emoteList({
    String? business,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchEmotePanel(
        context: await _context(account: AccountType.main, path: Api.myEmote),
        business: business ?? 'reply',
      );
      return _unwrap<List<rust_api.EmotePackage>?>(
        result.data,
        result.errorDetail,
        result.errorCode,
      );
    } catch (e, st) {
      return _crashed('表情包', e, st);
    }
  }

  /// Rust 侧的失败一律是值：`data` 为空就说明这次没拿到。
  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '请求失败', code: code) : Success(data);

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] 取$what出错: $e\n$st');
    return Error('$e');
  }
}
