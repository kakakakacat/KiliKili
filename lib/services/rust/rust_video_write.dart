import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/video/video_type.dart';
import 'package:kilikili/rust/api/video_write.dart' as rust_api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/extension/string_ext.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/common/constants.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/video_write.dart'
    show PgcTripleResult, UgcTripleResult;

/// 视频页写操作的取数（S4 / issue #54）。
///
/// 基线对应的 `VideoHttp` 写操作已随本次迁移删除，按 ADR-011 决策四与
/// ADR-013 决策五**不留降级开关**——写操作降级的代价是可能重复提交，
/// 比只读路径多一个量级。
///
/// ## 三类上下文，按主机选
///
/// 这一片横跨三个主机，鉴权方式跟着主机走（详见
/// `rust/crates/bili-api/src/video_write.rs` 的模块文档）：
///
/// - `api.bilibili.com`：Cookie + csrf，用 [_webContext]；
/// - `api.live.bilibili.com`：同上，只是 base_url 不同；
/// - `app.bilibili.com`：**不带 Cookie**，`access_key` + APP 签名，
///   用 [_appContext]。
///
/// 账号归属一律按 `Api.*` 那个完整常量去 `ApiType.apiTypeSet` 查
/// （[RustRequest.accountFor]），不在这里硬编码——心跳与三条上报归
/// `heartbeat`，「不感兴趣」归 `recommend`，其余落 `main`，少记一个
/// 就是换了个账号发请求。
///
/// ## 本地副作用的顺序是定死的
///
/// 拉黑名单、删评论缓存这些是平台存储，按 RFC §2.3 留在宿主；但必须
/// **先确认远端成功再改本地**，而「成功」只认 Rust 返回的
/// `errorKind == null`，不在 Dart 里重新解释 code（ADR-013 决策八）。
/// 放在本文件而不是各调用点，就是为了让这个顺序只有一处可写错。
///
/// ## 没迁的那一条
///
/// `replyAdd`（发评论）仍在 `VideoHttp`：它返回 protobuf `ReplyInfo`，
/// 由 gRPC 驱动的评论区消费，迁它等于先迁完 S2（#52）。它也是本片唯一
/// 一条 `Unsafe` 定级的接口。
abstract final class RustVideoWrite {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// 基线对「账号没有 access_key」的统一提示。APP 端接口没有它就只能
  /// 被服务端拒，提前拦住并告诉用户怎么办。
  static const String _needRelogin = '请退出账号后重新登录';

  /// web 端写操作的上下文。
  ///
  /// [endpoint] 用 `Api.*` 那个完整常量（账号与 Cookie 都按它查），
  /// [baseUrl] 单独给——`Api.roomEntryAction` 挂在直播主机上。
  ///
  /// [silent] 为 true 时不重试，给高频上报用，见
  /// [RustTransport.silentReport]。
  static Future<rust_api.VideoWriteContext> _webContext(
    String endpoint, {
    String baseUrl = HttpString.apiBaseUrl,
    bool silent = false,
  }) async {
    final account = RustRequest.accountFor(endpoint);
    return rust_api.VideoWriteContext(
      baseUrl: baseUrl,
      cookie: await RustRequest.cookieFor(account, endpoint),
      csrf: RustRequest.csrfFor(account),
      headers: RustRequest.headersFor(account),
      timeoutMs: BigInt.from(_timeoutMs),
      transport: silent
          ? RustTransport.silentReport()
          : RustTransport.current(),
    );
  }

  /// APP 端写操作的上下文。没有 cookie 字段，不是遗漏——基线拦截器对
  /// `app.bilibili.com` 的请求根本不加载 Cookie。
  static rust_api.AppWriteContext _appContext(Account account) =>
      rust_api.AppWriteContext(
        baseUrl: HttpString.appBaseUrl,
        headers: RustRequest.headersFor(account),
        appkey: Constants.appKey,
        appsec: Constants.appSec,
        accessKey: account.accessKey,
        ts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
        timeoutMs: BigInt.from(_timeoutMs),
        transport: RustTransport.current(),
      );

  /// 带自定义请求头的 web 上下文。
  ///
  /// 三连与关注在基线里额外带了桌面端 UA、`origin` 与 `referer`（不带会
  /// 被风控拦）。这些值的真相源在 Dart（`BrowserUa.pc`、各自的页面地址），
  /// 由这里放进上下文传入，Rust 不另存一份。
  static Future<rust_api.VideoWriteContext> _webContextWithHeaders(
    String endpoint,
    Map<String, String> extra,
  ) async {
    final base = await _webContext(endpoint);
    final overrides = extra.keys.map((key) => key.toLowerCase()).toSet();
    final headers = <(String, String)>[
      // reqwest 会保留同名头的多个值；基线 Dio 的逐键覆盖只发最终值。
      for (final header in base.headers)
        if (!overrides.contains(header.$1.toLowerCase())) header,
      ...extra.entries.map((e) => (e.key, e.value)),
    ];
    return rust_api.VideoWriteContext(
      baseUrl: base.baseUrl,
      cookie: base.cookie,
      csrf: base.csrf,
      headers: headers,
      timeoutMs: base.timeoutMs,
      transport: base.transport,
    );
  }

  // ------------------------------------------------------------ APP 端

  /// （取消）点赞。[like] 是**目标状态**。
  ///
  /// 成功时返回服务端给的 toast 文案；服务端没给文案也算成功，此处回落到
  /// 一个固定串——兜底文案是 i18n 的事，留在宿主。
  static Future<LoadingState<String>> likeVideo({
    required String bvid,
    required bool like,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final account = RustRequest.accountFor(Api.likeVideo);
      final result = await rust_api.likeVideo(
        context: _appContext(account),
        bvid: bvid,
        like: like,
      );
      if (result.errorKind != null) {
        return Error(result.errorDetail ?? '操作失败', code: result.errorCode);
      }
      return Success(result.toast ?? '操作成功');
    } catch (e, st) {
      return _crashed('点赞', e, st);
    }
  }

  /// （取消）点踩。[dislike] 是**目标状态**。
  static Future<LoadingState<void>> dislikeVideo({
    required String bvid,
    required bool dislike,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final account = RustRequest.accountFor(Api.dislikeVideo);
    if (account.accessKey.isNullOrEmpty) return const Error(_needRelogin);
    try {
      return _unwrapWrite(
        await rust_api.dislikeVideo(
          context: _appContext(account),
          bvid: bvid,
          dislike: dislike,
        ),
      );
    } catch (e, st) {
      return _crashed('点踩', e, st);
    }
  }

  /// 投币。[multiply] 是币数，[selectLike] 为 1 时附带点赞。
  ///
  /// **不自动重试**：重发会多扣硬币（ADR-013 定级为可判定）。失败了由
  /// 用户自己再按一次——用户看得见硬币数，比客户端再查一次可靠。
  static Future<LoadingState<void>> coinVideo({
    required String bvid,
    required int multiply,
    int selectLike = 0,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.coinVideo(
          context: _appContext(RustRequest.accountFor(Api.coinVideo)),
          bvid: bvid,
          multiply: multiply,
          selectLike: selectLike,
        ),
      );
    } catch (e, st) {
      return _crashed('投币', e, st);
    }
  }

  /// 首页「不感兴趣」反馈。[reasonId] 与 [feedbackId] 二选一。
  static Future<LoadingState<void>> feedDislike({
    required String goto,
    required int id,
    int? reasonId,
    int? feedbackId,
  }) => _feedDislike(
    goto: goto,
    id: id,
    reasonId: reasonId,
    feedbackId: feedbackId,
    cancel: false,
  );

  /// 撤销「不感兴趣」。
  static Future<LoadingState<void>> feedDislikeCancel({
    required String goto,
    required int id,
    int? reasonId,
    int? feedbackId,
  }) => _feedDislike(
    goto: goto,
    id: id,
    reasonId: reasonId,
    feedbackId: feedbackId,
    cancel: true,
  );

  static Future<LoadingState<void>> _feedDislike({
    required String goto,
    required int id,
    required int? reasonId,
    required int? feedbackId,
    required bool cancel,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final endpoint = cancel ? Api.feedDislikeCancel : Api.feedDislike;
    final account = RustRequest.accountFor(endpoint);
    if (account.accessKey.isNullOrEmpty) return const Error(_needRelogin);
    try {
      return _unwrapWrite(
        await rust_api.feedDislike(
          context: _appContext(account),
          goto: goto,
          id: id,
          reasonId: reasonId,
          feedbackId: feedbackId,
          cancel: cancel,
        ),
      );
    } catch (e, st) {
      return _crashed('不感兴趣反馈', e, st);
    }
  }

  // ------------------------------------------------------------ web 端

  /// UGC 一键三连。
  ///
  /// 返回的是三件事**各自的回执**：三连在协议上不是原子操作，部分成功是
  /// 正常结果。基线也是这么消费的。
  static Future<LoadingState<rust_api.UgcTripleResult>> ugcTriple({
    required String bvid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.ugcTriple(
        context: await _webContextWithHeaders(Api.ugcTriple, {
          'origin': HttpString.baseUrl,
          'referer': '${HttpString.baseUrl}/video/$bvid',
          'user-agent': BrowserUa.pc,
        }),
        bvid: bvid,
      );
      if (result.errorKind != null) {
        return Error(result.errorDetail ?? '三连失败', code: result.errorCode);
      }
      return Success(result);
    } catch (e, st) {
      return _crashed('一键三连', e, st);
    }
  }

  /// PGC 一键三连。
  static Future<LoadingState<rust_api.PgcTripleResult>> pgcTriple({
    required int epId,
    int? seasonId,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.pgcTriple(
        context: await _webContextWithHeaders(Api.pgcTriple, {
          'origin': HttpString.baseUrl,
          'referer':
              '${HttpString.baseUrl}/bangumi/play/'
              '${seasonId == null ? "ep$epId" : "ss$seasonId"}',
          'user-agent': BrowserUa.pc,
        }),
        epId: epId,
      );
      if (result.errorKind != null) {
        return Error(result.errorDetail ?? '三连失败', code: result.errorCode);
      }
      return Success(result);
    } catch (e, st) {
      return _crashed('一键三连', e, st);
    }
  }

  /// 操作用户关系：关注 1 / 取关 2 / 拉黑 5 / 取消拉黑 6 / 移除粉丝 7。
  ///
  /// 未登录时的拉黑**不发请求**：基线就只写本地黑名单，按 ADR-013 决策三
  /// 第 1 条属于平台存储，留在宿主。
  static Future<LoadingState<void>> relationMod({
    required int mid,
    required int act,
    required int reSrc,
  }) async {
    const int block = 5;
    const int unblock = 6;
    final isBlock = act == block;
    if (isBlock && !Accounts.main.isLogin) {
      Pref.setBlackMid(mid);
      return const Success(null);
    }
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.relationMod(
        context: await _webContextWithHeaders(Api.relationMod, {
          'origin': HttpString.spaceBaseUrl,
          'referer': '${HttpString.spaceBaseUrl}/$mid/dynamic',
          'user-agent': BrowserUa.pc,
        }),
        mid: mid,
        act: act,
        reSrc: reSrc,
        fp: BrowserUa.pc,
      );
      if (result.errorKind != null) {
        return Error(result.errorDetail ?? '操作失败', code: result.errorCode);
      }
      // 先确认远端成功，再改本地（ADR-013 决策八）。
      if (isBlock) {
        Pref.setBlackMid(mid);
      } else if (act == unblock) {
        Pref.removeBlackMid(mid);
      }
      return const Success(null);
    } catch (e, st) {
      return _crashed('修改用户关系', e, st);
    }
  }

  /// 删除一条评论。成功后才清本地缓存。
  static Future<LoadingState<void>> replyDel({
    required int type,
    required int oid,
    required int rpid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.replyDel(
        context: await _webContext(Api.replyDel),
        replyType: type,
        oid: oid,
        rpid: rpid,
      );
      if (result.errorKind != null) {
        return Error(result.errorDetail ?? '删除失败', code: result.errorCode);
      }
      GStorage.reply?.delete(rpid.toString());
      return const Success(null);
    } catch (e, st) {
      return _crashed('删除评论', e, st);
    }
  }

  /// 追番 / 追剧：加入。[seasonId] 为空时不发请求。
  static Future<LoadingState<String>> pgcAdd({int? seasonId}) => _season(
    Api.pgcAdd,
    (ctx) => rust_api.pgcAdd(context: ctx, seasonId: seasonId),
  );

  /// 追番 / 追剧：取消。
  static Future<LoadingState<String>> pgcDel({int? seasonId}) => _season(
    Api.pgcDel,
    (ctx) => rust_api.pgcDel(context: ctx, seasonId: seasonId),
  );

  /// 追番状态改为「想看 1 / 在看 2 / 看过 3」。
  ///
  /// [seasonIds] 是**逗号分隔的一串** id：追番列表页会一次改掉勾选的
  /// 若干季。基线的参数类型是 String 正是为此，不是写漏了。
  static Future<LoadingState<String>> pgcUpdate({
    required String seasonIds,
    required int status,
  }) => _season(
    Api.pgcUpdate,
    (ctx) =>
        rust_api.pgcUpdate(context: ctx, seasonIds: seasonIds, status: status),
  );

  static Future<LoadingState<String>> _season(
    String endpoint,
    Future<rust_api.ToastResult> Function(rust_api.VideoWriteContext) call,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await call(await _webContext(endpoint));
      if (result.errorKind != null) {
        return Error(result.errorDetail ?? '操作失败', code: result.errorCode);
      }
      return Success(result.toast ?? '操作成功');
    } catch (e, st) {
      return _crashed('追番', e, st);
    }
  }

  // ------------------------------------------------------------ 上报类
  //
  // 四条都是 `Future<void>`：基线同样不看它们的结果，失败也不该打扰用户
  // （`AccountManager.toast` 的 skipShow 里就列着 heartbeat 与
  // history/report）。它们一律走 `silent: true`，不重试。
  //
  // ## 标识符参数为什么是 `Object` 而不是 `int`
  //
  // 基线这几条的签名就是 `Object`，而调用点真的会传非 int：
  // `VideoDetailController.updateMediaListHistory` 传的
  // `args['mediaId']` 来自路由参数，有的入口放进去的是 `item.id`（int），
  // 有的是 `item.id.toString()`（String）。收紧成 `int` 不会在编译期
  // 报出来——那几个值是 `dynamic`——只会在**某一个入口**上变成运行时
  // TypeError，而这是一条用户看不见的后台上报。
  //
  // 所以这里照收 `Object`，由 [_asInt] 统一折算；折不出数字就不发
  // 请求（一条 oid 不是数字的历史记录本来也写不进去）。

  /// 历史记录上报。
  static Future<void> historyReport({
    required Object aid,
    required Object type,
  }) async {
    if (!RustCore.isReady) return;
    final (id, kind) = (_asInt(aid), _asInt(type));
    if (id == null || kind == null) return;
    try {
      await rust_api.historyReport(
        context: await _webContext(Api.historyReport, silent: true),
        aid: id,
        kind: kind,
      );
    } catch (e, st) {
      _ignored('历史上报', e, st);
    }
  }

  /// 播放进度心跳。
  ///
  /// **未登录直接不发**：基线的拦截器会把这条请求 reject 掉
  /// （`account_mgr.dart`）。那个判断要的是账号状态，留在宿主。
  static Future<void> heartBeat({
    Object? aid,
    Object? bvid,
    required Object cid,
    required int progress,
    Object? epid,
    Object? seasonId,
    Object? subType,
    required VideoType videoType,
  }) async {
    if (!RustCore.isReady) return;
    if (!RustRequest.accountFor(Api.heartBeat).isLogin) return;
    final id = _asInt(cid);
    if (id == null) return;
    try {
      await rust_api.heartBeat(
        context: await _webContext(Api.heartBeat, silent: true),
        aid: _asInt(aid),
        bvid: bvid?.toString(),
        cid: id,
        epid: _asInt(epid),
        seasonId: _asInt(seasonId),
        subType: _asInt(subType),
        videoType: videoType.name,
        playedTime: progress,
      );
    } catch (e, st) {
      _ignored('播放心跳', e, st);
    }
  }

  /// 播放列表（medialist）的历史上报。
  static Future<void> medialistHistory({
    required int desc,
    required Object oid,
    required Object upperMid,
  }) async {
    if (!RustCore.isReady) return;
    final (id, mid) = (_asInt(oid), _asInt(upperMid));
    if (id == null || mid == null) return;
    try {
      await rust_api.mediaListHistory(
        context: await _webContext(Api.mediaListHistory, silent: true),
        desc: desc,
        oid: id,
        upperMid: mid,
      );
    } catch (e, st) {
      _ignored('播放列表历史上报', e, st);
    }
  }

  /// 进入直播间的上报。挂在直播主机上，所以要单独给 base_url。
  static Future<void> roomEntryAction({required Object roomId}) async {
    if (!RustCore.isReady) return;
    final id = _asInt(roomId);
    if (id == null) return;
    try {
      await rust_api.roomEntryAction(
        context: await _webContext(
          Api.roomEntryAction,
          baseUrl: HttpString.liveBaseUrl,
          silent: true,
        ),
        roomId: id,
      );
    } catch (e, st) {
      _ignored('进直播间上报', e, st);
    }
  }

  // -------------------------------------------------------------- 内部

  /// 把上报类接口那些 `Object` 标识符折算成数字。见「上报类」那段注释。
  static int? _asInt(Object? value) => switch (value) {
    null => null,
    final int v => v,
    final num v => v.toInt(),
    _ => int.tryParse('$value'),
  };

  static LoadingState<void> _unwrapWrite(WriteResult result) =>
      result.errorKind == null
      ? const Success(null)
      : Error(result.errorDetail ?? '操作失败', code: result.errorCode);

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what出错: $e\n$st');
    return Error('$e');
  }

  /// 上报类的失败。只记日志——基线同样不打扰用户。
  static void _ignored(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what出错（已忽略）: $e\n$st');
  }
}
