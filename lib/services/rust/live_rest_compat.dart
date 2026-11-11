import 'package:kilikili/models_new/live/live_medal_wall/uinfo_medal.dart';
import 'package:kilikili/rust/api/live.dart';

// 直播剩余只读接口的**单向适配**（ADR-011 决策六）。
//
// S11a 第三批把表情包、直播搜索、屏蔽设置、贡献榜、勋章墙、勋章名六条
// 迁到了 Rust。其中只有**粉丝勋章**没跟着走。
//
// ## 为什么 `UinfoMedal` 留在 Dart
//
// 它有四个生产者，本批只迁得动其中两个：
//
// | 生产者 | 归属 |
// |---|---|
// | 贡献榜 `uinfo.medal` | 本批，已迁 |
// | 勋章墙 `uinfo_medal` | 本批，已迁 |
// | SuperChat `SUPER_CHAT_MESSAGE` | **S11b**（#62） |
// | 弹幕消息 `live_danmaku/danmaku_msg.dart` | **S11b**（#62） |
//
// 后两个都在弹幕长连接那条路径上，而 S11b 阻塞在 ADR-012（#49，连文档
// 都还没有）。按决策六，共享模型要等**所有**生产者迁完才能删，所以
// `UinfoMedal` 连同渲染它的 `lib/pages/member/widget/medal_widget.dart`
// 一起留着，本文件负责把 Rust 的 [LiveMedal] 转过去。
//
// **随哪个切片删除**：S11b（#62）迁完弹幕与 SuperChat 时，本文件连同
// `live_rest_parity_test.dart` 一并删除。
//
// ## 为什么 SuperChat 不在本批
//
// 同一个理由的更强版本。`SuperChatItem` 自己就有两个生产者——REST 的
// `superChatMsg` 与 WebSocket 的 `SUPER_CHAT_MESSAGE`（见
// `live_room/controller.dart` 里 `case 'SUPER_CHAT_MESSAGE'`，它直接
// 调 `SuperChatItem.fromJson`）。只迁 REST 那条，WebSocket 那条仍在
// 构造同一个 Dart 类，模型一行都删不掉，只多出一个适配层。
//
// 它还有两个与生产者无关的障碍：`expired` / `deleted` 是**可变**的
// 客户端状态（`superChatMsg.removeWhere((e) => e.expired)`），而 FRB
// 结构不可变；解析时读 `GlobalData().showMedal` 这个全局设置，等于把
// 客户端状态混进了协议解析。**SuperChat 整条留给 S11b**。
//
// ## 适配是单向的
//
// 只有 Rust → Dart。`UinfoMedal` 上的改动不会写回 FRB 结构。

/// 把 Rust 的粉丝勋章转成 Dart 的 `UinfoMedal`。
///
/// 字段名在 Rust 侧去掉了协议的 `v2_` 前缀（那是上游的字段版本号，不是
/// 语义），这里转回基线的命名。
UinfoMedal toUinfoMedal(LiveMedal medal) => UinfoMedal(
  name: medal.name,
  level: medal.level,
  id: medal.id,
  ruid: medal.ruid,
  v2MedalColorStart: medal.colorStart,
  v2MedalColorText: medal.colorText,
);

/// [toUinfoMedal] 的可空版，省去调用点逐个写三元表达式。
UinfoMedal? toUinfoMedalOrNull(LiveMedal? medal) =>
    medal == null ? null : toUinfoMedal(medal);
