import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/models/user/danmaku_rule.dart';
import 'package:kilikili/rust/api/danmaku.dart' as rust_api;
import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:fixnum/fixnum.dart';
import 'package:flutter/foundation.dart';

/// 弹幕引擎的 Rust 会话。一个 cid 一个会话，与 `PlDanmakuController` 同生命周期。
///
/// 与其他 Rust 封装（搜索建议、视频详情）不同，这条路径**持有状态**：
/// 一个视频已解出的弹幕全在 Rust 侧的会话里。因此这里的每个方法都遵循
/// 同一条约定——**失败返回 null / false，不抛异常**，由调用方决定降级，
/// 而不是在这里偷偷吞掉一个分段。吞掉的表现是「这几分钟没人发弹幕」，
/// 界面上没有任何提示，是最难查的那类故障。
class RustDanmakuSession {
  RustDanmakuSession._(this._id);

  final int _id;
  bool _closed = false;

  /// 开一个会话。开关关闭、Rust 未就绪、规则编不过时返回 null，调用方
  /// 走 Dart 原路径。
  ///
  /// 规则编不过是**预期内**的失败：用户的正则来自 `Pref.danmakuFilterRule`，
  /// Dart `RegExp` 支持而 Rust `regex` 不支持（或语义不同）的那些会被
  /// 拒绝。此时整条弹幕链路回到基线，而不是少过滤一条（ADR-010）。
  static RustDanmakuSession? open({
    required bool isLogin,
    required String midHash,
    required bool mergeDanmaku,
    required RuleFilter filters,
  }) {
    if (!RustFlags.rustDanmakuEngine) return null;
    try {
      final result = rust_api.danmakuOpen(
        options: rust_api.DanmakuEngineOptions(
          isLogin: isLogin,
          selfMidHash: midHash,
          merge: mergeDanmaku,
          blockKeywords: filters.dmFilterString,
          // 存进 Hive 的就是 pattern 本身（两端斜杠在构造 RegExp 前已被
          // 剥掉），Rust 侧按同样的源串编译。
          blockRegexes: filters.dmRegExp.map((e) => e.pattern).toList(),
          blockUids: filters.dmUid.toList(),
        ),
      );
      final id = result.sessionId;
      if (id == null) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 弹幕会话未建立，降级回 Dart: '
            '${result.errorKind} / ${result.errorDetail}',
          );
        }
        return null;
      }
      return RustDanmakuSession._(id);
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 弹幕桥接失败，降级回 Dart: $e');
      return null;
    }
  }

  /// 并入一个分段（**解压后**的 protobuf 字节）。
  ///
  /// 返回 null 表示失败，调用方必须整条降级回 Dart。异步：解码几千条
  /// protobuf 在 Rust 工作线程上做，不占 Dart 线程。
  Future<rust_api.DanmakuPushResult?> pushSegment(
    int segmentIndex,
    Uint8List bytes,
  ) async {
    if (_closed) return null;
    try {
      final result = await rust_api.danmakuPushSegment(
        sessionId: _id,
        segmentIndex: segmentIndex,
        bytes: bytes,
      );
      if (!result.ok) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 弹幕分段 $segmentIndex 并入失败，降级回 Dart: '
            '${result.errorKind} / ${result.errorDetail}',
          );
        }
        return null;
      }
      return result;
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 弹幕分段并入异常，降级回 Dart: $e');
      return null;
    }
  }

  /// 取某个播放进度所在的 100ms 桶，重建成基线的 `DanmakuElem`。
  ///
  /// 重建而不是换一种类型，是为了让渲染侧（`lib/pages/danmaku/view.dart`）
  /// 一行都不用改：两条路径交给它的是同一种对象，差别只在谁算出来的。
  ///
  /// 这是每 100ms 一次的热路径，对应的 Rust 函数是 `#[frb(sync)]`——
  /// 返回 Future 的话调用方就得改成异步，弹幕会晚一帧出现。
  List<DanmakuElem>? bucket(int progressMs) {
    if (_closed) return null;
    try {
      final items = rust_api.danmakuBucket(
        sessionId: _id,
        progressMs: progressMs,
      );
      if (items == null || items.isEmpty) return null;
      return items.map(_toElem).toList(growable: false);
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 取弹幕桶失败: $e');
      return null;
    }
  }

  /// 关闭并释放会话持有的全部弹幕。重复关闭无副作用。
  ///
  /// **必须**调用：不关就是泄漏一个视频的全部弹幕，而泄漏在界面上没有
  /// 任何症状。
  void close() {
    if (_closed) return;
    _closed = true;
    try {
      rust_api.danmakuClose(sessionId: _id);
      if (kDebugMode) {
        // 剩余会话数应当随页面退出归零。泄漏在界面上没有任何症状，
        // 只能靠这一行看出来（ADR-010 的真机验收项之一）。
        debugPrint(
          '[rust] 弹幕会话 $_id 已关闭，'
          '剩余 ${rust_api.danmakuOpenSessionCount()} 个',
        );
      }
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 关闭弹幕会话失败: $e');
    }
  }
}

/// 把 Rust 返回的一条弹幕重建成基线的 `DanmakuElem`。
///
/// 只有渲染与交互要用的字段（见 Rust 侧 `DanmakuBucketItem` 的说明）；
/// 其余字段取 proto3 零值。
DanmakuElem _toElem(rust_api.DanmakuBucketItem item) => DanmakuElem()
  ..id = Int64(item.id)
  ..progress = item.progress
  ..mode = item.mode
  ..fontsize = item.fontsize
  ..color = item.color
  ..midHash = item.midHash
  ..content = item.content
  ..weight = item.weight
  ..likeCount = Int64(item.likeCount)
  // 认不出的取值退回默认，与 protobuf.dart 解到未知枚举时的行为一致。
  ..colorful = DmColorfulType.valueOf(item.colorful) ?? DmColorfulType.NoneType
  ..count = item.count
  ..isSelf = item.isSelf;

/// 播放进度游标：决定一帧进度该不该取桶、要不要先清屏（issue #89）。
///
/// 判据全在 Rust（`bili_danmaku::cursor`），这里**只存两个数字**。之所以
/// 不跟 [RustDanmakuSession] 绑在一起，是因为它必须对两条路径都生效：
/// Rust 引擎开着时取 Rust 的桶，引擎降级回 Dart 时取 `_dmSegMap` 的桶，
/// 而降级恰恰发生在 Rust 会话建不起来的时候。挂在会话上，降级路径就
/// 没人看门了。
class DanmakuProgressCursor {
  int _lastBucketMs = -1;
  int _seenEpoch = 0;

  /// 判定这一帧。返回 null 表示**跳过**：不取桶，也不要触发分段请求。
  ///
  /// 跨 FFI 失败时同样返回 null。这里刻意**不**在 Dart 里补一份判据
  /// 作为兜底：取流本身（#79）已经走 Rust，桥接真的坏了的话根本没有
  /// 视频在播，弹幕怎么判都无所谓；为这种情形留一份平行实现，换来的
  /// 只是两份判据迟早不一致。
  rust_api.DanmakuProgressStep? step(int positionMs, int epoch) {
    try {
      final result = rust_api.danmakuProgressStep(
        lastBucketMs: _lastBucketMs,
        seenEpoch: _seenEpoch,
        positionMs: positionMs,
        epoch: epoch,
      );
      _lastBucketMs = result.lastBucketMs;
      _seenEpoch = result.seenEpoch;
      return result.emit ? result : null;
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 进度游标失败，跳过这一帧: $e');
      return null;
    }
  }
}
