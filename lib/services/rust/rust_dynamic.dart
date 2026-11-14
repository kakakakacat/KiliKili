import 'package:kilikili/rust/api/dynamic.dart' as api;
import 'package:kilikili/services/rust/rust_grpc.dart';
import 'package:kilikili/services/rust_core.dart';

/// 动态接口的 typed Rust 入口。gRPC 凭据只在请求时传入 Rust。
abstract final class RustDynamic {
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
