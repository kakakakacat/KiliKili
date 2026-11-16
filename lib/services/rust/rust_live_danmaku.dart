import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;

import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/rust/api/live.dart' show LiveContext;
import 'package:kilikili/rust/api/live_danmaku.dart' as api;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/wbi_sign.dart';

export 'package:kilikili/rust/api/live_danmaku.dart';

/// S11b 的薄宿主入口；账号仍按 ApiType 查表，协议和过滤由 Rust 执行。
abstract final class RustLiveDanmaku {
  static const _notReady = 'Rust Core 未就绪，该功能不可用';

  static api.LiveBlockRules rules(List<String> keywords, Set<int> uids) =>
      api.LiveBlockRules(
        keywords: keywords,
        uids: Int64List.fromList(uids.toList()),
      );

  static Future<LiveContext> _context(String endpoint) async {
    final account = RustRequest.accountFor(endpoint);
    return LiveContext(
      baseUrl: HttpString.liveBaseUrl,
      cookie: await RustRequest.cookieFor(account, endpoint),
      headers: RustRequest.headersFor(account),
      timeoutMs: BigInt.from(10000),
      transport: RustTransport.current(),
    );
  }

  static Future<api.LiveDanmakuTokenResult> token(int roomId) async {
    try {
      if (!RustCore.isReady) throw StateError(_notReady);
      final key = await WbiSign.getWbiKeys();
      if (key.isEmpty) throw StateError('WBI 密钥获取失败，请稍后重试');
      return await api.fetchLiveDanmakuToken(
        context: await _context(Api.liveRoomDmToken),
        roomId: roomId,
        mixinKey: key,
        wts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      );
    } catch (_) {
      return const api.LiveDanmakuTokenResult(
        data: null,
        errorKind: 'network',
        errorDetail: '获取直播弹幕连接信息失败',
        errorCode: null,
      );
    }
  }

  static Future<api.LiveDanmakuPrefetchResult> prefetch(int roomId) async {
    try {
      if (!RustCore.isReady) throw StateError(_notReady);
      return await api.fetchLiveDanmakuPrefetch(
        context: await _context(Api.liveRoomDmPrefetch),
        roomId: roomId,
        userAgent: BrowserUa.pc,
      );
    } catch (_) {
      return const api.LiveDanmakuPrefetchResult(
        data: null,
        errorKind: 'network',
        errorDetail: '获取历史弹幕失败',
        errorCode: null,
      );
    }
  }

  static Future<api.LiveSuperChatResult> superChat(int roomId) async {
    try {
      if (!RustCore.isReady) throw StateError(_notReady);
      return await api.fetchLiveSuperChat(
        context: await _context(Api.superChatMsg),
        roomId: roomId,
      );
    } catch (_) {
      return const api.LiveSuperChatResult(
        data: null,
        errorKind: 'network',
        errorDetail: '获取醒目留言失败',
        errorCode: null,
      );
    }
  }
}
