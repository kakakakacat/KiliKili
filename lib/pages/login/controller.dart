import 'dart:async';
import 'dart:io';

import 'package:kilikili/common/dial_prefix.dart';
import 'package:kilikili/common/widgets/button/icon_button.dart';
import 'package:kilikili/common/widgets/radio_widget.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/models/login/model.dart';
import 'package:kilikili/pages/login/geetest/geetest_webview_dialog.dart';
import 'package:kilikili/services/rust/rust_login.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/theme_utils.dart';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter/services.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class LoginPageController extends GetxController
    with GetSingleTickerProviderStateMixin {
  final TextEditingController telTextController = TextEditingController();
  final TextEditingController usernameTextController = TextEditingController();
  final TextEditingController passwordTextController = TextEditingController();
  final TextEditingController smsCodeTextController = TextEditingController();
  final TextEditingController cookieTextController = TextEditingController();

  late final codeInfo = LoadingState<QrCodeData>.loading().obs;

  late final TabController tabController;

  late final CaptchaDataModel captchaData = CaptchaDataModel();
  late final RxInt qrCodeLeftTime = 180.obs;
  late final RxString statusQRCode = ''.obs;

  late var selectedCountryCodeId = Login.dialPrefix.first;
  late String captchaKey = '';
  late final RxInt smsSendCooldown = 0.obs;
  late int smsSendTimestamp = 0;

  // 定时器
  Timer? qrCodeTimer;
  Timer? smsSendCooldownTimer;

  bool _isReq = false;
  bool _isVerifyingCaptcha = false;

  @override
  void onInit() {
    super.onInit();
    tabController = TabController(length: 4, vsync: this)
      ..addListener(_handleTabChange);
  }

  @override
  void onClose() {
    tabController
      ..removeListener(_handleTabChange)
      ..dispose();
    qrCodeTimer?.cancel();
    smsSendCooldownTimer?.cancel();
    telTextController.dispose();
    usernameTextController.dispose();
    passwordTextController.dispose();
    smsCodeTextController.dispose();
    cookieTextController.dispose();
    super.onClose();
  }

  Future<void> refreshQRCode() async {
    final res = await RustLogin.qrCode();
    if (res case Success(:final response)) {
      qrCodeTimer?.cancel();
      codeInfo.value = res;
      qrCodeTimer = Timer.periodic(const Duration(milliseconds: 1000), (t) {
        final left = 180 - t.tick;
        if (left <= 0) {
          t.cancel();
          statusQRCode.value = '二维码已过期，请刷新';
          qrCodeLeftTime.value = 0;
          return;
        }
        qrCodeLeftTime.value = left;
        if (_isReq || tabController.index != 2) return;

        _isReq = true;
        RustLogin.qrCodePoll(response.authCode).then((poll) async {
          _isReq = false;
          if (poll case Error(:final errMsg)) {
            // 轮询失败只是这一次的网络问题，不停表：二维码还在有效期内。
            statusQRCode.value = errMsg ?? '';
            return;
          }
          final state = poll.data;
          switch (state.status) {
            // 三种状态在 Rust 侧已经分好（含 86038 这个失效码），这里
            // 不再靠 code 判一次。
            case QrPollStatus.confirmed:
              final tokens = state.tokens;
              if (tokens == null) {
                statusQRCode.value = '扫码成功但未返回身份信息';
                return;
              }
              t.cancel();
              statusQRCode.value = '扫码成功';
              await setAccount(tokens);
              Get.back();
            case QrPollStatus.expired:
              t.cancel();
              qrCodeLeftTime.value = 0;
            case QrPollStatus.pending:
              statusQRCode.value = state.message ?? '';
          }
        });
      });
    }
  }

  void _handleTabChange() {
    if (tabController.index == 2) {
      if (qrCodeTimer == null || !qrCodeTimer!.isActive) {
        refreshQRCode();
      }
    }
  }

  // 申请极验验证码
  Future<void> getCaptcha(
    String geeGt,
    String geeChallenge,
    VoidCallback onSuccess, {
    bool canRefreshRegistration = false,
  }) async {
    if (_isVerifyingCaptcha || isClosed) return;
    _isVerifyingCaptcha = true;
    var succeeded = false;
    try {
      captchaData
        ..validate = null
        ..seccode = null
        ..geetest = null;
      var session = CaptchaRegistrationData(
        gt: geeGt,
        challenge: geeChallenge,
        // 这一组的 token 来自上一步（preCapture 或登录接口给的 URL）。
        token: captchaData.token ?? '',
      );
      if (Platform.isIOS && canRefreshRegistration) {
        // 只有短信验证这条路支持用 preCapture 换一组新注册。
        // 「这组还能不能用、换不换、换到哪一组」全在 Rust
        // （`bili_api::login::resolve_captcha`），这里只拿结论。
        final resolved = await RustLogin.resolveCaptcha(
          session,
          // 原生极验 SDK 与 WebView 用的是两份配置，必须各查各的；
          // 基线这条探测固定带 iOS 原生的参数。
          nativeIos: true,
        );
        if (isClosed) return;
        if (resolved case Error(:final errMsg)) {
          SmartDialog.showToast(errMsg ?? '验证码服务暂时不可用，请稍后重试或使用扫码登录');
          return;
        }
        session = resolved.data;
      }

      final res = await GeetestWebviewDialog.geetest(
        session.gt,
        session.challenge,
      );
      if (res == null || isClosed) return;
      final validate = res['geetest_validate'];
      final seccode = res['geetest_seccode'];
      final challenge = res['geetest_challenge'];
      if (validate is! String ||
          validate.isEmpty ||
          seccode is! String ||
          seccode.isEmpty ||
          challenge is! String ||
          challenge.isEmpty) {
        SmartDialog.showToast('验证结果不完整，请重试');
        return;
      }
      captchaData
        ..token = session.token
        ..validate = validate
        ..seccode = seccode
        ..geetest = GeetestData(challenge: challenge, gt: session.gt);
      succeeded = true;
      SmartDialog.showToast('验证成功');
    } catch (_) {
      if (!isClosed) {
        SmartDialog.showToast('加载验证码失败，请稍后重试或使用扫码登录');
      }
    } finally {
      _isVerifyingCaptcha = false;
    }
    if (succeeded && !isClosed) onSuccess();
  }

  static String validateCookie(String cookie) {
    return cookie
        .split(';')
        .where((e) {
          try {
            Cookie.fromSetCookieValue(e.trim());
          } catch (_) {
            return false;
          }
          return true;
        })
        .join(';');
  }

  // cookie登录
  Future<void> loginByCookie() async {
    if (cookieTextController.text.isEmpty) {
      SmartDialog.showToast('cookie不能为空');
      return;
    }
    try {
      final result = await Request().get(
        "/x/member/web/account",
        options: Options(
          headers: {
            "cookie": validateCookie(cookieTextController.text),
          },
          extra: {'account': AnonymousAccount()},
        ),
      );
      if (result.data['code'] == 0) {
        try {
          await LoginAccount(
            BiliCookieJar.fromJson(
              Map.fromEntries(
                cookieTextController.text.split(';').map((item) {
                  final list = item.split('=');
                  return MapEntry(list.first, list.skip(1).join());
                }),
              ),
            ),
            null,
            null,
          ).onChange();
          if (!Accounts.main.isLogin) await switchAccountDialog(Get.context!);
          SmartDialog.showToast('登录成功');
          Get.back();
        } catch (e) {
          SmartDialog.showToast("登录失败: $e");
        }
      } else {
        SmartDialog.showToast("哔哩哔哩登录已失效，请重新登录");
      }
    } catch (e) {
      SmartDialog.showToast("获取哔哩哔哩用户信息失败，可前往账号管理重试");
    }
  }

  // app端密码登录
  //
  // 「登录成没成、要不要验证码、要不要先验手机号」由 Rust 判定
  // （`bili_api::login`），这里只按结论走界面（ADR-013 决策八）。
  Future<void> loginByPassword() async {
    final username = usernameTextController.text;
    final password = passwordTextController.text;
    if (username.isEmpty || password.isEmpty) {
      SmartDialog.showToast('用户名或密码不能为空');
      return;
    }

    // 公钥与盐不再经手 Dart：Rust 在同一次调用里取公钥、加密、发请求，
    // 明文密码不落任何结构体（见 `RustLogin` 的"凭据边界"）。
    final res = await RustLogin.loginByPassword(
      username: username,
      password: password,
      captcha: _captchaSolution(),
    );
    if (res case Error(:final errMsg)) {
      SmartDialog.showToast(errMsg ?? '登录失败');
      return;
    }
    final state = res.data;
    switch (state.status) {
      case PasswordLoginStatus.authenticated:
        final tokens = state.tokens;
        if (tokens == null) {
          SmartDialog.showToast('登录异常，接口未返回身份信息');
          return;
        }
        SmartDialog.showToast('正在保存身份信息');
        TextInput.finishAutofillContext();
        await setAccount(tokens);
        Get.back();
      case PasswordLoginStatus.captchaRequired:
        final registration = state.captcha;
        if (registration == null) return;
        captchaData.token = registration.token;
        // 过了人机验证再用同样的用户名密码重来一次。
        getCaptcha(registration.gt, registration.challenge, loginByPassword);
      case PasswordLoginStatus.riskControl:
        final risk = state.risk;
        if (risk == null) return;
        await _verifyTelBeforeLogin(risk);
    }
  }

  /// 风控：本次登录要求先验证手机号。
  ///
  /// 从基线 `loginByPassword` 里原样搬出来的一段弹窗流程——它本来嵌在
  /// 登录函数中间，和三条接口的解析混在一起。参数 [risk] 里的 tmp_code /
  /// request_id / source 由 Rust 从服务端给的 URL 解析好，不再在界面层
  /// 用 `!` 从查询串里现扒。
  Future<void> _verifyTelBeforeLogin(RiskVerificationData risk) async {
    SmartDialog.showToast(risk.message);

    final infoRes = await RustLogin.safeCenterInfo(risk.tmpCode);
    if (infoRes case Error(:final errMsg, :final code)) {
      SmartDialog.showToast("获取安全验证信息失败，请尝试其它登录方式\n($code) $errMsg");
      return;
    }
    final info = infoRes.data;
    if (!info.telVerify) {
      SmartDialog.showToast("当前账号未支持手机号验证，请尝试其它登录方式");
      return;
    }
    if (isClosed || Get.context == null) return;

    final textFieldController = TextEditingController();
    // 这一轮验证的短信凭证，发送成功后回填，提交时用。
    String smsCaptchaKey = '';
    await showDialog(
      context: Get.context!,
      builder: (context) => AlertDialog(
        titlePadding: const .only(left: 16, top: 18, right: 16, bottom: 12),
        contentPadding: const .symmetric(horizontal: 16),
        actionsPadding: const .symmetric(horizontal: 16, vertical: 12),
        title: const Text("本次登录需要验证您的手机号", textAlign: .center),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              info.hideTel ?? '未能获取手机号',
              style: const TextStyle(fontSize: 18),
            ),
            // 带有清空按钮的输入框
            TextField(
              style: const TextStyle(fontSize: 15),
              controller: textFieldController,
              textAlign: TextAlign.center,
              decoration: InputDecoration(
                hintText: "请输入短信验证码",
                hintStyle: const TextStyle(fontSize: 15),
                suffixIcon: iconButton(
                  icon: const Icon(Icons.clear),
                  size: 32,
                  onPressed: textFieldController.clear,
                ),
                suffixIconConstraints: const BoxConstraints(
                  maxHeight: 32,
                  maxWidth: 32,
                ),
              ),
            ),
          ],
        ),
        actions: <Widget>[
          TextButton(
            child: const Text("发送验证码"),
            onPressed: () async {
              final preCaptureRes = await RustLogin.preCapture();
              if (preCaptureRes case Error(:final errMsg, :final code)) {
                SmartDialog.showToast("获取验证码失败，请尝试其它登录方式\n($code) $errMsg");
                return;
              }
              final registration = preCaptureRes.data;
              captchaData.token = registration.token;
              getCaptcha(
                registration.gt,
                registration.challenge,
                () async {
                  final smsRes = await RustLogin.safeCenterSms(
                    verification: risk,
                    captcha: _captchaSolution(),
                  );
                  if (smsRes case Error(:final errMsg, :final code)) {
                    SmartDialog.showToast(
                      "发送短信验证码失败，请尝试其它登录方式\n($code) $errMsg",
                    );
                    return;
                  }
                  SmartDialog.showToast("短信验证码已发送，请查收");
                  smsCaptchaKey = smsRes.data;
                },
                canRefreshRegistration: true,
              );
            },
          ),
          TextButton(
            onPressed: Get.back,
            child: Text(
              "取消",
              style: TextStyle(color: ThemeUtils.theme.colorScheme.outline),
            ),
          ),
          TextButton(
            onPressed: () async {
              final code = textFieldController.text;
              if (code.isEmpty) {
                SmartDialog.showToast("请输入短信验证码");
                return;
              }
              final verifyRes = await RustLogin.safeCenterVerify(
                verification: risk,
                code: code,
                captchaKey: smsCaptchaKey,
              );
              if (verifyRes case Error(:final errMsg, :final code)) {
                SmartDialog.showToast("验证短信验证码失败，请尝试其它登录方式\n($code) $errMsg");
                return;
              }
              SmartDialog.showToast("验证成功，正在登录");
              final tokenRes = await RustLogin.exchangeOauthCode(
                verifyRes.data,
              );
              if (tokenRes case Error(:final errMsg, :final code)) {
                SmartDialog.showToast("登录失败，请尝试其它登录方式\n($code) $errMsg");
                return;
              }
              Get.back();
              SmartDialog.showToast('正在保存身份信息');
              TextInput.finishAutofillContext();
              await setAccount(tokenRes.data);
              Get.back();
            },
            child: const Text("确认"),
          ),
        ],
      ),
    ).whenComplete(textFieldController.dispose);
  }

  // 短信验证码登录
  Future<void> loginBySmsCode() async {
    if (telTextController.text.isEmpty) {
      SmartDialog.showToast('手机号不能为空');
      return;
    }
    if (captchaKey.isEmpty) {
      SmartDialog.showToast('请先点击获取验证码');
      return;
    }
    if (smsCodeTextController.text.isEmpty) {
      SmartDialog.showToast('验证码不能为空');
      return;
    }
    if (DateTime.now().millisecondsSinceEpoch - smsSendTimestamp >
        1000 * 60 * 5) {
      SmartDialog.showToast('验证码已过期，请重新获取');
      return;
    }
    final res = await RustLogin.loginBySms(
      cid: selectedCountryCodeId.countryId.toString(),
      tel: telTextController.text,
      code: smsCodeTextController.text,
      captchaKey: captchaKey,
    );
    if (res case Error(:final errMsg)) {
      SmartDialog.showToast(errMsg ?? '登录失败');
      return;
    }
    SmartDialog.showToast('登录成功');
    await setAccount(res.data);
    Get.back();
  }

  // app端验证码
  //
  // 「发出去了」与「要先过人机验证」是服务端的两种回应，由 Rust 判定后
  // 合成 [SmsSendStatus]：基线要在成功响应的 `recaptcha_url` 非空、失败码
  // -105、以及参数不全时另调 preCapture 三处各判一次。
  Future<void> sendSmsCode() async {
    if (telTextController.text.isEmpty) {
      SmartDialog.showToast('手机号不能为空');
      return;
    }
    final res = await RustLogin.sendSmsCode(
      cid: selectedCountryCodeId.countryId.toString(),
      tel: telTextController.text,
      captcha: _captchaSolution(),
    );
    if (res case Error(:final errMsg)) {
      SmartDialog.showToast(errMsg ?? '发送失败');
      return;
    }
    final state = res.data;
    switch (state.status) {
      case SmsSendStatus.sent:
        SmartDialog.showToast('发送成功');
        smsSendTimestamp = DateTime.now().millisecondsSinceEpoch;
        smsSendCooldown.value = 60;
        captchaKey = state.captchaKey ?? '';
        smsSendCooldownTimer = Timer.periodic(const Duration(seconds: 1), (
          timer,
        ) {
          smsSendCooldown.value = 60 - timer.tick;
          if (smsSendCooldown <= 0) {
            smsSendCooldownTimer?.cancel();
            smsSendCooldown.value = 0;
          }
        });
      case SmsSendStatus.captchaRequired:
        final registration = state.captcha;
        if (registration == null) return;
        captchaData.token = registration.token;
        getCaptcha(
          registration.gt,
          registration.challenge,
          sendSmsCode,
          canRefreshRegistration: true,
        );
    }
  }

  /// 把界面手里的人机验证结果摊成跨 FFI 的形状。
  CaptchaSolutionData _captchaSolution() => CaptchaSolutionData(
    challenge: captchaData.geetest?.challenge,
    seccode: captchaData.seccode,
    validate: captchaData.validate,
    token: captchaData.token,
  );

  /// 把 Rust 交回来的身份材料存进账号体系。
  ///
  /// 存储是宿主的事（RFC-0001 §2.3）：Rust 不保存任何凭据，它只是把登录
  /// 接口的响应解析好交回来，真相源仍然只有 `lib/utils/accounts/` 一处。
  Future<void> setAccount(LoginTokensData tokens) async {
    final account = LoginAccount(
      BiliCookieJar.fromList([
        for (final cookie in tokens.cookies)
          {'name': cookie.name, 'value': cookie.value},
      ]),
      tokens.accessToken,
      tokens.refreshToken,
    );
    await Future.wait([?account.onChange(), AnonymousAccount().delete()]);
    for (int i = 0; i < AccountType.values.length; i++) {
      if (Accounts.accountMode[i].mid == account.mid) {
        Accounts.accountMode[i] = account;
      }
    }
    if (Accounts.main.isLogin) {
      SmartDialog.showToast('登录成功');
    } else {
      SmartDialog.showToast('登录成功, 请先设置账号模式');
      await switchAccountDialog(Get.context!);
    }
  }

  static Future<void>? switchAccountDialog(BuildContext parentContext) {
    // if (Accounts.account.isEmpty) {
    //   SmartDialog.showToast('请先登录');
    //   return Get.toNamed('/loginPage');
    // }
    final selectAccount = List.of(Accounts.accountMode);
    final options = {
      AnonymousAccount(): '0',
      ...Accounts.account.toMap().map(
        (k, v) => MapEntry(v, k as String),
      ),
    };
    bool quickSelect = selectAccount.every((e) => e == selectAccount.first);
    return showDialog(
      context: parentContext,
      builder: (context) {
        final colorScheme = ColorScheme.of(context);
        return AlertDialog(
          title: Row(
            crossAxisAlignment: .start,
            children: [
              Text.rich(
                style: const TextStyle(height: 1.5),
                TextSpan(
                  children: [
                    const TextSpan(text: '账号切换'),
                    TextSpan(
                      text: '\nmid为0时使用匿名',
                      style: TextStyle(
                        fontSize: 14,
                        color: colorScheme.outline,
                      ),
                    ),
                  ],
                ),
              ),
              const Spacer(),
              TextButton(
                style: TextButton.styleFrom(
                  visualDensity: .compact,
                  tapTargetSize: .shrinkWrap,
                ),
                onPressed: () {
                  quickSelect = !quickSelect;
                  (context as Element).markNeedsBuild();
                },
                child: Text(quickSelect ? '详细' : '快速'),
              ),
              IconButton(
                visualDensity: .compact,
                tooltip: '登录',
                onPressed: () async {
                  Get.back();
                  await Get.toNamed('/loginPage');
                  if (parentContext.mounted) {
                    switchAccountDialog(parentContext);
                  }
                },
                iconSize: 18,
                icon: Icon(Icons.login_outlined, color: colorScheme.secondary),
              ),
            ],
          ),
          titlePadding: const .only(left: 22, top: 16, right: 22, bottom: 3),
          contentPadding: const .symmetric(vertical: 5),
          actionsPadding: const .only(left: 16, right: 16, bottom: 10),
          content: SingleChildScrollView(
            child: AnimatedSize(
              curve: Curves.easeIn,
              alignment: .topCenter,
              duration: const Duration(milliseconds: 200),
              child: quickSelect
                  ? Builder(
                      builder: (context) => RadioGroup<Account>(
                        groupValue: selectAccount[0],
                        onChanged: (v) {
                          selectAccount.fillRange(0, selectAccount.length, v);
                          (context as Element).markNeedsBuild();
                        },
                        child: Column(
                          crossAxisAlignment: .start,
                          children: options.entries
                              .map(
                                (entry) => RadioWidget<Account>(
                                  value: entry.key,
                                  title: entry.value,
                                  mainAxisSize: .max,
                                  padding: PlatformUtils.isDesktop
                                      ? const .only(left: 12)
                                      : const .only(
                                          left: 12,
                                          top: 2,
                                          bottom: 2,
                                        ),
                                ),
                              )
                              .toList(),
                        ),
                      ),
                    )
                  : Column(
                      crossAxisAlignment: .start,
                      children: AccountType.values
                          .map(
                            (e) => Builder(
                              builder: (context) => RadioGroup<Account>(
                                groupValue: selectAccount[e.index],
                                onChanged: (v) {
                                  selectAccount[e.index] = v!;
                                  (context as Element).markNeedsBuild();
                                },
                                child: WrapRadioOptionsGroup<Account>(
                                  groupTitle: e.title,
                                  options: options,
                                ),
                              ),
                            ),
                          )
                          .toList(),
                    ),
            ),
          ),
          actions: [
            TextButton(
              onPressed: Get.back,
              child: Text('取消', style: TextStyle(color: colorScheme.outline)),
            ),
            TextButton(
              onPressed: () {
                Get.back();
                for (final type in AccountType.values) {
                  final index = type.index;
                  final account = quickSelect
                      ? selectAccount.first
                      : selectAccount[index];
                  if (account != Accounts.accountMode[index]) {
                    Accounts.set(type, account);
                  }
                }
              },
              child: const Text('确定'),
            ),
          ],
        );
      },
    );
  }
}
