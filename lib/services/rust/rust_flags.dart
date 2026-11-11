import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/storage.dart';
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
      (_box.get(
        rustVideoDetailKey,
        defaultValue: _defaultOn(rustVideoDetailKey),
      ) as bool);

  static Future<void> setRustVideoDetail(bool value) =>
      _box.put(rustVideoDetailKey, value);

  /// 取流走 Rust：请求由 Rust 发出、Rust 解析，结果进入播放器。
  ///
  /// 默认关闭，理由同 [rustVideoDetail]，且取流更敏感——详情取不到是
  /// 空页面，流取不到是播放器一直转圈。另有几条基线特有的重试分支
  /// （DRM 降级、UGC→PGC），任何一条没对齐都会表现为「某些番剧播不了」。
  ///
  /// 关闭、Rust 未就绪、取流失败——三种情况都走 Dart 原路径，见
  /// [RustPlayUrl.fetch]。
  static const String rustPlayUrlKey = 'rust.fetch.playUrl';

  static bool get rustPlayUrl =>
      RustCore.isReady &&
      (_box.get(rustPlayUrlKey, defaultValue: _defaultOn(rustPlayUrlKey))
          as bool);

  static Future<void> setRustPlayUrl(bool value) =>
      _box.put(rustPlayUrlKey, value);

  /// 取流 shadow mode：Dart 与 Rust 对**同一份响应**各自规范化后比对，
  /// 播放仍然用 Dart 的结果。debug 默认开启，理由同 [shadowVideoDetail]。
  static const String shadowPlayUrlKey = 'rust.shadow.playUrl';

  static bool get shadowPlayUrl =>
      RustCore.isReady &&
      (_box.get(shadowPlayUrlKey, defaultValue: kDebugMode) as bool);

  static Future<void> setShadowPlayUrl(bool value) =>
      _box.put(shadowPlayUrlKey, value);

  /// 搜索建议响应使用 Rust 解析。请求和 WBI 签名仍由现有 Dart 路径处理。
  /// 默认关闭；解析失败会回退到 `SearchSuggestModel.fromJson`。
  static const String rustSearchSuggestKey = 'rust.parse.searchSuggest';

  static bool get rustSearchSuggest =>
      RustCore.isReady &&
      (_box.get(
        rustSearchSuggestKey,
        defaultValue: _defaultOn(rustSearchSuggestKey),
      ) as bool);

  static Future<void> setRustSearchSuggest(bool value) =>
      _box.put(rustSearchSuggestKey, value);

  /// `DmView` 的字幕轨由 Rust 解析。请求仍由现有 Dart 路径发出。
  ///
  /// 默认关闭；解析失败会回退到 `DmGrpc.dmViewToSubtitle`。与弹幕引擎
  /// 不同，这条路径无状态、一次调用结束，失败只影响本次取字幕。
  static const String rustDmViewSubtitleKey = 'rust.parse.dmViewSubtitle';

  static bool get rustDmViewSubtitle =>
      RustCore.isReady &&
      (_box.get(
            rustDmViewSubtitleKey,
            defaultValue: _defaultOn(rustDmViewSubtitleKey),
          )
          as bool);

  static Future<void> setRustDmViewSubtitle(bool value) =>
      _box.put(rustDmViewSubtitleKey, value);

  /// 弹幕引擎走 Rust：分段解码、标记自己、合并、过滤、分桶都在 Rust，
  /// Dart 只按播放进度取**一桶**（100ms）。
  ///
  /// 默认关闭。与解析类开关不同，这条路径持有状态（一个视频的全部弹幕
  /// 都在 Rust 侧的会话里），出问题的表现是「这段没人发弹幕」——没有
  /// 任何报错，只有空白。因此任何一步失败都整条降级回 Dart，见
  /// [RustDanmakuSession] 与 `PlDanmakuController`。
  ///
  /// 还有一处**不保证等价**：用户的正则屏蔽规则。Dart `RegExp` 与 Rust
  /// `regex` 的语法和语义不是一个集合，编不过或语义不同的规则会让开会话
  /// 直接失败，于是整条回到 Dart（ADR-010）。
  static const String rustDanmakuEngineKey = 'rust.engine.danmaku';

  static bool get rustDanmakuEngine =>
      RustCore.isReady &&
      (_box.get(
            rustDanmakuEngineKey,
            defaultValue: _defaultOn(rustDanmakuEngineKey),
          )
          as bool);

  static Future<void> setRustDanmakuEngine(bool value) =>
      _box.put(rustDanmakuEngineKey, value);

  /// 构建期指定的初始值，用于真机验证。
  ///
  /// 取数开关没有设置项入口（见类文档：不往上游的设置页里加东西），
  /// 而验证一条取数路径必须能把它打开。办法是构建时指定：
  ///
  /// ```sh
  /// flutter build apk --debug --dart-define=rust.fetch.playUrl=true
  /// ```
  ///
  /// 只影响**初始值**：用户或代码此后写过的值仍然优先。不给 define 时
  /// 恒为 false，因此 release 产物不受影响。
  static bool _defaultOn(String key) => switch (key) {
    rustVideoDetailKey => const bool.fromEnvironment('rust.fetch.videoDetail'),
    rustPlayUrlKey => const bool.fromEnvironment('rust.fetch.playUrl'),
    rustSearchSuggestKey => const bool.fromEnvironment(
      'rust.parse.searchSuggest',
    ),
    rustDanmakuEngineKey => const bool.fromEnvironment('rust.engine.danmaku'),
    rustDmViewSubtitleKey => const bool.fromEnvironment(
      'rust.parse.dmViewSubtitle',
    ),
    _ => false,
  };
}
