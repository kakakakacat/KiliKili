import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/video/video_type.dart';
import 'package:kilikili/rust/api/play_url.dart' show PlayUrl;
import 'package:kilikili/rust/api/play_url_fetch.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/play_url.dart'
    show
        ClipInfo,
        Dash,
        Durl,
        LanguageItem,
        LanguageOptions,
        MediaStream,
        PlayFormat,
        PlayUrl,
        SegmentBase,
        Volume;

/// 取流的取数路径（issue #79）。**typed 直连，没有第二条路。**
///
/// ## 这里是取流的唯一实现
///
/// 基线的 `VideoHttp.videoUrl`、`rust.fetch.playUrl` 开关、
/// `play_url_normalize/denormalize/shadow` 三组胶水与 `PlayUrlModel`
/// 整棵模型树都随本切片删除（ADR-011 决策四：迁一条路径就连旧实现一起
/// 删，不留降级）。取流失败时用户看到的是错误文案，不再是「悄悄换一条
/// 路再试一次」。
///
/// ## 重试分支在 Rust 那边
///
/// 基线有三条：PGC 内容是 DRM 时改走降级接口、UGC 带 `ep_id` 失败后改
/// 按 PGC 重取、两者不叠加。它们现在只存在于
/// `bili_api::client::fetch_play_url`，由响应驱动，不由这里指定——迁移前
/// Dart 与 Rust 各有一份，那正是「某些番剧在某条路径下播不了」的来源。
///
/// ## 凭据
///
/// Cookie 与账号头按请求从 Dart 的账号体系取出传给 Rust，Rust 不保存。
abstract final class RustPlayUrl {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，无法取流';

  /// 取一次流。
  ///
  /// 参数与基线 `VideoHttp.videoUrl` 一一对应，但**没有** `fallback`
  /// 与 `allowRust`：前者由响应决定，后者随开关一起删了。
  static Future<LoadingState<PlayUrl>> fetch({
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
    if (!RustCore.isReady) return const Error(_notReady);

    try {
      final mixinKey = await WbiSign.getWbiKeys();
      // 基线取 key 失败时会用空串继续签名、换来一个业务错误码。照搬那个
      // 行为没有意义——空签名的请求必然失败，不如直接说清楚。
      if (mixinKey.isEmpty) return const Error('WBI 签名密钥获取失败');

      // 取流绑定 video 账号（lib/utils/accounts/api_type.dart）。
      final account = Accounts.video;
      final cookie = await RustRequest.cookieFor(account, videoType.api);

      final res = await rust_api.fetchPlayUrl(
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

      final data = res.data;
      if (data == null) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 取流失败: '
            '${res.errorKind} / ${res.errorCode} / ${res.errorDetail}',
          );
        }
        return Error(
          playUrlErrorMessage(res.errorCode, res.errorDetail),
          code: res.errorCode,
        );
      }
      return Success(data);
    } catch (e, st) {
      // 桥接未就绪、账号状态异常之类的意外：转成错误值，不抛。
      if (kDebugMode) debugPrint('[rust] 取流路径自身出错: $e\n$st');
      return Error('$e');
    }
  }
}

/// 取流失败的用户文案。
///
/// 从基线 `VideoHttp._parseVideoErr` 原样搬来——那两条特判是用户能看懂
/// 这件事的全部：`-404` 与 `87008` 的服务端 message 本身都不足以说明
/// 发生了什么。码由 Rust 从接口信封原样带回（见 `PlayUrlResult`）。
String playUrlErrorMessage(int? code, String? message) => switch (code) {
  -404 => '视频不存在或已被删除',
  87008 => '当前视频可能是专属视频，可能需包月充电观看($message})',
  null => message ?? '取流失败',
  _ => '错误($code): $message',
};
