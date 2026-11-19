import 'package:kilikili/grpc/bilibili/main/community/reply/v1.pb.dart';
import 'package:kilikili/grpc/bilibili/pagination.pb.dart';
import 'package:kilikili/grpc/grpc_req.dart';
import 'package:kilikili/grpc/url.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_reply_filter.dart';
import 'package:kilikili/utils/global_data.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:fixnum/fixnum.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;

abstract final class ReplyGrpc {
  static bool antiGoodsReply = Pref.antiGoodsReply;
  static var blockAtOnlyReply = Pref.blockAtOnlyReply;
  static var blockReplyAtCount = Pref.blockReplyAtCount;
  static RegExp replyRegExp = RegExp(
    Pref.banWordForReply,
    caseSensitive: false,
  );
  static bool enableFilter = replyRegExp.pattern.isNotEmpty;

  // static Future replyInfo({required int rpid}) {
  //   return _request(
  //     GrpcUrl.replyInfo,
  //     ReplyInfoReq(rpid: Int64(rpid)),
  //     ReplyInfoReply.fromBuffer,
  //     onSuccess: (response) => response.reply,
  //   );
  // }

  /// 过滤规则的取值。真相源留在这里的静态字段（设置页与长按菜单就地改写
  /// 它们，改 `Pref` 要等下次启动才一致），每次判定按请求传给 Rust。
  static ReplyFilterSettings _settings() => ReplyFilterSettings(
    blockedMids: Int64List.fromList(GlobalData().blackMids.toList()),
    antiGoods: antiGoodsReply,
    blockAtOnly: blockAtOnlyReply,
    blockAtCount: blockReplyAtCount,
    // 空串即关闭，与 `enableFilter` 同义；Rust 侧按同一个判据。
    banWord: replyRegExp.pattern,
  );

  /// 规则无法应用时的文案。
  ///
  /// **刻意让整个响应失败**，而不是当成「没有要过滤的」继续：关键词正则
  /// 在两端语义不同（如 `\d`）或 Rust 编不过（如 lookahead）时，放行意味着
  /// 用户明确屏蔽过的评论照常显示且没有任何提示。少看一页可以重试，看到
  /// 屏蔽过的内容不能撤回（ADR-010 决策二）。
  static const String _filterUnavailable =
      '评论过滤规则无法应用（关键词正则不被支持，或 Rust Core 未就绪），'
      '已停止加载以免漏过被屏蔽的评论';

  /// 一次判定要覆盖的全部评论：根评论、UP 置顶，以及它们各自的楼中楼。
  ///
  /// 与随后的遍历共用这一个函数，保证「送去判定的」与「要查结果的」是
  /// 同一批对象。
  static List<ReplyInfo> _collectMainList(MainListReply response) {
    final all = <ReplyInfo>[];
    if (response.hasUpTop()) {
      all
        ..add(response.upTop)
        ..addAll(response.upTop.replies);
    }
    for (final item in response.replies) {
      all
        ..add(item)
        ..addAll(item.replies);
    }
    return all;
  }

  static Future<LoadingState<MainListReply>> mainList({
    required int type,
    required int oid,
    Int64? rpid,
    required Mode mode,
    required String? offset,
    // Int64? cursorNext,
  }) async {
    final res = await GrpcReq.request(
      GrpcUrl.mainList,
      MainListReq(
        oid: Int64(oid),
        type: Int64(type),
        rpid: rpid,
        // cursor: CursorReq(
        //   mode: mode,
        //   next: cursorNext,
        // ),
        mode: mode,
        pagination: offset == null ? null : FeedPagination(offset: offset),
      ),
      MainListReply.fromBuffer,
    );
    if (res case Success(:final response)) {
      // keyword filter
      final verdict = RustReplyFilter.judge(
        _settings(),
        _collectMainList(response),
      );
      if (verdict == null) return const Error(_filterUnavailable);

      if (response.hasUpTop()) {
        final topReply = response.upTop;
        if (verdict.removes(topReply)) {
          response.clearUpTop();
        } else {
          topReply.replies.removeWhere(verdict.removes);
        }
      }

      if (response.replies.isNotEmpty) {
        response.replies.removeWhere((item) {
          final hasMatch = verdict.removes(item);
          if (!hasMatch && item.replies.isNotEmpty) {
            item.replies.removeWhere(verdict.removes);
          }
          return hasMatch;
        });
      }

      if (response.replies.isEmpty && !response.hasUpTop()) {
        final offset = response.paginationReply.nextOffset;
        if (offset.isNotEmpty && !response.cursor.isEnd) {
          return mainList(
            type: type,
            oid: oid,
            rpid: rpid,
            mode: mode,
            offset: offset,
          );
        }
      }
    }
    return res;
  }

  static Future<LoadingState<DetailListReply>> detailList({
    required int type,
    required int oid,
    required int root,
    required int rpid,
    required Mode mode,
    required String? offset,
    Map<Int64, ReplyInfo>? removed,
  }) async {
    final res = await GrpcReq.request(
      GrpcUrl.detailList,
      DetailListReq(
        oid: Int64(oid),
        type: Int64(type),
        root: Int64(root),
        rpid: Int64(rpid),
        scene: DetailListScene.REPLY,
        mode: mode,
        pagination: offset == null ? null : FeedPagination(offset: offset),
      ),
      DetailListReply.fromBuffer,
    );
    final data = res.dataOrNull;
    if (data != null) {
      final replies = data.root.replies;
      final verdict = RustReplyFilter.judge(_settings(), replies.toList());
      if (verdict == null) return const Error(_filterUnavailable);

      replies.removeWhere(
        removed == null
            ? verdict.removes
            : (i) {
                final flag = verdict.removes(i);
                if (flag) removed[i.id] = i;
                return flag;
              },
      );
    }
    return res;
  }

  static Future<LoadingState<FoldListResp>> foldList({
    required int type,
    required int oid,
    required FeedPagination pagination,
  }) async {
    final res = await GrpcReq.request(
      GrpcUrl.foldList,
      FoldListReq(
        oid: Int64(oid),
        type: Int64(type),
        pagination: pagination,
      ),
      FoldListResp.fromBuffer,
    );
    return _filterReplies(res, (data) => data.replies);
  }

  static Future<LoadingState<DialogListReply>> dialogList({
    required int type,
    required int oid,
    required int root,
    required int dialog,
    required String? offset,
  }) async {
    final res = await GrpcReq.request(
      GrpcUrl.dialogList,
      DialogListReq(
        oid: Int64(oid),
        type: Int64(type),
        root: Int64(root),
        dialog: Int64(dialog),
        pagination: offset == null ? null : FeedPagination(offset: offset),
      ),
      DialogListReply.fromBuffer,
    );
    return _filterReplies(res, (data) => data.replies);
  }

  /// 就地过滤一个响应里的评论列表；规则不可用时整条降级成 `Error`。
  static LoadingState<T> _filterReplies<T>(
    LoadingState<T> res,
    List<ReplyInfo> Function(T data) pick,
  ) {
    final data = res.dataOrNull;
    if (data == null) return res;

    final replies = pick(data);
    final verdict = RustReplyFilter.judge(_settings(), replies.toList());
    if (verdict == null) return const Error(_filterUnavailable);

    replies.removeWhere(verdict.removes);
    return res;
  }

  static Future<LoadingState<SearchItemReply>> searchItem({
    required int page,
    required SearchItemType itemType,
    required int oid,
    required int type,
    String? keyword,
  }) {
    return GrpcReq.request(
      GrpcUrl.searchItem,
      SearchItemReq(
        cursor: SearchItemCursorReq(
          next: Int64(page),
          itemType: itemType,
        ),
        oid: Int64(oid),
        type: Int64(type),
        keyword: keyword,
      ),
      SearchItemReply.fromBuffer,
    );
  }

  static Future<LoadingState<TranslateReplyResp>> translateReply({
    required Int64 type,
    required Int64 oid,
    required Int64 rpid,
  }) {
    return GrpcReq.request(
      GrpcUrl.translateReply,
      TranslateReplyReq(
        type: type,
        oid: oid,
        rpids: [rpid],
      ),
      TranslateReplyResp.fromBuffer,
    );
  }
}
