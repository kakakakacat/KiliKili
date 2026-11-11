import 'package:kilikili/http/constants.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/accounts/account_manager/account_mgr.dart';
import 'package:kilikili/utils/accounts/api_type.dart';

/// Rust 取数路径共用的请求头与 Cookie 取法。
///
/// 这些值的真相源是 Dart 的账号体系；Rust 不保存任何凭据，每次请求由
/// 这里取出、用完即弃（RFC-0001 §2.2）。两条取数路径（详情、取流）各抄
/// 一遍的话，账号头一旦变化就会只改一处。
abstract final class RustRequest {
  /// 账号头 + 拦截器会补上的那几条。
  static List<(String, String)> headersFor(Account account) => [
    ...account.headers.entries.map((e) => (e.key, e.value)),
    // 拦截器对未显式设置 referer 的请求补这一条
    // （lib/utils/accounts/account_manager/account_mgr.dart）。
    ('referer', HttpString.baseUrl),
    // 基线 BaseOptions 里带的两条。Http2Adapter 不会自动补 UA。
    ('user-agent', 'Dart/3.6 (dart:io)'),
    ('accept-encoding', 'br,gzip'),
  ];

  /// 取某条接口该用哪个账号。
  ///
  /// 照搬 `AccountManager._findAccount` 的查表，不在各服务里硬编码：
  /// 一个切片横跨多个账号桶时硬编码迟早错一个，而错了不会报错，只会
  /// 悄悄换一批数据。
  static Account accountFor(String path) => Accounts.get(
    AccountType.values.firstWhere(
      (t) => ApiType.apiTypeSet[t]?.contains(path) == true,
      orElse: () => AccountType.main,
    ),
  );

  /// 取某条接口对应的 Cookie 头。未登录时为 null。
  ///
  /// [endpoint] 可以是相对路径（拼到 `api.bilibili.com`），也可以是完整
  /// URL——直播接口挂在 `api.live.bilibili.com`，`Api.live*` 常量本身
  /// 就是完整 URL。按错主机取 Cookie 不会报错，只会少带几条。
  static Future<String?> cookieFor(Account account, String endpoint) async {
    if (account is NoAccount) return null;
    final uri = Uri.parse(
      endpoint.startsWith('http')
          ? endpoint
          : '${HttpString.apiBaseUrl}$endpoint',
    );
    final cookies = await account.cookieJar.loadForRequest(uri);
    final value = AccountManager.getCookies(cookies);
    return value.isEmpty ? null : value;
  }
}
