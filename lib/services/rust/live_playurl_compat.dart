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
// 1. **另外两个子类还没迁。** sealed class 的子类必须同文件（它们是
//    `part`），只迁走一个就得把层级拆了，而拆它要连视频取流一起动——
//    那是另一个切片（`ugcUrl`/`pgcUrl`，仍在 EPIC #67 的待办里）。
// 2. **一半的消费点与取流无关。** 设置页的画质下拉读
//    `LiveQuality.values` 列全部档位，`Pref.liveQuality` /
//    `liveQualityCellular` 的默认值读 `LiveQuality.origin.code` /
//    `superHD.code`。这两处都不经过网络响应，把类型换成 FRB 结构等于
//    让设置页依赖一次取流。
// 3. **它带着一张客户端静态表。** 原画 10000、蓝光 400、超清 250……
//    那不是协议数据，是本端对码率号的命名，响应里的 `g_qn_desc` 缺哪档
//    就由它补哪档。表留在 Dart，补齐也就留在 Dart。
//
// **随哪个切片删除**：`BaseQuality` 的最后一个子类迁完时——即视频取流
// （`VideoQuality` / `AudioQuality`）落地之后。届时本文件连同
// `live_playurl_parity_test.dart` 一并删除。
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
