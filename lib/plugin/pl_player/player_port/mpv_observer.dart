import 'dart:ffi';

import 'package:media_kit/ffi/ffi.dart';
import 'package:media_kit/generated/libmpv/bindings.dart' as mpv;
import 'package:media_kit/media_kit.dart';
import 'package:kilikili/rust/api/player_port.dart';

/// 独立 weak client：不能从 media-kit 的 handle 上 wait_event 抢走其命令回复。
class MpvObserver {
  MpvObserver(NativePlayer player) {
    final name = 'kilikili-player-port'.toNativeUtf8();
    try {
      _handle = _api.mpv_create_weak_client(player.ctx, name);
    } finally {
      calloc.free(name);
    }
    if (_handle == nullptr) throw StateError('无法创建 Player Port 观察端');
    final level = 'error'.toNativeUtf8();
    try {
      _api.mpv_request_log_messages(_handle, level);
    } finally {
      calloc.free(level);
    }
  }

  static final _api = NativePlayer.mpv;
  Pointer<mpv.mpv_handle> _handle = nullptr;
  int _remaining = 0;
  BigInt _generation = BigInt.zero;
  final _sample = <PlayerObservation>[];
  static const _properties = <(String, int, PlayerObservationKind)>[
    ('time-pos', mpv.mpv_format.MPV_FORMAT_DOUBLE, .position),
    ('duration', mpv.mpv_format.MPV_FORMAT_DOUBLE, .duration),
    ('pause', mpv.mpv_format.MPV_FORMAT_FLAG, .paused),
    ('paused-for-cache', mpv.mpv_format.MPV_FORMAT_FLAG, .buffering),
    ('eof-reached', mpv.mpv_format.MPV_FORMAT_FLAG, .ended),
    ('speed', mpv.mpv_format.MPV_FORMAT_DOUBLE, .speed),
    ('current-tracks/video/id', mpv.mpv_format.MPV_FORMAT_INT64, .videoTrack),
    ('current-tracks/audio/id', mpv.mpv_format.MPV_FORMAT_INT64, .audioTrack),
    ('demuxer-cache-time', mpv.mpv_format.MPV_FORMAT_DOUBLE, .buffer),
  ];

  /// 异步读取属性，回复用发起时的代际盖章；不把 timer 当视频时钟。
  void sample(BigInt generation) {
    if (_handle == nullptr || _remaining != 0) return;
    _generation = generation;
    _sample.clear();
    _remaining = _properties.length;
    for (var i = 0; i < _properties.length; i++) {
      final property = _properties[i];
      final name = property.$1.toNativeUtf8();
      try {
        final result = _api.mpv_get_property_async(
          _handle,
          i + 1,
          name,
          property.$2,
        );
        if (result < 0) _remaining--;
      } finally {
        calloc.free(name);
      }
    }
  }

  List<PlayerObservation> drain() {
    final result = <PlayerObservation>[];
    if (_handle == nullptr) return result;
    for (var count = 0; count < 512; count++) {
      final event = _api.mpv_wait_event(_handle, 0).ref;
      if (event.event_id == mpv.mpv_event_id.MPV_EVENT_NONE) return result;
      switch (event.event_id) {
        case mpv.mpv_event_id.MPV_EVENT_START_FILE:
          result.add(const PlayerObservation(kind: .startFile, value: 0));
        case mpv.mpv_event_id.MPV_EVENT_FILE_LOADED:
          result.add(const PlayerObservation(kind: .fileLoaded, value: 0));
        case mpv.mpv_event_id.MPV_EVENT_SEEK:
          result.add(const PlayerObservation(kind: .seek, value: 0));
        case mpv.mpv_event_id.MPV_EVENT_PLAYBACK_RESTART:
          result.add(const PlayerObservation(kind: .playbackRestart, value: 0));
        case mpv.mpv_event_id.MPV_EVENT_END_FILE:
          if (event.data != nullptr) {
            final end = event.data.cast<mpv.mpv_event_end_file>().ref;
            result.add(
              PlayerObservation(kind: .endFile, value: end.error.toDouble()),
            );
          }
        case mpv.mpv_event_id.MPV_EVENT_QUEUE_OVERFLOW:
        case mpv.mpv_event_id.MPV_EVENT_SHUTDOWN:
          result.add(const PlayerObservation(kind: .overflow, value: 0));
        case mpv.mpv_event_id.MPV_EVENT_LOG_MESSAGE:
          if (event.data != nullptr) {
            final log = event.data.cast<mpv.mpv_event_log_message>().ref;
            result.add(
              PlayerObservation(
                kind: .error,
                value: 0,
                text: log.text.toDartString().trim(),
              ),
            );
          }
        case mpv.mpv_event_id.MPV_EVENT_GET_PROPERTY_REPLY:
          final index = event.reply_userdata - 1;
          if (index < 0 || index >= _properties.length || _remaining == 0) {
            continue;
          }
          final kind = _properties[index].$3;
          if (event.error >= 0 && event.data != nullptr) {
            final property = event.data.cast<mpv.mpv_event_property>().ref;
            if (property.data != nullptr) {
              final value = switch (property.format) {
                mpv.mpv_format.MPV_FORMAT_DOUBLE =>
                  property.data.cast<Double>().value,
                mpv.mpv_format.MPV_FORMAT_FLAG =>
                  property.data.cast<Int32>().value.toDouble(),
                mpv.mpv_format.MPV_FORMAT_INT64 =>
                  property.data.cast<Int64>().value.toDouble(),
                _ => double.nan,
              };
              _sample.add(
                PlayerObservation(
                  kind: kind,
                  value: value,
                  generation: _generation,
                ),
              );
            }
          } else if (kind == .videoTrack ||
              kind == .audioTrack ||
              kind == .duration ||
              kind == .buffer) {
            _sample.add(
              PlayerObservation(kind: kind, value: -1, generation: _generation),
            );
          }
          _remaining--;
          if (_remaining == 0) result.addAll(_sample);
      }
    }
    // 限制一次占用 UI 的工作；不能在丢过确认事件后继续猜代际。
    result.add(const PlayerObservation(kind: .overflow, value: 0));
    return result;
  }

  void dispose() {
    final handle = _handle;
    _handle = nullptr;
    if (handle != nullptr) _api.mpv_destroy(handle);
  }
}
