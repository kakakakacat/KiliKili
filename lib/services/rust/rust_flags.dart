import 'package:PiliPlus/services/rust_core.dart';
import 'package:PiliPlus/utils/storage.dart';
import 'package:flutter/foundation.dart';
import 'package:hive_ce/hive.dart';

/// Rust 迁移的功能开关（RFC-0001 §5.3「发生故障可降级回 Dart」）。
///
/// 开关一律**默认关闭或只影响诊断**：Stage 1 里 Rust 还不是任何显示路径
/// 的数据来源，一个尚未投入使用的组件不该有能力让功能退化。
///
/// ## 为什么不写进 `Pref` / `SettingBoxKey`
///
/// 那两个文件来自上游（PiliPlus），每次同步上游都要合并。重构期新增的
/// 开关放进去，等于给每次同步制造冲突点（RFC §10.1）。这里自带命名空间
/// `rust.*`，共用 `setting` box 但不碰上游的键表。
abstract final class RustFlags {
  static Box get _box => GStorage.setting;

  /// 视频详情 shadow mode：Dart 与 Rust 对**同一份响应**各自规范化后比对，
  /// 显示仍然用 Dart 的结果。
  ///
  /// debug 构建默认开启，release 默认关闭：比对本身只是一次额外解析，
  /// 开销可忽略，但差异日志对普通用户毫无意义。需要在 release 排查时，
  /// 用 [setShadowVideoDetail] 打开。
  static const String shadowVideoDetailKey = 'rust.shadow.videoDetail';

  static bool get shadowVideoDetail =>
      // Rust 没起来时整个开关无意义——这一条同时就是降级路径：
      // 原生库没打进产物、init 失败，Dart 照常跑。
      RustCore.isReady &&
      (_box.get(shadowVideoDetailKey, defaultValue: kDebugMode) as bool);

  static Future<void> setShadowVideoDetail(bool value) =>
      _box.put(shadowVideoDetailKey, value);

  /// 视频详情**取数**走 Rust：请求由 Rust 发出、Rust 解析，结果进入 UI。
  ///
  /// 默认关闭，包括 debug。与 shadow mode 不同，这条路径一旦出问题是用户
  /// 可见的；而 Rust 侧的 HTTP 栈还没有基线 Dio 的重试、系统代理、证书
  /// 豁免与 HTTP/1.1 回退。先让它在开关后面跑够，再考虑默认值。
  ///
  /// 关闭、Rust 未就绪、取数失败——三种情况都走 Dart 原路径，见
  /// [RustVideoDetail.fetch]。
  static const String rustVideoDetailKey = 'rust.fetch.videoDetail';

  static bool get rustVideoDetail =>
      RustCore.isReady &&
      (_box.get(rustVideoDetailKey, defaultValue: false) as bool);

  static Future<void> setRustVideoDetail(bool value) =>
      _box.put(rustVideoDetailKey, value);
}
