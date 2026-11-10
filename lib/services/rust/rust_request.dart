import 'package:kilikili/http/constants.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/accounts/account_manager/account_mgr.dart';

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

  /// 取某条接口路径对应的 Cookie 头。未登录时为 null。
  static Future<String?> cookieFor(Account account, String path) async {
    if (account is NoAccount) return null;
    final uri = Uri.parse('${HttpString.apiBaseUrl}$path');
    final cookies = await account.cookieJar.loadForRequest(uri);
    final value = AccountManager.getCookies(cookies);
    return value.isEmpty ? null : value;
  }
}
