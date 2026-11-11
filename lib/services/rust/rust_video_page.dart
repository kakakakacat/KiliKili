import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/rust/api/video_page.dart' as rust_api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/video_page.dart'
    show
        AiConclusion,
        AiOutline,
        AiOutlinePart,
        HistoryNode,
        Interaction,
        NoteAuthor,
        NoteItem,
        NoteList,
        PlayInfo,
        PlaySubtitle,
        VideoShot,
        ViewPoint;

/// 播放页附属只读接口的取数路径：AI 总结、在线人数、预览缩略图、播放信息、
/// 笔记列表（S3 第二批 / issue #53）。
///
/// 基线对应的 `VideoHttp.aiConclusion` 等五个函数已随本次迁移删除，按
/// ADR-011 决策四不留降级开关。
///
/// ## 账号归属
///
/// 这五条在 `ApiType.apiTypeSet` 里分属两个账号：`onlineTotal` 与
/// `aiConclusion` 走 `heartbeat`，其余没有登记的走 `main`。必须与拦截器的
/// 选择一致，否则两条路径用的是不同账号。
abstract final class RustVideoPage {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  static Future<rust_api.VideoPageContext> _context({
    required AccountType account,
    required String path,
    String mixinKey = '',
    List<(String, String)>? extraHeaders,
  }) async {
    final acc = Accounts.get(account);
    return rust_api.VideoPageContext(
      baseUrl: HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(acc, path),
      headers: [...RustRequest.headersFor(acc), ...?extraHeaders],
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
      mixinKey: mixinKey,
      wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
    );
  }

  /// 取 WBI mixin key。拿不到就别发请求——基线会用空串继续签名并换回一个
  /// 业务错误码，那只是把失败推后，还更难诊断。
  static Future<String?> _mixinKey() async {
    final key = await WbiSign.getWbiKeys();
    return key.isEmpty ? null : key;
  }

  /// 同时在看人数。返回的是服务端排好版的字符串（如 `"1.2万"`）。
  static Future<LoadingState<String>> onlineTotal({
    int? aid,
    String? bvid,
    required int cid,
  }) async {
    assert(aid != null || bvid != null);
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchOnlineTotal(
        context: await _context(
          account: AccountType.heartbeat,
          path: Api.onlineTotal,
        ),
        aid: aid,
        bvid: bvid,
        cid: cid,
      );
      final data = result.data;
      if (data == null) return Error(_message(result.errorDetail));
      return Success(data);
    } catch (e, st) {
      return _crashed('在线人数', e, st);
    }
  }

  /// AI 视频总结。
  ///
  /// 失败时 `code` 是**内层** `data.code`：`1` 表示 AI 仍在处理，调用点据此
  /// 提示「稍后再试」而不是「不支持」。
  static Future<LoadingState<rust_api.AiConclusion>> aiConclusion({
    required String bvid,
    required int cid,
    int? upMid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchAiConclusion(
        context: await _context(
          account: AccountType.heartbeat,
          path: Api.aiConclusion,
          mixinKey: mixinKey,
        ),
        bvid: bvid,
        cid: cid,
        upMid: upMid,
      );
      final data = result.data;
      if (data == null) {
        return Error(_message(result.errorDetail), code: result.errorCode);
      }
      return Success(data);
    } catch (e, st) {
      return _crashed('AI 总结', e, st);
    }
  }

  /// 播放信息：续播位置、字幕、分段看点、互动视频入口。
  static Future<LoadingState<rust_api.PlayInfo>> playInfo({
    int? aid,
    String? bvid,
    required int cid,
    int? seasonId,
    int? epId,
  }) async {
    assert(aid != null || bvid != null);
    if (!RustCore.isReady) return const Error(_notReady);
    final mixinKey = await _mixinKey();
    if (mixinKey == null) return const Error('WBI 签名密钥获取失败');
    try {
      final result = await rust_api.fetchPlayInfo(
        context: await _context(
          account: AccountType.main,
          path: Api.playInfo,
          mixinKey: mixinKey,
        ),
        aid: aid,
        bvid: bvid,
        cid: cid,
        seasonId: seasonId,
        epId: epId,
      );
      final data = result.data;
      if (data == null) return Error(_message(result.errorDetail));
      return Success(data);
    } catch (e, st) {
      return _crashed('播放信息', e, st);
    }
  }

  /// 进度条预览缩略图。
  ///
  /// 这个视频没有缩略图时同样是失败（`errorKind == 'remote'`），与基线一致：
  /// 播放器据此不显示预览，而不是拿着空列表去索引。
  static Future<LoadingState<rust_api.VideoShot>> videoShot({
    required String bvid,
    required int cid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchVideoShot(
        context: await _context(
          account: AccountType.main,
          path: Api.videoshot,
          // 基线对这条额外带了桌面端 UA 与视频页 referer。
          extraHeaders: [
            ('user-agent', BrowserUa.pc),
            ('referer', 'https://www.bilibili.com/video/$bvid'),
          ],
        ),
        bvid: bvid,
        cid: cid,
      );
      final data = result.data;
      if (data == null) return Error(_message(result.errorDetail));
      return Success(data);
    } catch (e, st) {
      return _crashed('预览缩略图', e, st);
    }
  }

  /// 视频笔记列表的一页。
  static Future<LoadingState<rust_api.NoteList>> noteList({
    required int oid,
    required int page,
    int? uperMid,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final result = await rust_api.fetchNoteList(
        context: await _context(
          account: AccountType.main,
          path: Api.archiveNoteList,
        ),
        // 只读接口却要 csrf，是服务端的要求；真相源在 Dart 的账号体系。
        csrf: Accounts.main.csrf,
        oid: oid,
        page: page,
        uperMid: uperMid,
      );
      final data = result.data;
      if (data == null) return Error(_message(result.errorDetail));
      return Success(data);
    } catch (e, st) {
      return _crashed('笔记列表', e, st);
    }
  }

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] 取$what出错: $e\n$st');
    return Error('$e');
  }

  /// Rust 侧的 `errorDetail` 已经是可直接展示的文案；兜底一句免得空白。
  static String _message(String? detail) => detail ?? '请求失败';
}

/// 雪碧图每张图里有几格。基线把它放在 `VideoShotData` 上，模型删掉之后
/// 作为扩展留在这里——进度条预览的取图算法要用。
extension VideoShotLayout on rust_api.VideoShot {
  int get totalPerImage => imgXLen * imgYLen;
}
