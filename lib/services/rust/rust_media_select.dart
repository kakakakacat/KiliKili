import 'package:kilikili/models/common/video/video_decode_type.dart';
import 'package:kilikili/models/video/play/url.dart';
import 'package:kilikili/rust/api/media_select.dart' as rust_api;
import 'package:kilikili/services/rust/rust_flags.dart';
import 'package:flutter/foundation.dart';
// FRB 自带的 Int64List（为 Web 的 BigInt 语义而设），与 dart:typed_data
// 的同名类不是一个类型。生成的签名用的是前者。
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;

/// 清晰度 / 编码 / 音质选择的 Rust 路径。
///
/// 全部是**同步**调用：算法是纯计算（几十条记录、没有 I/O），而基线
/// `VideoUtils.findTargetVideo` 也是同步的。返回 Future 会迫使起播与下载
/// 两条路径都改成异步。
///
/// 失败一律返回 null，由 [VideoUtils] 走原实现——包括基线会抛异常的那几
/// 种输入（编码认不出、`codecs` 缺席、accept_quality 给了 dash 里没有的
/// 清晰度）。Rust 不替基线「修」掉它们：修了就不是同一个行为，而这条
/// 路径的开关随时可以关掉，两边必须给出同一个结果。
abstract final class RustMediaSelect {
  /// 返回选中的「清晰度 + support_formats 下标 + 视频流下标 + 编码」。
  static ({
    int quality,
    int formatIndex,
    int videoIndex,
    VideoDecodeFormatType codec,
  })?
  selectVideo({
    required List<VideoItem> videoList,
    required int preferQuality,
    required List<int>? acceptQuality,
    required List<FormatItem> supportFormats,
    required List<VideoDecodeFormatType> preferCodecs,
  }) {
    if (!RustFlags.rustMediaSelect) return null;
    try {
      final result = rust_api.selectTargetVideo(
        videos: [
          for (final v in videoList)
            rust_api.VideoCandidateInput(id: v.id, codecs: v.codecs),
        ],
        preferQuality: preferQuality,
        // FRB 把 Vec<i64> 映射成 Int64List（省一次逐元素装箱）。
        acceptQuality: acceptQuality == null
            ? null
            : Int64List.fromList(acceptQuality),
        supportFormats: [
          for (final f in supportFormats)
            rust_api.FormatCandidateInput(
              quality: f.quality ?? 0,
              codecs: f.codecs,
            ),
        ],
        preferCodecIndices: [for (final c in preferCodecs) c.index],
      );

      final selection = result.selection;
      if (selection == null) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 清晰度选择失败，降级回 Dart: '
            '${result.errorKind} / ${result.errorDetail}',
          );
        }
        return null;
      }

      // 下标由 Rust 算出，越界只可能是两端对不上（比如传过去的列表与
      // 取回下标时用的不是同一个）。宁可降级，也不要越界崩溃。
      if (selection.videoIndex >= videoList.length ||
          selection.formatIndex >= supportFormats.length ||
          selection.codecIndex < 0 ||
          selection.codecIndex >= VideoDecodeFormatType.values.length) {
        if (kDebugMode) debugPrint('[rust] 清晰度选择返回了越界下标，降级回 Dart');
        return null;
      }

      return (
        quality: selection.quality,
        formatIndex: selection.formatIndex,
        videoIndex: selection.videoIndex,
        codec: VideoDecodeFormatType.values[selection.codecIndex],
      );
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 清晰度选择桥接失败，降级回 Dart: $e');
      return null;
    }
  }

  /// 返回选中音频流在 [audioList] 里的下标。
  static int? selectAudio({
    required List<AudioItem> audioList,
    required int preferQuality,
  }) {
    if (!RustFlags.rustMediaSelect) return null;
    try {
      final result = rust_api.selectTargetAudio(
        audioIds: Int64List.fromList([for (final a in audioList) a.id]),
        preferQuality: preferQuality,
      );
      final index = result.index;
      if (index == null) {
        if (kDebugMode) {
          debugPrint(
            '[rust] 音质选择失败，降级回 Dart: '
            '${result.errorKind} / ${result.errorDetail}',
          );
        }
        return null;
      }
      if (index >= audioList.length) {
        if (kDebugMode) debugPrint('[rust] 音质选择返回了越界下标，降级回 Dart');
        return null;
      }
      return index;
    } catch (e) {
      if (kDebugMode) debugPrint('[rust] 音质选择桥接失败，降级回 Dart: $e');
      return null;
    }
  }
}
