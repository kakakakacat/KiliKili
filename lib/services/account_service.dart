import 'dart:async';

import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:get/get.dart';

class AccountService extends GetxService {
  final RxString face = ''.obs;
  final RxBool isLogin = false.obs;

  /// **登录态取自账号本身，不取自资料缓存**（issue #161）。
  ///
  /// 原来这里判的是「有没有 `userInfoCache`」。那份缓存只是头像与昵称的
  /// 本地副本，可能因为换存储格式而为空——S7 #57 把盒名从 `userInfo` 换成
  /// `userInfoJson` 并删掉老盒之后，老用户升级上来缓存就是空的，于是
  /// cookie 还在、动态流照常是登录态的内容，`isLogin` 却是 false，全 app
  /// 的登录分支（消息红点、未读动态、「我的」页）一起哑掉。
  ///
  /// 账号本身（`Accounts.main`，即有没有 cookie/token）才是这件事的唯一
  /// 真相。`Request.setCookie` 判协一直是这么做的。
  ///
  /// 时机：`Accounts.refresh()` 在 `Request()` 里、`runApp` 之前跑完，而
  /// 本服务是 `lazyPut`，第一次 `Get.find` 必然在那之后。
  @override
  void onInit() {
    super.onInit();
    isLogin.value = Accounts.main.isLogin;
    // 头像仍然来自缓存：没有就先空着，等 nav 回来再填。
    face.value = Pref.userInfoCache?.face ?? '';
  }
}

mixin AccountMixin on GetLifeCycleBase {
  StreamSubscription<bool>? _listener;

  AccountService get accountService => Get.find<AccountService>();

  void onChangeAccount(bool isLogin);

  @override
  void onInit() {
    super.onInit();
    _listener = accountService.isLogin.listen(onChangeAccount);
  }

  @override
  void onClose() {
    _listener?.cancel();
    _listener = null;
    super.onClose();
  }
}
