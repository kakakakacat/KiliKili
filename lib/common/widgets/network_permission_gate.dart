import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:kilikili/common/widgets/loading_widget/http_error.dart';
import 'package:kilikili/utils/network_permission.dart';
import 'package:kilikili/utils/permission_handler.dart' show openAppSettings;
import 'package:material_ui/material_ui.dart';

/// iOS 启动门：拿到联网授权之前不构建 [child]，也就不会加载任何页面。
///
/// 国行 iPhone 首次联网会弹「允许使用无线数据？」。弹框期间发出的请求全部
/// 失败，而首页此前不等授权就开始加载，点「允许」后只剩一屏错误。现在：
///
/// - 授权状态为「允许」：直接放行；
/// - 其余情况发一个探测请求（同时也是它触发系统弹框）：能连通就放行，
///   覆盖只允许 WLAN、或机型根本没有这项限制（状态恒为未知）的情况；
/// - 连不通就停在本页：未决定时显示等待，被拒绝时给「去设置」。授权变化
///   和从设置页回来都会重新检查。
///
/// 放行状态记在 [NetworkPermission]，整个进程内只拦一次，启动早期不经过
/// 页面的请求也靠它等待。其他平台直接返回 [child]。
class NetworkPermissionGate extends StatefulWidget {
  const NetworkPermissionGate({super.key, required this.child});

  final Widget child;

  @override
  State<NetworkPermissionGate> createState() => _NetworkPermissionGateState();
}

class _NetworkPermissionGateState extends State<NetworkPermissionGate> {
  static const _channel = MethodChannel(
    'io.github.kakacat.kilikili/network_permission',
  );

  /// 探测地址：只看能否连通，不关心响应内容。
  static final _probeUri = Uri.https('www.bilibili.com');
  static const _probeTimeout = Duration(seconds: 5);

  late bool _ready = NetworkPermission.isReady;

  /// `allowed` / `restricted` / `unknown`；拿到第一次状态前为 null，
  /// 此时只画空白，避免已授权的用户每次启动都闪一下等待页。
  String? _state;
  bool _probing = false;
  AppLifecycleListener? _lifecycle;

  @override
  void initState() {
    super.initState();
    if (_ready) return;
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'onChanged') _onState(call.arguments as String);
    });
    _channel
        .invokeMethod<String>('state')
        .then(
          (state) => _onState(state ?? 'unknown'),
          // 原生侧不可用时不能把用户永远挡在门外。
          onError: (_) => _pass(),
        );
    _lifecycle = AppLifecycleListener(onResume: _probe);
  }

  void _onState(String state) {
    if (_ready || !mounted) return;
    setState(() => _state = state);
    if (state == 'allowed') {
      _pass();
    } else {
      _probe();
    }
  }

  Future<void> _probe() async {
    if (_ready || _probing) return;
    _probing = true;
    final client = HttpClient()..connectionTimeout = _probeTimeout;
    try {
      final request = await client.headUrl(_probeUri);
      final response = await request.close().timeout(_probeTimeout);
      await response.drain<void>();
      _pass();
    } catch (_) {
      // 连不通：停在本页，等授权变化或回到前台再试。
    } finally {
      client.close(force: true);
      _probing = false;
    }
  }

  void _pass() {
    if (_ready) return;
    NetworkPermission.markReady();
    _channel.setMethodCallHandler(null);
    _lifecycle?.dispose();
    _lifecycle = null;
    if (mounted) setState(() => _ready = true);
  }

  @override
  void dispose() {
    if (!_ready) _channel.setMethodCallHandler(null);
    _lifecycle?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_ready) return widget.child;
    final skip = TextButton(onPressed: _pass, child: const Text('仍然继续'));
    return Scaffold(
      body: switch (_state) {
        null => const SizedBox.shrink(),
        'restricted' => Center(
          child: Column(
            mainAxisSize: .min,
            children: [
              const HttpError(
                isSliver: false,
                safeArea: false,
                errMsg: 'KiliKili 没有联网权限\n请在「设置 → KiliKili → 无线数据」中允许',
                btnText: '去设置',
                onReload: openAppSettings,
              ),
              skip,
            ],
          ),
        ),
        _ => Center(
          child: Column(
            mainAxisSize: .min,
            children: [
              const CircularProgressIndicator(),
              const SizedBox(height: 24),
              Text('等待联网授权…', style: Theme.of(context).textTheme.titleSmall),
              const SizedBox(height: 8),
              skip,
            ],
          ),
        ),
      },
    );
  }
}
