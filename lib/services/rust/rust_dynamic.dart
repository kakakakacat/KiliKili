import 'package:kilikili/common/constants.dart';
import 'package:flutter/foundation.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/dynamic.dart' as api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_grpc.dart';
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:kilikili/utils/accounts/account.dart';

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

  // ======================================== S5 续单第一批（issue #150）

  /// 本批几条只读接口的上下文。与 [_writeContext] 分开：它们一个 csrf
  /// 都不要，合用一个等于每个调用点都要答一遍「这里要不要传 csrf」。
  static Future<api.DynamicReadContext> _readContext(
    String endpoint, {
    String? baseUrl,
    String mixinKey = '',
    bool app = false,
    Account? account,
  }) async {
    final selectedAccount = account ?? RustRequest.accountFor(endpoint);
    return api.DynamicReadContext(
      baseUrl: baseUrl ?? HttpString.apiBaseUrl,
      cookie: app
          ? null
          : await RustRequest.cookieFor(selectedAccount, endpoint),
      headers: RustRequest.headersFor(selectedAccount),
      timeoutMs: BigInt.from(10000),
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

  /// 专栏阅读页的计数与收藏态。
  static Future<LoadingState<api.ArticleInfoData>> articleInfo({
    required String cvId,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await api.articleInfo(
        context: await _readContext(Api.articleInfo, mixinKey: mixinKey),
        cvId: cvId,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('专栏信息', e, st);
    }
  }

  /// 文集。
  static Future<LoadingState<api.ArticleListData>> articleList({
    required String id,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await api.articleList(
        context: await _readContext(Api.articleList),
        id: id,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('文集', e, st);
    }
  }

  /// 话题页顶部。**走 app 网关**，与其余几条不同域名。
  static Future<LoadingState<api.TopicTopData>> topicTop({
    required String topicId,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final account = RustRequest.accountFor(Api.topicTop);
      final result = await api.topicTop(
        context: await _readContext(
          Api.topicTop,
          baseUrl: HttpString.appBaseUrl,
          app: true,
          account: account,
        ),
        topicId: topicId,
        accessKey: account.accessKey,
        appkey: Constants.appKey,
        appsec: Constants.appSec,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('话题顶部', e, st);
    }
  }

  /// 推荐话题。
  static Future<LoadingState<List<api.TopicItemData>>> topicRcmd({
    int pageSize = 25,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await api.topicRcmd(
        context: await _readContext(Api.dynTopicRcmd),
        pageSize: pageSize,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('推荐话题', e, st);
    }
  }

  /// 动态页顶部的部落气泡。
  static Future<LoadingState<api.BubbleData>> bubble({
    required String tribeId,
    String? categoryId,
    int? sortType,
    required int page,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await api.bubble(
        context: await _readContext(Api.bubble),
        tribeId: tribeId,
        categoryId: categoryId,
        sortType: sortType,
        page: page,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('部落气泡', e, st);
    }
  }

  /// 发布页的话题搜索。**走 app 网关。**
  ///
  /// 本属搜索那一族，拉进本批是因为它是 `TopicItem` 的第二个生产者，
  /// 见 Rust 侧注释（#150 / ADR-011 补充十二）。
  static Future<LoadingState<api.TopicPubSearchData>> topicPubSearch({
    required String keywords,
    String content = '',
    required int pageNum,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final account = RustRequest.accountFor(Api.topicPubSearch);
      final result = await api.topicPubSearch(
        context: await _readContext(
          Api.topicPubSearch,
          baseUrl: HttpString.appBaseUrl,
          app: true,
          account: account,
        ),
        keywords: keywords,
        content: content,
        pageNum: pageNum,
        accessKey: account.accessKey,
        appkey: Constants.appKey,
        appsec: Constants.appSec,
      );
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      return _crashed('话题搜索', e, st);
    }
  }

  static const String _notReady = 'Rust Core 未就绪';

  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '请求失败', code: code) : Success(data);

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what 出错: $e\n$st');
    return Error('$e');
  }
}
