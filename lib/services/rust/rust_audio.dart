// FRB 的 Int64List 元素用 BigInt 表示，不能用 dart:typed_data 的同名类。
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/audio.dart' as api;
import 'package:kilikili/services/rust/rust_grpc.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/video_utils.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/audio.dart'
    show
        AudioItem,
        AudioPart,
        AudioPlayback,
        AudioPlaylist,
        AudioPlaylistSource,
        AudioListOrder;

/// 音频 gRPC 调用面；账号与播放器设置仍由宿主注入。
abstract final class RustAudio {
  static const _notReady = 'Rust Core 未就绪，音频暂不可用';

  static LoadingState<T> _unwrap<T>(T? data, String? detail, int? code) =>
      data == null ? Error(detail ?? '请求失败', code: code) : Success(data);

  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what 出错: $e\n$st');
    return Error('$e');
  }

  static Future<LoadingState<api.AudioPlaylist>> playlist({
    required int id,
    int? anchorOid,
    List<int> anchorSubIds = const [],
    int? anchorItemType,
    api.AudioPlaylistSource? source,
    int? extraId,
    String? next,
    api.AudioListOrder order = api.AudioListOrder.orderNormal,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await api.fetchAudioPlaylist(
        context: RustGrpc.context(),
        query: api.AudioPlaylistQuery(
          id: id,
          anchorOid: anchorOid,
          anchorSubIds: Int64List.fromList(anchorSubIds),
          anchorItemType: anchorItemType,
          source: source,
          extraId: extraId,
          next: next,
          order: order,
        ),
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('音频列表', e, st);
    }
  }

  static Future<LoadingState<api.AudioPlayback>> playUrl({
    required int oid,
    required List<int> subIds,
    required int itemType,
    required int preferredQuality,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await api.fetchAudioPlayUrl(
        context: RustGrpc.context(),
        oid: oid,
        subIds: Int64List.fromList(subIds),
        itemType: itemType,
        preferredQuality: preferredQuality,
        cdnService: VideoUtils.cdnService.name,
        disableAudioCdn: VideoUtils.disableAudioCDN,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('音频取流', e, st);
    }
  }

  static Future<LoadingState<api.AudioWrite>> thumbUp({
    required int oid,
    required List<int> subIds,
    required int itemType,
    required bool like,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await api.audioThumbUp(
        context: RustGrpc.context(),
        oid: oid,
        subIds: Int64List.fromList(subIds),
        itemType: itemType,
        like: like,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('音频点赞', e, st);
    }
  }

  static Future<LoadingState<api.AudioWrite>> tripleLike({
    required int oid,
    required List<int> subIds,
    required int itemType,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await api.audioTripleLike(
        context: RustGrpc.context(),
        oid: oid,
        subIds: Int64List.fromList(subIds),
        itemType: itemType,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('音频三连', e, st);
    }
  }

  static Future<LoadingState<api.AudioWrite>> coinAdd({
    required int oid,
    required List<int> subIds,
    required int itemType,
    required int num,
    required bool thumbUp,
  }) async {
    if (!RustCore.isReady) return const Error(_notReady);
    try {
      final res = await api.audioCoinAdd(
        context: RustGrpc.context(),
        oid: oid,
        subIds: Int64List.fromList(subIds),
        itemType: itemType,
        num: num,
        thumbUp: thumbUp,
      );
      return _unwrap(res.data, res.errorDetail, res.errorCode);
    } catch (e, st) {
      return _crashed('音频投币', e, st);
    }
  }
}
