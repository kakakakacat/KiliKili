import 'dart:typed_data';

import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/reply_grpc.dart' as rust_api;
import 'package:kilikili/services/rust/rust_grpc.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/global_data.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;

export 'package:kilikili/rust/api/reply_grpc.dart'
    show
        ReplyCard,
        ReplyContent,
        ReplyControlInfo,
        ReplyDetailPage,
        ReplyEmote,
        ReplyEntity,
        ReplyFlatPage,
        ReplyFoldCard,
        ReplyMainPage,
        ReplyMedal,
        ReplyNote,
        ReplyParentMember,
        ReplyPicture,
        ReplySearchItem,
        ReplySearchItemKind,
        ReplySearchList,
        ReplySortMode,
        ReplySubject,
        ReplyUrl,
        ReplyUser,
        ReplyVote,
        ReplyVoteCard,
        ReplyVoteOption;

/// 评论区 gRPC 取数的调用面（S2 续单 / issue #176）。
///
/// 基线 `lib/grpc/reply.dart` 的 `ReplyGrpc` 整体搬到 Rust：请求、解帧、
/// protobuf 解码、过滤、投影都在那边，这里只剩账号与设置的注入。
///
/// ## 没有降级开关
///
/// typed 直连删掉了 Dart 原实现（ADR-011 决策四），`lib/grpc/bilibili/
/// main/community/reply/**` 已随本切片删除，降级无处可去。`RustCore`
/// 未就绪时评论区整体不可用，表现为一条明确的错误而不是崩溃。
///
/// ## 过滤规则取值的真相源在这里
///
/// [`antiGoodsReply`] 等四个静态字段是真相源，不是 `Pref` 的缓存：设置页
/// 与长按菜单就地改写它们，改 `Pref` 要等下次启动才一致。每次请求把当下
/// 的取值递给 Rust。
///
/// 规则编不过（关键词正则在两端语义不同，如 `\d`，或 Rust 不支持
/// lookahead）时整条请求失败，`errorKind == 'unsupported'`。**不能当成
/// 「没有要过滤的」继续**：用户明确屏蔽过的评论照常显示且没有任何提示，
/// 比列表加载失败更糟（ADR-010 决策二，同 `RustVideoFeed`）。
abstract final class RustReplyGrpc {
  static bool antiGoodsReply = Pref.antiGoodsReply;
  static bool blockAtOnlyReply = Pref.blockAtOnlyReply;
  static int? blockReplyAtCount = Pref.blockReplyAtCount;
  static RegExp replyRegExp = RegExp(
    Pref.banWordForReply,
    caseSensitive: false,
  );
  static bool enableFilter = replyRegExp.pattern.isNotEmpty;

  static const String _notReady = 'Rust Core 未就绪，评论区暂不可用';

  /// 设置里的「过滤简介复读」开关。简介本身按稿件不同，由调用点随请求
  /// 传入（[mainList] 的 `intro`）——它不是全局状态，同时开着两个视频页
  /// 时存一份静态的必然串台。
  static final bool _filterIntro = Pref.filterIntro;

  static rust_api.ReplyFilterInput _filter({String? intro}) =>
      rust_api.ReplyFilterInput(
    blockedMids: Int64List.fromList(GlobalData().blackMids.toList()),
    antiGoods: antiGoodsReply,
    blockAtOnly: blockAtOnlyReply,
    blockAtCount: blockReplyAtCount,
    // 空串即关闭，与 `enableFilter` 同义；Rust 侧按同一个判据。
    banWord: replyRegExp.pattern,
    intro: _filterIntro ? intro : null,
  );

  /// 把 `XxxResult` 信封摊平成 [LoadingState]。
  static LoadingState<T> _state<T>(
    T? data,
    String? errorKind,
    String? errorDetail,
    int? errorCode,
  ) {
    if (data != null) return Success(data);
    return Error(
      errorKind == 'unsupported'
          ? '评论过滤规则无法应用（$errorDetail），已停止加载以免漏过被屏蔽的评论'
          : errorDetail,
      code: errorCode,
    );
  }

  /// 取一页根评论。
  ///
  /// `isRefresh` 为真时 Rust 会把 UP 置顶并进列表头部；翻页时置顶已经在
  /// 列表里了，再并一次会出现两条。
  static Future<LoadingState<rust_api.ReplyMainPage>> mainList({
    required int oid,
    required int type,
    int? rpid,
    required rust_api.ReplySortMode mode,
    required String? offset,
    required bool isRefresh,
    String? intro,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final res = await rust_api.fetchReplyMainList(
      context: RustGrpc.context(),
      filter: _filter(intro: intro),
      oid: oid,
      kind: type,
      rpid: rpid,
      mode: mode,
      offset: offset ?? '',
      isRefresh: isRefresh,
    );
    return _state(res.data, res.errorKind, res.errorDetail, res.errorCode);
  }

  /// 取一页楼中楼。
  static Future<LoadingState<rust_api.ReplyDetailPage>> detailList({
    required int oid,
    required int type,
    required int root,
    required int rpid,
    required rust_api.ReplySortMode mode,
    required String? offset,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final res = await rust_api.fetchReplyDetailList(
      context: RustGrpc.context(),
      filter: _filter(),
      oid: oid,
      kind: type,
      root: root,
      rpid: rpid,
      mode: mode,
      offset: offset ?? '',
    );
    return _state(res.data, res.errorKind, res.errorDetail, res.errorCode);
  }

  /// 取一页对话。
  static Future<LoadingState<rust_api.ReplyFlatPage>> dialogList({
    required int oid,
    required int type,
    required int root,
    required int dialog,
    required String? offset,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final res = await rust_api.fetchReplyDialogList(
      context: RustGrpc.context(),
      filter: _filter(),
      oid: oid,
      kind: type,
      root: root,
      dialog: dialog,
      offset: offset ?? '',
    );
    return _state(res.data, res.errorKind, res.errorDetail, res.errorCode);
  }

  /// 取一页折叠评论。`offset` 来自折叠卡，原样回填。
  static Future<LoadingState<rust_api.ReplyFlatPage>> foldList({
    required int oid,
    required int type,
    required String offset,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final res = await rust_api.fetchReplyFoldList(
      context: RustGrpc.context(),
      filter: _filter(),
      oid: oid,
      kind: type,
      offset: offset,
    );
    return _state(res.data, res.errorKind, res.errorDetail, res.errorCode);
  }

  /// 评论区里搜稿件 / 专栏（发评论时插链接用）。
  static Future<LoadingState<rust_api.ReplySearchList>> searchItem({
    required int oid,
    required int type,
    required int page,
    required rust_api.ReplySearchItemKind itemKind,
    required String keyword,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final res = await rust_api.fetchReplySearchItem(
      context: RustGrpc.context(),
      oid: oid,
      kind: type,
      page: page,
      itemKind: itemKind,
      keyword: keyword,
    );
    return _state(res.data, res.errorKind, res.errorDetail, res.errorCode);
  }

  /// 翻译一条评论。
  ///
  /// 成功但没有译文时返回 `Success(null)`——那是一种正常结果（基线此时
  /// toast「翻译结果为空」），不是失败。
  static Future<LoadingState<rust_api.ReplyContent?>> translateReply({
    required int oid,
    required int type,
    required int rpid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final res = await rust_api.fetchReplyTranslation(
      context: RustGrpc.context(),
      oid: oid,
      kind: type,
      rpid: rpid,
    );
    if (res.errorKind != null) {
      return Error(res.errorDetail, code: res.errorCode?.toInt());
    }
    return Success(res.data);
  }

  // ─────────────────────── 「我的评论」的本地存档 ───────────────────────
  //
  // 基线用 protobuf 生成类做这几件事（`writeToBuffer` / `fromBuffer` /
  // `toProto3Json` / `mergeFromProto3Json`）。pb 子树删掉之后没人实现了，
  // 而盒子里与用户导出的文件都是**已经存在的数据**，不能跟着一起消失。
  // 格式决策与两种老格式的读入见 `bili_api::reply_grpc_store`。

  /// 读一条本地保存的评论。读不出来返回 null（由调用点决定跳过还是报错）。
  static rust_api.ReplyEntity? decodeStored(List<int> bytes) {
    if (!RustCore.isReady) return null;
    return rust_api.decodeStoredReply(bytes: bytes).data;
  }

  /// 写一条本地保存的评论。
  static Uint8List? encodeStored(rust_api.ReplyEntity entity) {
    if (!RustCore.isReady) return null;
    return rust_api.encodeStoredReply(entity: entity).data;
  }

  /// 导出整盒评论。
  static String? exportStored(List<Uint8List> items) {
    if (!RustCore.isReady) return null;
    return rust_api.exportStoredReplies(items: items).data;
  }

  /// 导入一份导出文件，返回 `盒子的键 → 字节`。
  static Map<String, Uint8List>? importStored(String json) {
    if (!RustCore.isReady) return null;
    final entries = rust_api.importStoredReplies(json: json).data;
    if (entries == null) return null;
    return {for (final entry in entries) entry.key: entry.bytes};
  }

  /// 把 `/x/v2/reply/add` 返回的 `data.reply` 变成一条评论。
  ///
  /// 请求本身还在 `lib/http/video.dart`（写操作按 ADR-013 单独迁），
  /// 这里只接管字段映射。
  static rust_api.ReplyEntity? fromRestJson(String json) {
    if (!RustCore.isReady) return null;
    return rust_api.replyFromRestJson(json: json).data;
  }
}
