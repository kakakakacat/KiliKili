import 'package:kilikili/http/constants.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/api_type.dart';
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

  /// 按接口路径选账号。
  ///
  /// 与拦截器的选法必须一致，否则 Rust 路径与基线用的是**不同账号**
  /// ——这不会报错，只会悄悄换一批结果（`api_type.dart` 里就写着
  /// 「不同账号搜索结果可能不一样」）。
  ///
  /// 逻辑照搬 `AccountManager._findAccount`：在 `ApiType.apiTypeSet` 里
  /// 查这条 path 属于哪个 `AccountType`，查不到落到 `main`。查表而不是
  /// 在各自的服务里硬编码，是因为 S10 的八条接口横跨两个桶（索引与排行
  /// 归 recommend，时间表/评论/追番状态落 main），硬编码迟早错一个。
  ///
  /// 登录类接口不走这里——那条分支由拦截器自己处理，Rust 侧暂无写操作。
  static Account accountFor(String path) => Accounts.get(
    AccountType.values.firstWhere(
      (t) => ApiType.apiTypeSet[t]?.contains(path) == true,
      orElse: () => AccountType.main,
    ),
  );

  /// 取某条接口路径对应的 Cookie 头。未登录时为 null。
  static Future<String?> cookieFor(Account account, String path) async {
    if (account is NoAccount) return null;
    final uri = Uri.parse('${HttpString.apiBaseUrl}$path');
    final cookies = await account.cookieJar.loadForRequest(uri);
    final value = AccountManager.getCookies(cookies);
    return value.isEmpty ? null : value;
  }
}
