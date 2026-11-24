import CoreTelephony
import Flutter

/// 读 App 的「无线数据」联网授权。
///
/// 国行 iPhone 首次联网会弹「允许使用无线数据？」，授权前的请求一律失败。
/// 这里只上报状态，是否放行由 Dart 侧的启动门决定（见
/// `lib/common/widgets/network_permission_gate.dart`）。
///
/// 状态：`allowed`（不受限）、`restricted`（受限，可能只允许 WLAN 或完全
/// 不允许）、`unknown`（尚未决定，或机型不受此限制）。
final class NetworkPermissionPlugin: NSObject, FlutterPlugin {
  private static let channelName = "io.github.kakacat.kilikili/network_permission"

  static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: channelName,
      binaryMessenger: registrar.messenger()
    )
    let instance = NetworkPermissionPlugin(channel: channel)
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  private let channel: FlutterMethodChannel
  private let cellularData = CTCellularData()

  private init(channel: FlutterMethodChannel) {
    self.channel = channel
    super.init()
    // 设置时会立即回调一次当前状态，之后每次用户改授权都会回调。
    cellularData.cellularDataRestrictionDidUpdateNotifier = { [weak self] state in
      DispatchQueue.main.async {
        self?.channel.invokeMethod("onChanged", arguments: Self.name(of: state))
      }
    }
  }

  func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "state":
      result(Self.name(of: cellularData.restrictedState))
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  private static func name(of state: CTCellularDataRestrictedState) -> String {
    switch state {
    case .notRestricted: return "allowed"
    case .restricted: return "restricted"
    default: return "unknown"
    }
  }
}
