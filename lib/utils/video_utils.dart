import 'package:kilikili/models/common/video/base_quality.dart';
import 'package:kilikili/models/common/video/cdn_type.dart';
import 'package:kilikili/models/common/video/video_decode_type.dart';
import 'package:kilikili/rust/api/playback.dart' as rust_api;
import 'package:kilikili/services/rust/rust_play_url.dart'
    show MediaStream, PlayFormat;
import 'package:kilikili/services/rust/rust_live.dart' show LiveCodec;
import 'package:kilikili/services/rust/rust_media_select.dart';
import 'package:kilikili/utils/extension/iterable_ext.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:flutter/foundation.dart' show kDebugMode, debugPrint;

abstract final class VideoUtils {
  static CDNService cdnService = Pref.defaultCDNService;
  static String? liveCdnUrl = Pref.liveCdnUrl;
  static bool disableAudioCDN = Pref.disableAudioCDN;

  /// 从候选地址里挑一个真正要播的。
  ///
  /// 规则在 Rust（`bili-core::cdn`）：镜像站改 host、`os=mcdn` 走 tf
  /// 代理、`szbdyd.com` 换回源站、Akamai 保留自带签名——按 RFC §2.3，
  /// 这些换掉宿主框架一个字都不会变，所以不留在这边。
  ///
  /// [defaultCDNService] 不传时用当前设置。[isAudio] 配合
  /// `disableAudioCDN`：音频换 CDN 的收益小、踩签名问题的代价大。
  static String getCdnUrl(
    Iterable<String> urls, {
    CDNService? defaultCDNService,
    bool isAudio = false,
  }) => rust_api.pickCdnUrl(
    urls: urls is List<String> ? urls : urls.toList(),
    // 枚举名即 Rust 侧的 key，两边同一张表（`cdnServices()`）。
    service: (defaultCDNService ?? cdnService).name,
    isAudio: isAudio,
    disableAudioCdn: disableAudioCDN,
  );

  static String getLiveCdnUrl(LiveCodec e, {int index = 0}) {
    final urlInfo = e.urlInfo.getOrFirst(index);
    return (liveCdnUrl ?? urlInfo.host) + e.baseUrl + urlInfo.extra;
  }

  static VideoDecodeFormatType selectCodec(
    Iterable<String> codecs,
    List<VideoDecodeFormatType> preferCodecs,
  ) {
    if (preferCodecs.isNotEmpty) {
      int bestIndex = preferCodecs.length;
      for (final e in codecs) {
        for (int i = 0; i < bestIndex; i++) {
          if (preferCodecs[i].codes.any(e.startsWith)) {
            bestIndex = i;
            if (bestIndex == 0) {
              return preferCodecs[0];
            }
            break;
          }
        }
      }
      if (bestIndex < preferCodecs.length) {
        return preferCodecs[bestIndex];
      }
    }
    return VideoDecodeFormatType.fromString(codecs.first);
  }

  static (
    int targetQuality,
    PlayFormat targetFormat,
    VideoDecodeFormatType targetDecode,
    MediaStream targetVideo,
  )
  findTargetVideo(
    List<MediaStream> videoList,
    int preferQuality,
    List<int>? acceptQuality,
    List<PlayFormat> supportFormats,
    List<VideoDecodeFormatType> preferCodecs,
  ) {
    final selection = RustMediaSelect.selectVideo(
      videoList: videoList,
      preferQuality: preferQuality,
      acceptQuality: acceptQuality,
      supportFormats: supportFormats,
      preferCodecs: preferCodecs,
    );
    final video = videoList[selection.videoIndex];
    return (
      video.id,
      supportFormats[selection.formatIndex],
      selection.codec,
      video,
    );
  }

  static MediaStream findTargetAudio(
    List<MediaStream> audioList,
    int preferQuality,
  ) =>
      audioList[RustMediaSelect.selectAudio(
        audioList: audioList,
        preferQuality: preferQuality,
      )];
}
