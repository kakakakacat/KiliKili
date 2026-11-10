import 'dart:convert';

import 'package:PiliPlus/http/constants.dart';
import 'package:PiliPlus/http/loading_state.dart';
import 'package:PiliPlus/models/common/video/video_type.dart';
import 'package:PiliPlus/models/video/play/url.dart';
import 'package:PiliPlus/rust/api/play_url_fetch.dart' as rust_api;
import 'package:PiliPlus/services/rust/play_url_denormalize.dart';
import 'package:PiliPlus/services/rust/rust_flags.dart';
import 'package:PiliPlus/services/rust/rust_request.dart';
import 'package:PiliPlus/services/rust/rust_transport.dart';
import 'package:PiliPlus/utils/accounts.dart';
import 'package:PiliPlus/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

/// 取流的 Rust 取数路径：请求由 Rust 发出、Rust 解析。
///
/// 开关 `rust.fetch.playUrl` 默认关闭。任何一步不成立都返回 `null`，
/// 由调用方走原来的 Dart 路径——**降级是默认行为**，不是异常分支
/// （RFC-0001 §5.3）。
///
/// ## 失败也降级，哪怕失败来自内容本身
///
/// 服务端明确拒绝（番剧需充电、稿件已删）时，降级回 Dart 会再失败一次，
/// 多一次请求。仍然这么做：错误文案由基线 `VideoHttp._parseVideoErr`
/// 统一产出，两条路径给出不同措辞，比多一次只读请求糟糕得多。等 Rust
/// 成为默认路径时，这里才值得改成直接报错。
///
/// ## 凭据
///
/// Cookie 与账号头按请求从 Dart 的账号体系取出传给 Rust，Rust 不保存。
abstract final class RustPlayUrl {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  /// 取一次流。返回 `null` 表示本次不走 Rust，调用方应走 Dart 路径。
  ///
  /// 参数与 `VideoHttp.videoUrl` 一一对应，但**没有** `fallback`：
  /// 降级（DRM）由 Rust 按响应自行决定，与基线的递归同样是响应驱动的。
  static Future<LoadingState<PlayUrlModel>?> fetch({
    int? avid,
    String? bvid,
    required int cid,
    required int qn,
    int? epid,
    int? seasonId,
    required bool tryLook,
    required VideoType videoType,
    String? language,
    bool voiceBalance = false,
  }) async {
    if (!RustFlags.rustPlayUrl) return null;

    try {
      final mixinKey = await WbiSign.getWbiKeys();
      // 取 key 失败时基线会用空串继续签名并换来一个业务错误码。那是基线
      // 的既有行为，不是我们要复制的行为——没有 key 就降级。
      if (mixinKey.isEmpty) return null;

      // 取流绑定 video 账号（lib/utils/accounts/api_type.dart），必须与
      // 拦截器的选择一致，否则两条路径用的是不同账号。
      final account = Accounts.video;
      final cookie = await RustRequest.cookieFor(account, videoType.api);

      final result = await rust_api.fetchPlayUrl(
        baseUrl: HttpString.apiBaseUrl,
        request: rust_api.PlayUrlRequest(
          avid: avid,
          bvid: bvid,
          epId: epid,
          seasonId: seasonId,
          cid: cid,
          qn: qn,
          videoType: videoType.name,
          tryLook: tryLook,
          voiceBalance: voiceBalance,
          language: language,
        ),
        mixinKey: mixinKey,
        wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
        cookie: cookie,
        headers: RustRequest.headersFor(account),
        timeoutMs: BigInt.from(_timeoutMs),
        transport: RustTransport.current(),
      );

      final json = result.json;
      if (json == null) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 取流失败，降级回 Dart: '
            '${result.errorKind} / ${result.errorCode} / ${result.errorDetail}',
          );
        }
        return null;
      }

      final normalized = jsonDecode(json) as Map<String, dynamic>;
      final playUrl = PlayUrlModel.fromJson(denormalizePlayUrl(normalized));
      if (kDebugMode) debugPrint('[rust] 取流成功 (cookie=${cookie != null})');
      return Success(playUrl);
    } catch (e, st) {
      // 包括桥接未就绪、账号状态异常在内的一切意外：降级，不抛。
      if (kDebugMode) debugPrint('[rust] 取流路径自身出错，降级: $e\n$st');
      return null;
    }
  }
}
