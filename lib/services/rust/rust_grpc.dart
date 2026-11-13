import 'package:kilikili/http/constants.dart';
import 'package:kilikili/rust/api/grpc.dart' as api;
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/utils.dart';

/// gRPC 基线没有 ApiType 分账号映射，统一使用 main；不借用 REST 的取流账号。
abstract final class RustGrpc {
  static api.GrpcContext context() {
    final account = Accounts.main;
    return api.GrpcContext(
      baseUrl: HttpString.appBaseUrl,
      identity: api.GrpcIdentity(
        accessKey: account.accessKey,
        buvid: account.buvid,
        sessionId: Utils.generateRandomString(8),
      ),
      timeoutMs: BigInt.from(10000),
      transport: RustTransport.current(),
    );
  }
}
