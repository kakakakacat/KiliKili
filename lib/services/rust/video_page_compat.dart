import 'package:kilikili/models_new/video/video_play_info/subtitle.dart';
import 'package:kilikili/models_new/video/video_play_info/view_point.dart';
import 'package:kilikili/rust/api/video_page.dart' as rust_api;

/// 播放信息里两个**共享运行时模型**的单向适配（ADR-011 决策六）。
///
/// `playInfo` 的解析已经迁到 Rust，但 `Subtitle` 与 `ViewPoint` 不能跟着删：
///
/// - `Subtitle` 还有两个生产者——下载条目的本地缓存（`LocalSubtitle`、
///   `Subtitle.fromCache`）与 gRPC 的 `DmGrpc.dmViewSubtitles`；
/// - `ViewPoint` 会被序列化进下载条目的 JSON（`toJson` / `fromCache`），
///   并且带着 `toSegment` 这个画进度条的方法。
///
/// 等这两个生产者都迁完（下载相关见 S15 #66，dmView 的 gRPC 见 #49），
/// 这个文件连同那两个模型一起删。
///
/// 适配里只做**界面那一半**：AI 字幕的「（AI）」后缀与排序偏好。协议那一半
/// （字段映射、取值归一）已经在 Rust 做完了。
abstract final class VideoPageCompat {
  /// 把 Rust 的字幕转成运行时模型，并按基线的偏好排序。
  ///
  /// 基线在 `SubtitleInfo.fromJson` 里构造完就 `..sort()`：中文优先、
  /// AI 字幕靠后。那是界面偏好而不是协议，所以留在 Dart。
  static List<Subtitle> subtitles(List<rust_api.PlaySubtitle> list) =>
      list.map(_subtitle).toList()..sort();

  static Subtitle _subtitle(rust_api.PlaySubtitle s) {
    // 基线：type == 1 即 AI 生成，并在语言名后面补「（AI）」。
    final isAi = s.kind == 1;
    return Subtitle(
      id: s.id,
      lan: s.lan ?? '',
      lanDoc: '${s.lanDoc}${isAi ? '（AI）' : ''}',
      subtitleUrl: s.subtitleUrl,
      type: s.kind,
      isAi: isAi,
    );
  }

  /// 把 Rust 的分段看点转成运行时模型。
  static List<ViewPoint> viewPoints(List<rust_api.ViewPoint> list) =>
      list.map(_viewPoint).toList();

  static ViewPoint _viewPoint(rust_api.ViewPoint v) => ViewPoint(
    type: v.kind,
    from: v.from,
    to: v.to,
    content: v.content,
    imgUrl: v.imgUrl,
  );
}
