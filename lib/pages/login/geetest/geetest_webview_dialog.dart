import 'dart:async' show Timer;
import 'dart:convert' show jsonDecode;
import 'dart:io' show Platform;

import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/main.dart';
import 'package:kilikili/pages/login/geetest/geetest_html.dart';
import 'package:kilikili/plugin/linux_webview.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class GeetestWebviewDialog extends StatefulWidget {
  const GeetestWebviewDialog(this.gt, this.challenge, {super.key});

  final String gt;
  final String challenge;

  @override
  State<GeetestWebviewDialog> createState() => _GeetestWebviewDialogState();

  static Future<Map<String, dynamic>?> geetest(String gt, String challenge) {
    return showDialog<Map<String, dynamic>>(
      context: Get.context!,
      builder: (context) => GeetestWebviewDialog(gt, challenge),
    );
  }
}

class _GeetestWebviewDialogState extends State<GeetestWebviewDialog> {
  late final String _html;
  Timer? _loadTimer;
  bool _loading = true;
  bool _finished = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    // 重建状态提示不能生成新页面，否则同一 challenge 会被再次初始化。
    _html = buildGeetestHtml(
      widget.gt,
      widget.challenge,
      linux: Platform.isLinux,
    );
    _loadTimer = Timer(const Duration(seconds: 30), () {
      _fail('验证码加载超时，请关闭后重试或使用扫码登录');
    });
  }

  @override
  void dispose() {
    _loadTimer?.cancel();
    _finished = true;
    super.dispose();
  }

  void _ready() {
    if (!mounted || _finished || _error != null) return;
    _loadTimer?.cancel();
    setState(() => _loading = false);
  }

  void _fail([String message = '验证码加载失败，请关闭后重试或使用扫码登录']) {
    if (!mounted || _finished || _error != null) return;
    _loadTimer?.cancel();
    setState(() {
      _loading = false;
      _error = message;
    });
  }

  void _finish([Map<String, dynamic>? data]) {
    if (!mounted || _finished || ModalRoute.of(context)?.isCurrent != true) {
      return;
    }
    // 错误后迟到的成功回调不能继续发送短信；关闭操作仍允许。
    if (data != null && _error != null) return;
    _finished = true;
    _loadTimer?.cancel();
    Navigator.of(context).pop(data);
  }

  void _success(Object? data) {
    if (data is Map<String, dynamic> &&
        ['geetest_validate', 'geetest_seccode', 'geetest_challenge'].every(
          (key) => data[key] is String && (data[key] as String).isNotEmpty,
        )) {
      _finish(data);
    } else {
      _fail('验证结果不完整，请关闭后重试');
    }
  }

  Widget _status() => Positioned.fill(
    child: Material(
      color: Theme.of(context).colorScheme.surface,
      child: Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: _error != null
              ? Text(_error!, textAlign: TextAlign.center)
              : const Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    CircularProgressIndicator(),
                    SizedBox(height: 16),
                    Text('正在加载验证码…'),
                  ],
                ),
        ),
      ),
    ),
  );

  @override
  Widget build(BuildContext context) {
    if (Platform.isLinux) {
      return AlertDialog(
        title: const Text('验证码'),
        actions: [TextButton(onPressed: _finish, child: const Text('关闭'))],
        content: SizedBox(
          width: 300,
          height: 400,
          child: Stack(
            children: [
              LinuxWebview(
                initialHtml: _html,
                userAgent: BrowserUa.mob,
                incognito: true,
                onWebMessageReceived: (msg) {
                  final value = msg.toString();
                  final separator = value.indexOf(':');
                  if (separator < 0) return;
                  switch (value.substring(0, separator)) {
                    case 'success':
                      try {
                        _success(jsonDecode(value.substring(separator + 1)));
                      } catch (_) {
                        _fail('验证结果不完整，请关闭后重试');
                      }
                    case 'ready':
                      _ready();
                    case 'error':
                      _fail();
                    case 'close':
                      _finish();
                  }
                },
              ),
              if (_loading || _error != null) _status(),
            ],
          ),
        ),
      );
    }

    return Stack(
      children: [
        InAppWebView(
          webViewEnvironment: webViewEnvironment,
          initialSettings: InAppWebViewSettings(
            clearCache: true,
            javaScriptEnabled: true,
            forceDark: ForceDark.AUTO,
            useHybridComposition: true,
            algorithmicDarkeningAllowed: true,
            useShouldOverrideUrlLoading: true,
            userAgent: BrowserUa.mob,
            mixedContentMode: .MIXED_CONTENT_ALWAYS_ALLOW,

            incognito: true,
            allowFileAccess: false,
            allowsLinkPreview: false,
            allowContentAccess: false,
            useOnDownloadStart: false,
            geolocationEnabled: false,
            thirdPartyCookiesEnabled: false,
            enterpriseAuthenticationAppLinkPolicyEnabled: false,
            saveFormData: false,
            safeBrowsingEnabled: false,
            isFraudulentWebsiteWarningEnabled: false,
            domStorageEnabled: false,
            databaseEnabled: false,
            cacheEnabled: false,
            cacheMode: .LOAD_NO_CACHE,

            horizontalScrollBarEnabled: false,
            verticalScrollBarEnabled: false,
            overScrollMode: .NEVER,

            pageZoom: Platform.isIOS ? 3 : 1,
          ),
          initialData: InAppWebViewInitialData(data: _html),
          onWebViewCreated: (ctr) {
            ctr
              ..addJavaScriptHandler(
                handlerName: 'success',
                callback: (args) => _success(args.isEmpty ? null : args[0]),
              )
              ..addJavaScriptHandler(
                handlerName: 'ready',
                callback: (_) => _ready(),
              )
              ..addJavaScriptHandler(
                handlerName: 'error',
                callback: (_) => _fail(),
              )
              ..addJavaScriptHandler(
                handlerName: 'close',
                callback: (_) => _finish(),
              );
          },
          onReceivedError: (_, request, _) {
            if (request.isForMainFrame == true) _fail();
          },
          onReceivedHttpError: (_, request, _) {
            if (request.isForMainFrame == true) _fail();
          },
        ),
        if (_loading || _error != null) _status(),
        Positioned(
          left: 8,
          top: 8,
          child: IconButton(
            icon: const Icon(Icons.close),
            onPressed: _finish,
            tooltip: '关闭',
          ),
        ),
      ],
    );
  }
}
