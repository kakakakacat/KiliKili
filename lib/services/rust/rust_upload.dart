import 'package:flutter/foundation.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/upload.dart' as api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';

/// 图片上传的 typed Rust 入口（ADR-015 / issue #193）。
///
/// ## 取消怎么用
///
/// [begin] 系列**同步**返回一个 id，联网在后台跑；拿着这个 id 再 `await`
/// 或 [cancel]。连接还没建立也能取消。
///
/// 两条约定，省得调用点各写一套：
///
/// - **重复取消、取消一个已经结束的、取消一个不存在的 id，都是成功**。
///   调用点常常在 `finally` 里兜底调一次，让它报错只会逼出一圈 try/catch。
/// - 被取消的上传以 `errorKind == 'cancelled'` 返回，[isCancelled] 判它。
///   宿主据此**不弹 toast**——用户自己按的取消，再提示一次是噪音。
///
/// ## 路径必须是真实文件
///
/// 不能是 Android 的 SAF uri（`content://...`）。这不是新增的约束：基线
/// `MultipartFile.fromFile(path)` 要的同样是真实路径。当前的调用点都来自
/// `image_picker` / `image_cropper`，两者会先把内容复制到应用缓存目录。
/// 将来若有调用点直接拿到 uri，由它负责先落成文件（ADR-015 决策二）。
abstract final class RustUpload {
  /// 与基线 `BaseOptions` 一致的超时。
  ///
  /// 上传比普通请求慢得多，但这里仍沿用同一个值：基线的 `Request().post`
  /// 用的就是它，改大就不再是「迁移等价」了。真机上确认不够再单独调。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，暂时无法上传';

  static Future<api.UploadContext> _context(String endpoint) async {
    final account = RustRequest.accountFor(endpoint);
    return api.UploadContext(
      baseUrl: HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(account, endpoint),
      csrf: RustRequest.csrfFor(account),
      headers: RustRequest.headersFor(account),
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
    );
  }

  /// 开始传收藏夹封面。返回 `null` 表示 Rust Core 没就绪，没有东西可取消。
  static Future<int?> beginImage({
    required String path,
    required String bucket,
    required String dir,
  }) async {
    if (!RustCore.isReady) return null;
    try {
      return api.uploadImageBegin(
        context: await _context(Api.uploadImage),
        filePath: path,
        bucket: bucket,
        dir: dir,
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 封面上传启动失败: $e\n$st');
      return null;
    }
  }

  /// 等一个封面上传的结果，拿到的是图片地址。
  static Future<LoadingState<String>> awaitImage(int? uploadId) async {
    if (uploadId == null) return const Error(_notReady);
    try {
      final result = await api.uploadImageAwait(uploadId: uploadId);
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 封面上传出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 开始传动态/私信配图。
  static Future<int?> beginBfs({
    required String path,
    String? category,
    String? biz,
  }) async {
    if (!RustCore.isReady) return null;
    try {
      return api.uploadBfsBegin(
        context: await _context(Api.uploadBfs),
        filePath: path,
        category: category,
        biz: biz,
      );
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 配图上传启动失败: $e\n$st');
      return null;
    }
  }

  /// 等一个配图上传的结果。
  static Future<LoadingState<api.UploadedImageData>> awaitBfs(
    int? uploadId,
  ) async {
    if (uploadId == null) return const Error(_notReady);
    try {
      final result = await api.uploadBfsAwait(uploadId: uploadId);
      return _unwrap(result.data, result.errorDetail, result.errorCode);
    } catch (e, st) {
      if (kDebugMode) debugPrint('[rust] 配图上传出错: $e\n$st');
      return Error('$e');
    }
  }

  /// 取消一个进行中的上传。`null` 与未知 id 都是空操作。
  static void cancel(int? uploadId) {
    if (uploadId == null || !RustCore.isReady) return;
    try {
      api.uploadCancel(uploadId: uploadId);
    } catch (_) {
      // 取消本身失败没有可做的补救，也不该把它抛给调用点——那只会让
      // finally 里的兜底取消变成一个新的崩溃点。
    }
  }

  /// 上传失败是不是因为用户自己取消的。取消不该弹 toast。
  ///
  /// 判的是 `errorDetail`，因为 [LoadingState] 的 `Error` 只带文案与业务码，
  /// 不带 `errorKind`。两处取消文案都由 Rust 侧给，是同一个常量。
  static bool isCancelled(LoadingState<Object?> state) =>
      state is Error && state.errMsg == '上传已取消';

  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '上传失败', code: code) : Success(data);
}
