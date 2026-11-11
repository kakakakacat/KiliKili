import 'package:kilikili/build_config.dart';
import 'package:kilikili/common/constants.dart';
import 'package:kilikili/rust/api/sponsor_block.dart' as rust_api;
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/sponsor_block.dart'
    show
        SkipSegment,
        SkipSegmentsResult,
        SponsorStatusResult,
        SponsorTextResult,
        SponsorUserInfo,
        SponsorUserInfoResult;

/// SponsorBlock 的取数路径。**typed 直连的样板**（issue #64 / EPIC #67）。
///
/// 与此前七条路径的区别：这里不返回 JSON 字符串、Dart 侧也不再有一份
/// 规范化器与还原器——Rust 直接给出 typed 结构，调用点消费它。
///
/// ## 没有降级开关，这是有意的
///
/// 往返模式下每条路径都配一个 `rust.*` 开关，失败就走 Dart 原实现。
/// typed 直连**删掉了 Dart 原实现**，降级无处可去：`lib/http/sponsor_block.dart`
/// 里对应的四个只读函数已随本次迁移删除。
///
/// 代价写在明处：`RustCore` 未就绪时（原生库没打进产物）本功能整体不可用，
/// 表现为"查不到片段"，而不是崩溃。这是 typed 直连的固有取舍，已作为
/// ADR-011（#48）的待决项提出。
///
/// ## 设置按请求传入
///
/// 服务器地址来自用户设置（`Pref.blockServer`），与 Cookie 同理：真相源
/// 留在 Dart，Rust 不存。
abstract final class RustSponsorBlock {
  /// 与基线一致的超时。
  static const int _timeoutMs = 10000;

  static rust_api.SponsorBlockContext _context() => rust_api.SponsorBlockContext(
    baseUrl: Pref.blockServer,
    // 上游 wiki 要求的公用参数；基线在 debug 构建下不带，保持一致。
    headers: kDebugMode
        ? const []
        : [
            ('origin', Constants.appName),
            ('x-ext-version', BuildConfig.versionName),
          ],
    timeoutMs: BigInt.from(_timeoutMs),
    transport: RustTransport.current(),
  );

  /// Rust 未就绪时的统一结果：不抛异常，调用点照常走失败分支。
  static const String _notReady = 'Rust Core 未就绪，跳过片段功能不可用';

  /// 取一个视频的可跳过片段。
  ///
  /// `errorCode == 404` 表示这个视频没有片段，调用点应当静默处理。
  static Future<rust_api.SkipSegmentsResult> getSkipSegments({
    required String bvid,
    required int cid,
  }) async {
    if (!RustCore.isReady) {
      return const rust_api.SkipSegmentsResult(
        errorKind: 'storage',
        errorDetail: _notReady,
      );
    }
    try {
      return await rust_api.fetchSkipSegments(
        context: _context(),
        bvid: bvid,
        cid: cid,
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取跳过片段出错: $e\n$st');
      return rust_api.SkipSegmentsResult(
        errorKind: 'protocol',
        errorDetail: '$e',
      );
    }
  }

  /// 取提交者统计。
  static Future<rust_api.SponsorUserInfoResult> userInfo(
    List<String> values, {
    String? userId,
  }) async {
    if (!RustCore.isReady) {
      return const rust_api.SponsorUserInfoResult(
        errorKind: 'storage',
        errorDetail: _notReady,
      );
    }
    try {
      return await rust_api.fetchSponsorUserInfo(
        context: _context(),
        userId: userId ?? Pref.blockUserID,
        values: values,
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 取提交者统计出错: $e\n$st');
      return rust_api.SponsorUserInfoResult(
        errorKind: 'protocol',
        errorDetail: '$e',
      );
    }
  }

  /// 服务器是否在线。
  static Future<bool> uptimeStatus() async {
    if (!RustCore.isReady) return false;
    try {
      return (await rust_api.fetchSponsorUptime(context: _context())).ok;
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 探测服务器状态出错: $e');
      return false;
    }
  }

  /// 查稿件绑定的 YouTube 视频 id。
  static Future<rust_api.SponsorTextResult> getPortVideo({
    required String bvid,
    required int cid,
  }) async {
    if (!RustCore.isReady) {
      return const rust_api.SponsorTextResult(
        errorKind: 'storage',
        errorDetail: _notReady,
      );
    }
    try {
      return await rust_api.fetchPortVideo(
        context: _context(),
        bvid: bvid,
        cid: cid,
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 查绑定视频出错: $e\n$st');
      return rust_api.SponsorTextResult(
        errorKind: 'protocol',
        errorDetail: '$e',
      );
    }
  }
}
