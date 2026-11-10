import 'package:PiliPlus/rust/api/transport.dart';
import 'package:PiliPlus/utils/storage_pref.dart';

/// 把基线 Dio 的传输开关折算成 Rust 取数路径的传输设置（issue #25）。
///
/// 这些开关的真相源只有一处：用户在设置页改的 `Pref`。Rust 侧不读配置、
/// 不缓存，每次请求带上当次的值——与 Cookie 的处理是同一条原则，避免
/// 「设置改了而另一条路径还用旧值」。
///
/// 未对齐的部分（HTTP/2 连接池在切网后的重建、`RetryInterceptor` 手工
/// 处理的 3xx）见 `rust/crates/bili-api/src/transport_reqwest.rs` 的模块
/// 文档：reqwest 自带的行为在只读接口上与之等价。
abstract final class RustTransport {
  /// 读取当前设置。调用点在一次网络请求之前，几次 Hive 读取可忽略。
  static TransportSettings current() {
    // 基线：开了系统代理且 host/port 都有效时才真正走代理
    // （`lib/http/init.dart::_createPool`）。半截配置按直连处理。
    final useProxy = Pref.enableSystemProxy;
    final host = useProxy ? Pref.systemProxyHost : '';
    final port = useProxy ? int.tryParse(Pref.systemProxyPort) : null;
    final proxyOk = host.isNotEmpty && port != null && port > 0 && port < 65536;

    return TransportSettings(
      retryCount: Pref.retryCount < 0 ? 0 : Pref.retryCount,
      retryDelayMs: BigInt.from(Pref.retryDelay),
      proxyHost: proxyOk ? host : null,
      proxyPort: proxyOk ? port : null,
      // 基线在走系统代理时无条件接受无效证书（抓包场景），另有一个独立
      // 开关。两者取或，与 `_createPool` 里的分支一致。
      acceptInvalidCerts: proxyOk || Pref.badCertificateCallback,
      // 基线默认不开 HTTP/2；reqwest 默认经 ALPN 协商 h2。不显式对齐的话
      // Rust 路径会在用户没开 HTTP/2 时偷偷用 h2。
      http1Only: !Pref.enableHttp2,
    );
  }
}
