import 'dart:typed_data';

import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/im.dart' as rust_api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_grpc.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:flutter/foundation.dart';
import 'package:uuid/v4.dart';

export 'package:kilikili/rust/api/im.dart'
    show
        ImEmote,
        ImKeywordBlocking,
        ImMessage,
        ImMessagePage,
        ImMsgKind,
        ImSession,
        ImSessionOffset,
        ImSessionPage,
        ImSessionTarget,
        ImSessionTargetKind,
        ImSetting,
        ImSettingOption,
        ImSettingPage,
        ImSettingRedirect,
        ImSettingsSnapshot,
        ImShareTarget,
        ImThreeDotItem,
        ImThreeDotKind,
        ImTotalUnread,
        ImUnreadStyle;

/// 私信（IM）的调用面（S12 续单 / issue #63）。
///
/// 基线 `lib/grpc/im.dart` 的 `ImGrpc` 整体搬到 Rust：请求、解帧、
/// protobuf 解码与投影都在那边，这里只剩账号注入与一点参数整形。
/// `lib/grpc/bilibili/{app/im,im}/**` 已随本切片删除，按 ADR-011 决策四
/// 不留降级开关。
///
/// ## 导出面上没有 Int64，也没有 protobuf 类型
///
/// 基线满屏的 `Int64` 来自 protobuf 的 Dart 实现。FRB 这边 i64 就是
/// Dart 的 int，调用点不必再 `.toInt()`；会话树也已经在 Rust 侧投影完
/// （头像三件套、折叠入口的页面类型、`vip_info` 那段 JSON），见
/// `bili_api::grpc_im`。
///
/// ## 发私信：dev_id 在这里生成，而且只生成一次
///
/// 服务端拿 `dev_id` 去重。基线每次调用 `ImGrpc.sendMsg` 都现生成一个
/// UUID，等于告诉服务端「每次都是新的一条」——真发生重发时去重就失效了。
///
/// [sendMessage] 因此在**一次逻辑发送内**生成一次（见那里的说明）。
/// Rust 侧对这条的定级是 `Unsafe`：传输层一次都不重发，`dev_id` 是第二
/// 道闸，不是放开重试的理由（ADR-013 补充二十）。
abstract final class RustIm {
  static const String _notReady = 'Rust Core 未就绪，私信暂不可用';

  /// 分享面板默认取几个「最近聊过的人」。与基线一致。
  static const int shareListSize = 5;

  /// 基线 `getTotalUnread` 固定传 2。
  static const int totalUnreadType = 2;

  /// 主会话列表那一页的页面类型。
  ///
  /// 由 Rust 给，不在 Dart 写死：这个数字来自协议的 `SessionPageType`，
  /// 而同名的 `SessionType` 枚举里同一个数字是另一件事。其余页面类型都
  /// 随 `ImSessionTarget.foldPageType` 过来，已经映射好。
  static final int sessionPageTypeHome = rust_api.imHomePageType();

  /// 把消息类型换成协议取值。界面不会发的那几种返回 null。
  static int? msgTypeOf(rust_api.ImMsgKind kind) =>
      rust_api.imMsgType(kind: kind);

  /// 界面会主动打开的四个设置页。
  ///
  /// 协议的 `ImSettingType` 有二十多个取值，这四个数字相邻且无规律
  /// （0、17、18、19）。由 Rust 给，别在 Dart 写死；其余的都由
  /// `ImSettingRedirect.parentSettingType` 带过来。
  static final int settingPageAll = rust_api.imSettingType(
    page: rust_api.ImSettingPage.all,
  );
  static final int settingPageReplyMe = rust_api.imSettingType(
    page: rust_api.ImSettingPage.replyMe,
  );
  static final int settingPageAtMe = rust_api.imSettingType(
    page: rust_api.ImSettingPage.atMe,
  );
  static final int settingPageReceiveLike = rust_api.imSettingType(
    page: rust_api.ImSettingPage.receiveLike,
  );

  // ------------------------------------------------------------ 会话列表

  /// 主会话列表。首屏传 `null`。
  static Future<LoadingState<rust_api.ImSessionPage>> sessionMain({
    List<rust_api.ImSessionOffset>? offsets,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImSessionMain(
        context: RustGrpc.context(),
        offsets: offsets ?? const [],
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('私信会话列表', e, st);
    }
  }

  /// 二级会话列表。
  ///
  /// [pageType] 要用 [rust_api.ImSessionTarget.foldPageType]——那是 Rust
  /// 映射好的页面类型，**不是**会话类型。两个枚举的数字几乎每行都不同。
  static Future<LoadingState<rust_api.ImSessionPage>> sessionSecondary({
    required int pageType,
    List<rust_api.ImSessionOffset>? offsets,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImSessionSecondary(
        context: RustGrpc.context(),
        pageType: pageType,
        offsets: offsets ?? const [],
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('二级会话列表', e, st);
    }
  }

  /// 单个会话的最新状态。界面用它查「现在是不是置顶」。
  ///
  /// 服务端查不到这个会话时返回 `Success(null)`——那是「没有」，不是失败。
  static Future<LoadingState<rust_api.ImSession?>> session(
    int talkerUid,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImSession(
        context: RustGrpc.context(),
        talkerUid: talkerUid,
      );
      return res.errorKind == null
          ? Success(res.data)
          : Error(res.errorDetail ?? '请求失败', code: res.errorCode);
    } catch (e, st) {
      return _crashed('会话状态', e, st);
    }
  }

  // ------------------------------------------------------ 会话上的写操作

  /// 置顶 / 取消置顶。两边都是置为语义，重发安全。
  static Future<LoadingState<void>> setPinned({
    required int talkerUid,
    required bool pinned,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final context = RustGrpc.context();
      return _unwrapWrite(
        pinned
            ? await rust_api.pinImSession(
                context: context,
                talkerUid: talkerUid,
              )
            : await rust_api.unpinImSession(
                context: context,
                talkerUid: talkerUid,
              ),
      );
    } catch (e, st) {
      return _crashed('会话置顶', e, st);
    }
  }

  /// 把某一页的未读全部清掉。
  static Future<LoadingState<void>> clearUnread(int pageType) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.clearImUnread(
          context: RustGrpc.context(),
          pageType: pageType,
        ),
      );
    } catch (e, st) {
      return _crashed('清除未读', e, st);
    }
  }

  /// 删掉某一页的整个会话列表。
  static Future<LoadingState<void>> deleteSessionList(int pageType) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.deleteImSessionList(
          context: RustGrpc.context(),
          pageType: pageType,
        ),
      );
    } catch (e, st) {
      return _crashed('删除会话列表', e, st);
    }
  }

  // ---------------------------------------------------------------- 消息

  /// 拉一页会话消息。[endSeqno] 是上一页最老那条的 seqno；首屏传 `null`。
  static Future<LoadingState<rust_api.ImMessagePage>> messages({
    required int talkerId,
    int? endSeqno,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImMessages(
        context: RustGrpc.context(),
        talkerId: talkerId,
        endSeqno: endSeqno,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('会话消息', e, st);
    }
  }

  /// 发一条私信。**传输层一次都不重发。**
  ///
  /// [devId] 服务端拿来去重。不传时这里生成一个——那是「用户按了一次
  /// 发送」的语义。**要重试同一条消息时必须把上一次的 devId 传回来**，
  /// 否则服务端会当成新的一条，用户看到的就是发了两遍。
  ///
  /// 基线每次调用都现生成，等于永远走「新的一条」这条路；把它提成参数
  /// 是为了让这个区别留在看得见的地方（ADR-013 补充二十）。
  static Future<LoadingState<void>> sendMessage({
    required int senderUid,
    required int receiverId,
    required int msgType,
    required String content,
    String? devId,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.sendImMessage(
          context: RustGrpc.context(),
          senderUid: senderUid,
          receiverId: receiverId,
          msgType: msgType,
          content: content,
          timestamp: DateTime.now().millisecondsSinceEpoch ~/ 1000,
          devId: devId ?? const UuidV4().generate(),
        ),
      );
    } catch (e, st) {
      return _crashed('发送私信', e, st);
    }
  }

  /// 取会话的已读位点。「标为已读」要拿它当参数。
  static Future<LoadingState<int>> ackSeqno(int talkerId) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImAckSeqno(
        context: RustGrpc.context(),
        talkerId: talkerId,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('已读位点', e, st);
    }
  }

  /// 「最近聊过的人」，分享面板用。
  static Future<LoadingState<List<rust_api.ImShareTarget>>> shareList({
    int size = shareListSize,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImShareList(
        context: RustGrpc.context(),
        size: size,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('分享列表', e, st);
    }
  }

  /// 消息中心四个 Tab 的未读数（gRPC 版）。
  ///
  /// 与 `RustMsg.msgFeedUnread`（REST）同义不同源：首页红点用那条，
  /// 私信列表页用这条——它顺手从同一个响应里拿，省一次往返。
  static Future<LoadingState<rust_api.ImTotalUnread>> totalUnread() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImTotalUnread(
        context: RustGrpc.context(),
        unreadType: totalUnreadType,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('未读总数', e, st);
    }
  }

  // ------------------------------------------------------- 设置与屏蔽词

  /// 私信设置页。返回里的 `raw` 是一段**不透明字节**，写回时原样传回。
  static Future<LoadingState<rust_api.ImSettingsSnapshot>> settings(
    int settingType,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImSettings(
        context: RustGrpc.context(),
        settingType: settingType,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('私信设置', e, st);
    }
  }

  /// 改一项设置并写回整张表。
  ///
  /// [raw] 是 [settings] 带回来的那段字节，原样传。[switchOn] 与
  /// [selectedValue] 二选一；后者传的是 `ImSettingOption.value`，
  /// **不是下标**。
  ///
  /// 为什么要把整段字节带回来：`SetImSettings` 收的是整张设置表，按
  /// 投影出来的窄实体重建会发出一个缺字段的赝品，把用户没碰过的设置
  /// 悄悄抹掉。Rust 侧拿字节重新解码、只改一处、再整张发回去。
  static Future<LoadingState<void>> setSetting({
    required Uint8List raw,
    required int key,
    bool? switchOn,
    int? selectedValue,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.setImSetting(
          context: RustGrpc.context(),
          raw: raw,
          key: key,
          switchOn: switchOn,
          selectedValue: selectedValue,
        ),
      );
    } catch (e, st) {
      return _crashed('保存私信设置', e, st);
    }
  }

  /// 关键词屏蔽列表。
  static Future<LoadingState<rust_api.ImKeywordBlocking>>
  keywordBlocking() async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await rust_api.fetchImKeywordBlocking(
        context: RustGrpc.context(),
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('屏蔽词列表', e, st);
    }
  }

  /// 加一个屏蔽词。**失败不自动重试**：重发会多一条，而列表查得到，
  /// 要不要再加由用户决定（ADR-013 决策一的「可判定」档）。
  static Future<LoadingState<void>> addKeyword(String keyword) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.addImKeywordBlocking(
          context: RustGrpc.context(),
          keyword: keyword,
        ),
      );
    } catch (e, st) {
      return _crashed('添加屏蔽词', e, st);
    }
  }

  /// 删一个屏蔽词。
  static Future<LoadingState<void>> deleteKeyword(String keyword) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      return _unwrapWrite(
        await rust_api.deleteImKeywordBlocking(
          context: RustGrpc.context(),
          keyword: keyword,
        ),
      );
    } catch (e, st) {
      return _crashed('删除屏蔽词', e, st);
    }
  }

  /// 写操作的结果没有载荷：`errorKind` 为空就是成功。
  static LoadingState<void> _unwrapWrite(WriteResult result) =>
      result.errorKind == null
      ? const Success(null)
      : Error(result.errorDetail ?? '请求失败', code: result.errorCode);

  /// Rust 侧的失败一律是值：`data` 为空就说明这次没拿到。
  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '请求失败', code: code) : Success(data);

  /// 桥接层自身出错。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what 出错: $e\n$st');
    return Error('$e');
  }
}
