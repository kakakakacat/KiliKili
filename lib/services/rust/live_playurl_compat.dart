import 'package:kilikili/models/common/video/base_quality.dart';
import 'package:kilikili/rust/api/live.dart';

// 直播取流的**单向适配**（ADR-011 决策六）。
//
// S11a 第二批把 `liveRoomInfo` 的解析迁到了 Rust，七个 Dart 模型
// （`RoomPlayInfoData` / `PlayurlInfo` / `Playurl` / `Stream` / `Format` /
// `CodecItem` / `UrlInfo`）随之删除。唯独画质没跟着走。
//
// ## 为什么 `LiveQuality` 留在 Dart
//
// 它是 `BaseQuality` 这个 sealed class 的子类，和 `VideoQuality`、
// `AudioQuality` 同属一个封闭层级。三件事让它搬不动：
//
// 1. ~~另外两个子类还没迁。~~ **2026-11-11 已不成立**：视频取流
//    （#79）迁完后 `base_quality.dart` 不再 import 取流模型，层级的
//    外部耦合解开了。但下面两条与层级无关，所以本文件还是留着。
// 2. **一半的消费点与取流无关。** 设置页的画质下拉读
//    `LiveQuality.values` 列全部档位，`Pref.liveQuality` /
//    `liveQualityCellular` 的默认值读 `LiveQuality.origin.code` /
//    `superHD.code`。这两处都不经过网络响应，把类型换成 FRB 结构等于
//    让设置页依赖一次取流。
// 3. **它带着一张客户端静态表。** 原画 10000、蓝光 400、超清 250……
//    那不是协议数据，是本端对码率号的命名，响应里的 `g_qn_desc` 缺哪档
//    就由它补哪档。表留在 Dart，补齐也就留在 Dart。
//
// **随哪个切片删除**：随**画质设置项迁移**——那一片要把
// `VideoQuality` / `AudioQuality` / `LiveQuality` 三张客户端静态表一起
// 下沉 Rust，并改掉设置页下拉、`Pref` 默认值与一串 `Rxn<VideoQuality>`。
// 届时本文件连同 `live_playurl_parity_test.dart` 一并删除。
//
// 这个期限是**修正过的**：原先写的是「随视频取流迁完」，而取流
// （#79）迁完时只消掉了上面三条理由里的第一条。写「随 X 一并删」时要
// 逐条检查每个理由是不是都随 X 消失（ADR-011 补充十四）。
//
// ## 适配是单向的
//
// 只有 Rust → Dart。`LiveQuality` 不会写回 FRB 结构，也不该写回：
// 用户在选择器里选的那一档只是**下一次请求的入参**，发回去的是
// `qn.code`，不是这个对象。

/// 把 Rust 的一档画质转成 `LiveQuality`。
///
/// 逐行复刻基线 `LiveQuality.fromFormat` 的三级回退：
///
/// 1. 响应的 `g_qn_desc` 里有这一档 → 用它给的名字（[LiveQn.desc] 非空
///    就表示命中，解析时已经查过表了）；
/// 2. 没有 → 查客户端静态表 `LiveQuality.fromCode`；
/// 3. 静态表也没有 → `fromId`，名字退化成码率号的十进制串。
///
/// 第 2、3 步都在 `fromCode` 内部，所以这里只需分两支。
LiveQuality toLiveQuality(LiveQn qn) {
  final desc = qn.desc;
  return desc == null
      ? LiveQuality.fromCode(qn.code)
      : LiveQuality(qn.code, desc);
}

/// [toLiveQuality] 的列表版，用于 `accept_qn`。
List<LiveQuality> toLiveQualityList(List<LiveQn> qns) =>
    qns.map(toLiveQuality).toList();
