import 'package:kilikili/models/common/video/video_decode_type.dart';
import 'package:kilikili/rust/api/media_select.dart' as rust_api;
import 'package:kilikili/services/rust/rust_play_url.dart'
    show MediaStream, PlayFormat;
// FRB 自带的 Int64List（为 Web 的 BigInt 语义而设），与 dart:typed_data
// 的同名类不是一个类型。生成的签名用的是前者。
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;

/// 清晰度 / 编码 / 音质的选择（RFC-0001 §16.1）。**Rust 是唯一实现。**
///
/// ## 原来的 Dart 实现去哪了
///
/// 迁移期它藏在 `rust.select.media` 开关后面，`VideoUtils` 里还留着一份
/// Dart 原实现兜底。取流迁成 typed 直连时（issue #79）两者一并删除——
/// ADR-011 决策四，以及 RFC §2.3：选哪一档画质是业务规则，换掉宿主框架
/// 不会换掉它，留两份只会让两份慢慢长歪。
///
/// ## 失败会抛
///
/// 基线在这几种输入上本来就抛异常（编码认不出、`codecs` 缺席、
/// `accept_quality` 给了 dash 里没有的清晰度），Rust 把它们表达成
/// `Unsupported`。既然没有第二条路可降级，这里原样抛出去——交给起播
/// 路径既有的失败处理，而不是在这里猜一个「差不多的」画质。
abstract final class RustMediaSelect {
  /// 选中的「清晰度 + support_formats 下标 + 视频流下标 + 编码」。
  static ({
    int quality,
    int formatIndex,
    int videoIndex,
    VideoDecodeFormatType codec,
  })
  selectVideo({
    required List<MediaStream> videoList,
    required int preferQuality,
    required List<int>? acceptQuality,
    required List<PlayFormat> supportFormats,
    required List<VideoDecodeFormatType> preferCodecs,
  }) {
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
      throw StateError(
        '清晰度选择失败: ${result.errorKind} / ${result.errorDetail}',
      );
    }

    // 下标由 Rust 算出，越界只可能是两端对不上（传过去的列表与取回下标
    // 时用的不是同一个）。宁可报错，也不要越界崩在别处。
    if (selection.videoIndex >= videoList.length ||
        selection.formatIndex >= supportFormats.length ||
        selection.codecIndex < 0 ||
        selection.codecIndex >= VideoDecodeFormatType.values.length) {
      throw StateError('清晰度选择返回了越界下标');
    }

    return (
      quality: selection.quality,
      formatIndex: selection.formatIndex,
      videoIndex: selection.videoIndex,
      codec: VideoDecodeFormatType.values[selection.codecIndex],
    );
  }

  /// 选中音频流在 [audioList] 里的下标。
  static int selectAudio({
    required List<MediaStream> audioList,
    required int preferQuality,
  }) {
    final result = rust_api.selectTargetAudio(
      audioIds: Int64List.fromList([for (final a in audioList) a.id]),
      preferQuality: preferQuality,
    );
    final index = result.index;
    if (index == null) {
      throw StateError('音质选择失败: ${result.errorKind} / ${result.errorDetail}');
    }
    if (index >= audioList.length) {
      throw StateError('音质选择返回了越界下标');
    }
    return index;
  }
}
