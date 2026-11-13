import 'package:kilikili/rust/api/grpc.dart' as api;
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/utils.dart';

/// 尚未迁移的 gRPC 调用也共用 Rust 编码，避免两套设备头在后续切片中漂移。
// Device 的 Dart 生成类型仍被 dynamic/v2 → vas/garb 间接依赖，保留到 S5/S6
// 最后一个消费者迁完再删（#133）；本文件已经不再使用它编码 metadata。
abstract final class GrpcHeaders {
  static api.GrpcIdentity _identity({String? accessKey, String buvid = ''}) =>
      api.GrpcIdentity(
        accessKey: accessKey,
        buvid: buvid,
        sessionId: Utils.generateRandomString(8),
      );

  static String get fawkes => Map.fromEntries(
    api
        .buildGrpcHeaders(identity: _identity())
        .map((e) => MapEntry(e.$1, e.$2)),
  )['x-bili-fawkes-req-bin']!;

  static Map<String, String> newHeaders(Account account) => Map.fromEntries(
    api
        .buildGrpcHeaders(
          identity: _identity(
            accessKey: account.accessKey,
            buvid: account.buvid,
          ),
        )
        .map((e) => MapEntry(e.$1, e.$2)),
  );
}
