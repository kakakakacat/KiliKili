import 'package:kilikili/grpc/bilibili/main/community/reply/v1.pb.dart'
    show ReplyInfo, Url;
import 'package:kilikili/rust/api/reply_filter.dart' as rust_api;
import 'package:kilikili/services/rust_core.dart';

export 'package:kilikili/rust/api/reply_filter.dart'
    show ReplyFilterSettings, ReplyUrlExtra;

/// 评论过滤规则的调用面（S2 / issue #52）。
///
/// 规则本身在 Rust（`bili_domain::reply_filter`）：带货、纯 @、@ 数量、
/// 关键词正则，以及拉黑的 UP 主。设置的取值每次由调用点传入——用户随时
/// 能在设置页改，Rust 缓存一份必然漂移。
///
/// ## 为什么这里还认识 protobuf
///
/// 本切片只迁**规则**，gRPC 传输与消息解码仍在 Dart（见 #52 的续单）。
/// 于是 [_project] 把 `ReplyInfo` 削成规则真正要读的那几个字段递过去。
///
/// **这段投影随 gRPC 传输那一片删除**：那之后评论由 Rust 自己解码，规则
/// 直接作用在领域实体上，宿主不再需要把字段搬一遍。按 AGENTS.md §2 的
/// 要求把「为什么留」和「随哪个切片删」写在这里，免得下一个人当成疏漏
/// 再迁一次。
///
/// ## 失败不是「没有要过滤的」
///
/// 关键词正则在两端语义不同（如 `\d`）或 Rust 编不过（如 lookahead）时
/// [judge] 返回 null。调用点必须把**整个响应**降级成 `Error`，不能当成
/// 「没有要过滤的」继续——用户明确屏蔽过的评论照常显示且没有任何提示，
/// 比列表加载失败更糟（同 `RustVideoFeed`，理由见 ADR-010 决策二）。
abstract final class RustReplyFilter {
  /// 对一批评论做一次判定。失败返回 null。
  ///
  /// 一次递一页而不是逐条调用：基线是 `replies.removeWhere(needRemoveGrpc)`，
  /// 照搬成每条一次跨 FFI，一页就是几十到上百次调用（RFC §2.2 要避免的
  /// 形状），而且关键词正则会逐条重新编译。
  static ReplyVerdict? judge(
    rust_api.ReplyFilterSettings settings,
    List<ReplyInfo> replies,
  ) {
    if (!RustCore.isReady) return null;
    if (replies.isEmpty) return const ReplyVerdict.empty();

    final result = rust_api.replyFilterMask(
      settings: settings,
      items: replies.map(_project).toList(),
    );

    final mask = result.data;
    if (mask == null || mask.length != replies.length) return null;

    // 用 Map.identity 而不是普通 Map：protobuf 生成类按值比较相等，同一页
    // 里两条内容完全相同的评论会被当成同一个键。那两条的判定结果其实一样，
    // 换普通 Map 也不会错，但依赖「结果恰好相同」来保证正确性是个容易在
    // 以后被改坏的前提。
    final verdicts = Map<ReplyInfo, bool>.identity();
    for (var i = 0; i < replies.length; i++) {
      verdicts[replies[i]] = mask[i];
    }
    return ReplyVerdict._(verdicts);
  }

  static rust_api.ReplyFilterItem _project(ReplyInfo reply) {
    final content = reply.content;
    return rust_api.ReplyFilterItem(
      mid: reply.mid.toInt(),
      message: content.message,
      atNames: content.atNameToMid.keys.toList(),
      hasEmotes: content.emotes.isNotEmpty,
      hasPictures: content.pictures.isNotEmpty,
      // 基线 `_hasGoods` 第一步就是 `url.hasExtra()`，没有 extra 的直接
      // 判 false，所以这里只递带 extra 的那些，Rust 侧不必再表达「有没有」。
      urlExtras: [
        for (final Url url in content.urls.values)
          if (url.hasExtra())
            rust_api.ReplyUrlExtra(
              goodsCmControl: url.extra.goodsCmControl.toInt(),
              hasGoodsItemId: url.extra.hasGoodsItemId(),
              hasGoodsPrefetchedCache: url.extra.hasGoodsPrefetchedCache(),
            ),
      ],
      root: reply.root.toInt(),
      parent: reply.parent.toInt(),
    );
  }
}

/// 一批评论的判定结果，按**对象身份**查表。
class ReplyVerdict {
  final Map<ReplyInfo, bool>? _verdicts;

  const ReplyVerdict._(Map<ReplyInfo, bool> verdicts) : _verdicts = verdicts;

  /// 空批次：没有东西要判，什么都不移除。
  const ReplyVerdict.empty() : _verdicts = null;

  /// 这条评论是否要移除。可以直接当谓词传给 `removeWhere`。
  ///
  /// 查不到时返回 false（不过滤）。查不到意味着调用点漏把它放进
  /// [RustReplyFilter.judge] 的那一批里，是代码错误而不是运行时情况；
  /// 调用点用同一个收集函数产出「送去判定的列表」与「随后遍历的对象」，
  /// 正是为了让它不可能发生。
  bool removes(ReplyInfo reply) => _verdicts?[reply] ?? false;
}
