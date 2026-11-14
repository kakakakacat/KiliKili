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
  static Future<api.DynamicWriteContext> _writeContext(
    String endpoint, {
    bool referer = false,
  }) async {
    final account = RustRequest.accountFor(endpoint);
    return api.DynamicWriteContext(
      baseUrl: HttpString.apiBaseUrl,
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
