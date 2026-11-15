import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/rust/api/dynamic.dart' as api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_grpc.dart';
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';

/// 动态接口的 typed Rust 入口。gRPC 凭据只在请求时传入 Rust。
abstract final class RustDynamic {
  static Future<api.VoteInfoResult> voteInfo({required int voteId}) async {
    if (!RustCore.isReady) {
      return const api.VoteInfoResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.voteInfo(
        context: await _writeContext(Api.voteInfo),
        voteId: voteId,
      );
    } catch (_) {
      return const api.VoteInfoResult(
        errorKind: 'protocol',
        errorDetail: '投票信息请求未完成',
      );
    }
  }

  static Future<api.VoteInfoResult> doVote({
    required int voteId,
    required List<int> votes,
    required bool anonymous,
    int? dynamicId,
    int? voterUid,
  }) async {
    if (!RustCore.isReady) {
      return const api.VoteInfoResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.doVote(
        context: await _writeContext(Api.doVote),
        voteId: voteId,
        votes: votes,
        anonymous: anonymous,
        dynamicId: dynamicId,
        voterUid: voterUid,
      );
    } catch (_) {
      return const api.VoteInfoResult(
        errorKind: 'protocol',
        errorDetail: '投票提交请求未完成',
      );
    }
  }

  static Future<api.VoteIdResult> saveVote({
    required api.VoteInfoData info,
    required bool update,
  }) async {
    if (!RustCore.isReady) {
      return const api.VoteIdResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.saveVote(
        context: await _writeContext(update ? Api.updateVote : Api.createVote),
        info: info,
        update: update,
      );
    } catch (_) {
      return const api.VoteIdResult(
        errorKind: 'protocol',
        errorDetail: '投票保存请求未完成',
      );
    }
  }

  static Future<api.FolloweeVotesResult> followeeVotes({
    required int voteId,
  }) async {
    if (!RustCore.isReady) {
      return const api.FolloweeVotesResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.followeeVotes(
        context: await _writeContext(
          Api.followeeVotes,
          baseUrl: HttpString.tUrl,
        ),
        voteId: voteId,
      );
    } catch (_) {
      return const api.FolloweeVotesResult(
        errorKind: 'protocol',
        errorDetail: '关注者投票请求未完成',
      );
    }
  }

  static Future<api.ReserveInfoResult> reserveInfo({required int sid}) async {
    if (!RustCore.isReady) {
      return const api.ReserveInfoResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.reserveInfo(
        context: await _writeContext(Api.reserveInfo),
        sid: sid,
      );
    } catch (_) {
      return const api.ReserveInfoResult(
        errorKind: 'protocol',
        errorDetail: '预约信息请求未完成',
      );
    }
  }

  static Future<api.ReserveIdResult> createReserve({
    required int subType,
    required String title,
    required int start,
  }) async {
    if (!RustCore.isReady) {
      return const api.ReserveIdResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.createReserve(
        context: await _writeContext(Api.createReserve),
        subType: subType,
        title: title,
        start: start,
      );
    } catch (_) {
      return const api.ReserveIdResult(
        errorKind: 'protocol',
        errorDetail: '创建预约请求未完成',
      );
    }
  }

  static Future<api.ReserveIdResult> updateReserve({
    required int sid,
    required int subType,
    required String title,
    required int start,
  }) async {
    if (!RustCore.isReady) {
      return const api.ReserveIdResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.updateReserve(
        context: await _writeContext(Api.updateReserve),
        sid: sid,
        subType: subType,
        title: title,
        start: start,
      );
    } catch (_) {
      return const api.ReserveIdResult(
        errorKind: 'protocol',
        errorDetail: '更新预约请求未完成',
      );
    }
  }

  static Future<api.ReserveActionResult> dynReserve({
    required int? reserveId,
    required int? status,
    required String dynamicId,
    required int? total,
  }) async {
    if (!RustCore.isReady) {
      return const api.ReserveActionResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.dynReserve(
        context: await _writeContext(Api.dynReserve),
        reserveId: reserveId,
        status: status,
        dynamicId: dynamicId,
        total: total,
      );
    } catch (_) {
      return const api.ReserveActionResult(
        errorKind: 'protocol',
        errorDetail: '动态预约请求未完成',
      );
    }
  }

  static Future<WriteResult> privatePubSetting({
    required Object dynamicId,
    int? dynType,
    required String action,
  }) async {
    if (!RustCore.isReady) {
      return const WriteResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.privatePubSetting(
        context: await _writeContext(Api.dynPrivatePubSetting),
        dynamicId: dynamicId.toString(),
        dynType: dynType,
        action: action,
      );
    } catch (_) {
      return const WriteResult(
        errorKind: 'protocol',
        errorDetail: '动态私密设置请求未完成',
      );
    }
  }

  static Future<api.DynamicWriteContext> _writeContext(
    String endpoint, {
    bool referer = false,
    String? baseUrl,
  }) async {
    final account = RustRequest.accountFor(endpoint);
    return api.DynamicWriteContext(
      baseUrl: baseUrl ?? HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(account, endpoint),
      csrf: RustRequest.csrfFor(account),
      headers: [
        ...RustRequest.headersFor(account),
        if (referer) ('referer', HttpString.dynamicShareBaseUrl),
      ],
      timeoutMs: BigInt.from(10000),
      transport: RustTransport.current(),
    );
  }

  static Future<WriteResult> thumb({
    required String? dynamicId,
    required int up,
  }) async {
    if (!RustCore.isReady) {
      return const WriteResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    if (dynamicId == null) {
      return const WriteResult(errorKind: 'protocol', errorDetail: '动态 id 缺失');
    }
    try {
      return await api.thumbDynamic(
        context: await _writeContext(Api.thumbDynamic, referer: true),
        dynamicId: dynamicId,
        up: up,
      );
    } catch (_) {
      return const WriteResult(errorKind: 'protocol', errorDetail: '动态点赞请求未完成');
    }
  }

  static Future<WriteResult> top({
    required Object dynamicId,
    required bool setTop,
  }) async {
    if (!RustCore.isReady) {
      return const WriteResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪',
      );
    }
    try {
      return await api.topDynamic(
        context: await _writeContext(setTop ? Api.setTopDyn : Api.rmTopDyn),
        dynamicId: dynamicId.toString(),
        setTop: setTop,
      );
    } catch (_) {
      return const WriteResult(errorKind: 'protocol', errorDetail: '动态置顶请求未完成');
    }
  }

  static Future<api.LikeListResult> likeList({
    required String dynamicId,
    int? uidOffset,
    required int page,
  }) async {
    if (!RustCore.isReady) {
      return const api.LikeListResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪，动态点赞列表暂不可用',
      );
    }
    try {
      return await api.fetchLikeList(
        context: RustGrpc.context(),
        dynamicId: dynamicId,
        uidOffset: uidOffset,
        page: page,
      );
    } catch (_) {
      return const api.LikeListResult(
        errorKind: 'protocol',
        errorDetail: '动态点赞列表请求未完成',
      );
    }
  }

  static Future<api.DynRedResult> dynRed() async {
    if (!RustCore.isReady) {
      return const api.DynRedResult(
        errorKind: 'storage',
        errorDetail: 'Rust Core 未就绪，动态红点暂不可用',
      );
    }
    try {
      return await api.fetchDynRed(context: RustGrpc.context());
    } catch (_) {
      return const api.DynRedResult(
        errorKind: 'protocol',
        errorDetail: '动态红点请求未完成',
      );
    }
  }
}
