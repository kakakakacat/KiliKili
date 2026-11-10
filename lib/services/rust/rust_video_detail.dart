import 'dart:convert';

import 'package:PiliPlus/http/api.dart';
import 'package:PiliPlus/http/constants.dart';
import 'package:PiliPlus/http/loading_state.dart';
import 'package:PiliPlus/models_new/video/video_detail/data.dart';
import 'package:PiliPlus/rust/api/video_detail_fetch.dart' as rust_api;
import 'package:PiliPlus/services/rust/rust_flags.dart';
import 'package:PiliPlus/services/rust/video_detail_denormalize.dart';
import 'package:PiliPlus/utils/accounts.dart';
import 'package:PiliPlus/utils/accounts/account.dart';
import 'package:PiliPlus/utils/accounts/account_manager/account_mgr.dart';
import 'package:PiliPlus/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

/// 视频详情的 Rust 取数路径：请求由 Rust 发出、Rust 解析。
///
/// 开关 `rust.fetch.videoDetail` 默认关闭。任何一步不成立都返回 `null`，
/// 由调用方走原来的 Dart 路径——**降级是默认行为**，不是异常分支
/// （RFC-0001 §5.3）。
///
/// ## 凭据
///
/// Cookie 与账号头按请求从 Dart 的账号体系取出传给 Rust，Rust 不保存。
/// 账号切换、Cookie 刷新、登出仍然只发生在 Dart 一侧，不存在两份账号状态。
///
/// ## 与基线 Dio 的差距
///
/// Rust 侧没有基线的重试拦截器、系统代理、证书豁免与 HTTP/1.1 回退。
/// 依赖这些设置的环境下本路径会失败并降级，不会把请求发错地方。
abstract final class RustVideoDetail {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  /// 取一条视频详情。返回 `null` 表示本次不走 Rust，调用方应走 Dart 路径。
  static Future<LoadingState<VideoDetailData>?> fetch(String bvid) async {
    if (!RustFlags.rustVideoDetail) return null;

    try {
      final mixinKey = await WbiSign.getWbiKeys();
      if (mixinKey.isEmpty) {
        // 取 key 失败时基线会用空串继续签名并得到业务错误码。那是基线的
        // 既有行为，不是我们要在这里复制的行为——没有 key 就降级。
        return null;
      }

      // videoIntro 绑定 heartbeat 账号（lib/utils/accounts/api_type.dart）。
      // 这里必须与拦截器的选择一致，否则两条路径用的是不同账号。
      final account = Accounts.heartbeat;
      final uri = Uri.parse('${HttpString.apiBaseUrl}${Api.videoIntro}');

      final result = await rust_api.fetchVideoDetail(
        baseUrl: HttpString.apiBaseUrl,
        bvid: bvid,
        mixinKey: mixinKey,
        wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
        cookie: await _cookieFor(account, uri),
        headers: _headersFor(account),
        timeoutMs: BigInt.from(_timeoutMs),
      );

      final json = result.json;
      if (json == null) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 视频详情取数失败，降级回 Dart: '
            '${result.errorKind} / ${result.errorDetail}',
          );
        }
        return null;
      }

      final normalized = jsonDecode(json) as Map<String, dynamic>;
      return Success(
        VideoDetailData.fromJson(denormalizeVideoDetail(normalized)),
      );
    } catch (e, st) {
      // 包括桥接未就绪、账号状态异常在内的一切意外：降级，不抛。
      if (kDebugMode) debugPrint('[rust] 取数路径自身出错，降级: $e\n$st');
      return null;
    }
  }

  static List<(String, String)> _headersFor(Account account) => [
    ...account.headers.entries.map((e) => (e.key, e.value)),
    // 拦截器对未显式设置 referer 的请求补这一条
    // （lib/utils/accounts/account_manager/account_mgr.dart）。
    ('referer', HttpString.baseUrl),
    // 基线的 BaseOptions 里带的两条。Http2Adapter 不会自动补 UA。
    ('user-agent', 'Dart/3.6 (dart:io)'),
    ('accept-encoding', 'br,gzip'),
  ];

  static Future<String?> _cookieFor(Account account, Uri uri) async {
    if (account is NoAccount) return null;
    final cookies = await account.cookieJar.loadForRequest(uri);
    final value = AccountManager.getCookies(cookies);
    return value.isEmpty ? null : value;
  }
}
