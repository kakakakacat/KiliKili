import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/login.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/login.dart'
    show
        CaptchaRegistrationData,
        CaptchaSolutionData,
        LoginCookieData,
        LoginDeviceData,
        LoginTokensData,
        PasswordLoginState,
        PasswordLoginStatus,
        QrCodeData,
        QrPollState,
        QrPollStatus,
        RiskVerificationData,
        SafeCenterInfoData,
        SmsSendState,
        SmsSendStatus;

/// 登录流程的取数（S14 / issue #65）。
///
/// 基线 `LoginHttp` 已随本次迁移删除，按 ADR-011 决策四**不留降级开关**。
/// 登录尤其不能留两套实现：两套各自生成 `device_id`、各自签名，出问题时
/// 无从判断用户是从哪条路登进来的。
///
/// ## 这一层剩下什么
///
/// 只剩三件事，其余都在 `rust/crates/bili-api/src/login.rs`：
///
/// 1. 组装上下文（主机、匿名账号的头、buvid、传输设置、本机时区）；
/// 2. 把 Rust 的结果翻成 [LoadingState]；
/// 3. 登录成功后把身份材料交给账号体系——**存储是宿主的事**
///    （RFC-0001 §2.3）。
///
/// "登录成没成、要不要验证码、二维码过没过期"一概不在这里重新判断
/// （ADR-013 决策八）。
///
/// ## 凭据边界
///
/// Rust 不保存任何凭据：它把登录接口返回的 token 与 cookie 沿返回值交回
/// 来，由 `lib/utils/accounts/` 存，真相源仍然只有一处。明文密码只作为
/// [loginByPassword] 的参数穿过 FFI，Rust 侧立刻加密，不进任何结构体。
/// 详见 `bili_api::login` 的模块文档。
abstract final class RustLogin {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，登录暂不可用';

  /// 登录接口一律用匿名账号：基线的拦截器对 `passport.bilibili.com`
  /// 不挂任何已登录账号的 Cookie。
  static Account get _anonymous => AnonymousAccount();

  /// 未登录时用的上下文。
  static rust_api.LoginContext _context() => _build(_anonymous);

  /// 带当前主账号 Cookie 与 csrf 的上下文（登出、已登录设备要用）。
  static Future<rust_api.LoginContext> _accountContext(Account account) async =>
      _build(
        account,
        cookie: await RustRequest.cookieFor(account, HttpString.passBaseUrl),
        csrf: RustRequest.csrfFor(account),
      );

  static rust_api.LoginContext _build(
    Account account, {
    String? cookie,
    String? csrf,
  }) => rust_api.LoginContext(
    baseUrl: HttpString.passBaseUrl,
    cookie: cookie,
    csrf: csrf,
    headers: RustRequest.headersFor(account),
    timeoutMs: BigInt.from(_timeoutMs),
    transport: RustTransport.current(),
    appkey: Constants.appKey,
    appsec: Constants.appSec,
    // 匿名账号的 buvid 同时充当 local_id，与基线 `LoginHttp._buvid` 一致。
    buvid: _anonymous.buvid,
    userAgent: Constants.userAgent,
    statistics: Constants.statistics,
    traceId: Constants.traceId,
    // 设备号里嵌的是本地时间，时区只有宿主知道。
    utcOffsetSecs: DateTime.now().timeZoneOffset.inSeconds,
  );

  /// 申请扫码登录的二维码。
  static Future<LoadingState<rust_api.QrCodeData>> qrCode() =>
      _call('申请二维码', () async {
        final res = await rust_api.loginQrcode(context: _context());
        return (res.data, res.errorKind, res.errorDetail, res.errorCode);
      });

  /// 轮询扫码状态。
  ///
  /// 返回 [Error] 的只有传输与协议问题：未扫、未确认、已失效都是正常
  /// 状态，看 [rust_api.QrPollState.status]。
  static Future<LoadingState<rust_api.QrPollState>> qrCodePoll(
    String authCode,
  ) => _call('扫码轮询', () async {
    final res = await rust_api.loginQrcodePoll(
      context: _context(),
      authCode: authCode,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 密码登录。
  static Future<LoadingState<rust_api.PasswordLoginState>> loginByPassword({
    required String username,
    required String password,
    required rust_api.CaptchaSolutionData captcha,
  }) => _call('密码登录', () async {
    final res = await rust_api.loginByPassword(
      context: _context(),
      username: username,
      password: password,
      captcha: captcha,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 发送登录短信验证码。
  static Future<LoadingState<rust_api.SmsSendState>> sendSmsCode({
    required String cid,
    required String tel,
    required rust_api.CaptchaSolutionData captcha,
  }) => _call('发送验证码', () async {
    final res = await rust_api.loginSendSmsCode(
      context: _context(),
      cid: cid,
      tel: tel,
      captcha: captcha,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 短信验证码登录。
  static Future<LoadingState<rust_api.LoginTokensData>> loginBySms({
    required String cid,
    required String tel,
    required String code,
    required String captchaKey,
  }) => _call('短信登录', () async {
    final res = await rust_api.loginBySms(
      context: _context(),
      cid: cid,
      tel: tel,
      code: code,
      captchaKey: captchaKey,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 风控流程：取打码后的手机号/邮箱。
  static Future<LoadingState<rust_api.SafeCenterInfoData>> safeCenterInfo(
    String tmpCode,
  ) => _call('获取安全验证信息', () async {
    final res = await rust_api.loginSafeCenterInfo(
      context: _context(),
      tmpCode: tmpCode,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 申请一组人机验证注册。
  static Future<LoadingState<rust_api.CaptchaRegistrationData>> preCapture() =>
      _call('获取验证码', () async {
        final res = await rust_api.loginPreCapture(context: _context());
        return (res.data, res.errorKind, res.errorDetail, res.errorCode);
      });

  /// 确认一组注册当前可用，不可用就**整组**换一次（#140）。
  ///
  /// [nativeIos] 对应原生极验 SDK 那条分支：WebView 与原生 SDK 用的是
  /// 不同配置，必须各查各的。
  static Future<LoadingState<rust_api.CaptchaRegistrationData>> resolveCaptcha(
    rust_api.CaptchaRegistrationData registration, {
    bool nativeIos = false,
  }) => _call('校验验证码配置', () async {
    final res = await rust_api.loginResolveCaptcha(
      context: _context(),
      registration: registration,
      nativeIos: nativeIos,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 风控流程：发短信，返回登录要回填的 `captcha_key`。
  static Future<LoadingState<String>> safeCenterSms({
    required rust_api.RiskVerificationData verification,
    required rust_api.CaptchaSolutionData captcha,
    String? smsType,
  }) => _call('发送短信验证码', () async {
    final res = await rust_api.loginSafeCenterSms(
      context: _context(),
      verification: verification,
      smsType: smsType,
      captcha: captcha,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 风控流程：提交验证码，换一次性 code。
  static Future<LoadingState<String>> safeCenterVerify({
    required rust_api.RiskVerificationData verification,
    required String code,
    required String captchaKey,
    String? smsType,
  }) => _call('验证短信验证码', () async {
    final res = await rust_api.loginSafeCenterVerify(
      context: _context(),
      verification: verification,
      code: code,
      captchaKey: captchaKey,
      smsType: smsType,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 用一次性 code 换身份材料。
  static Future<LoadingState<rust_api.LoginTokensData>> exchangeOauthCode(
    String code,
  ) => _call('换取登录凭证', () async {
    final res = await rust_api.loginExchangeOauthCode(
      context: _context(),
      code: code,
    );
    return (res.data, res.errorKind, res.errorDetail, res.errorCode);
  });

  /// 登出指定账号。
  static Future<LoadingState<void>> logout(Account account) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.loginLogout(
        context: await _accountContext(account),
      );
      if (res.errorKind != null) {
        return Error(res.errorDetail ?? '退出登录失败', code: res.errorCode);
      }
      return const Success(null);
    } catch (e, st) {
      return _crashed('退出登录', e, st);
    }
  }

  /// 已登录设备列表。
  static Future<LoadingState<List<rust_api.LoginDeviceData>>>
  loginDevices() async {
    if (!RustCore.isReady) return const Error(_notReady);
    final account = Accounts.main;
    try {
      final res = await rust_api.loginDevices(
        context: await _accountContext(account),
        accessKey: account.accessKey,
      );
      if (res.errorKind != null) {
        return Error(res.errorDetail ?? '获取登录设备失败', code: res.errorCode);
      }
      return Success(res.data ?? const []);
    } catch (e, st) {
      return _crashed('获取登录设备', e, st);
    }
  }

  /// 统一的调用外壳：就绪判定、结果翻译、异常兜底各写一次。
  ///
  /// 返回的四元组就是每个结果类型的四个字段；`data` 为空且 `errorKind`
  /// 为空不会同时出现（ADR-011），真出现了按"未返回数据"处理，而不是
  /// 把 null 当成功传下去。
  static Future<LoadingState<T>> _call<T>(
    String what,
    Future<(T?, String?, String?, int?)> Function() run,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final (data, errorKind, errorDetail, errorCode) = await run();
      if (errorKind != null) {
        return Error(errorDetail ?? '$what失败', code: errorCode);
      }
      if (data == null) return Error('$what失败：接口未返回数据');
      return Success(data);
    } catch (e, st) {
      return _crashed(what, e, st);
    }
  }

  /// 跨 FFI 的异常（原生库缺失、panic）不该让登录页整个崩掉。
  static Error _crashed(String what, Object e, StackTrace st) {
    if (kDebugMode) {
      debugPrint('Rust $what 调用异常: $e\n$st');
    }
    return Error('$what失败: $e');
  }
}
