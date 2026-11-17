import 'dart:io';

import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/services/account_service.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hive_ce/hive.dart';

/// issue #161 的回归：全局登录态不能挂在资料缓存上。
///
/// S7 #57 把用户资料缓存换了盒名（`userInfo` → `userInfoJson`）并删掉老盒，
/// 老用户升级上来缓存为空。当时 `AccountService` 判的是「有没有缓存」，
/// 于是 cookie 还在、动态流照常是登录态的内容，app 却全程当成未登录。
///
/// 这里只用「缓存为空」这一种形态：有缓存的那条分支要把 JSON 交给 Rust
/// 解析（`RustUser.cachedUserInfo`），属于 `test/bridge/` 的范围，而本用例
/// 要验的恰恰是缓存**没有**的时候。
LoginAccount _account() => LoginAccount(
  BiliCookieJar.fromJson({
    'DedeUserID': '123',
    'bili_jct': 'csrf',
  }),
  'access-key',
  'refresh-token',
);

void main() {
  late Directory tempDir;

  setUpAll(() async {
    tempDir = await Directory.systemTemp.createTemp('kilikili-account-service-');
    Hive.init(tempDir.path);
    GStorage.regAdapter();
    GStorage.setting = await Hive.openBox('setting');
    GStorage.localCache = await Hive.openBox('localCache');
    GStorage.video = await Hive.openBox('video');
    GStorage.userInfo = await Hive.openBox<String>('userInfoJson');
    Accounts.account = await Hive.openBox<LoginAccount>('account');
  });

  setUp(() {
    // 直接摆 `accountMode`，不走 `Accounts.refresh()`：那条会给未激活的
    // 账号发 buvid 激活请求，测试里不该有网络。
    Accounts.accountMode[AccountType.main.index] = AnonymousAccount();
    GStorage.userInfo.delete('userInfoCache');
  });

  tearDownAll(() async {
    await Hive.close();
    await tempDir.delete(recursive: true);
  });

  test('资料缓存为空但账号还在时，仍然是登录态', () {
    Accounts.accountMode[AccountType.main.index] = _account();
    expect(Accounts.main.isLogin, isTrue, reason: '前提：账号里有 cookie');

    final service = AccountService()..onInit();

    expect(
      service.isLogin.value,
      isTrue,
      reason: '登录态取自账号，不取自缓存（#161）',
    );
    expect(service.face.value, '', reason: '没有缓存就先空着，等 nav 回来');
  });

  test('没有账号时是未登录', () {
    final service = AccountService()..onInit();

    expect(service.isLogin.value, isFalse);
    expect(service.face.value, '');
  });
}
