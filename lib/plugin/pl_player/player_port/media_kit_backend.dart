import 'package:media_kit/media_kit.dart';
import 'package:kilikili/rust/api/player_port.dart';

import 'package:kilikili/plugin/pl_player/player_port/flutter_media_kit_adapter.dart';
import 'package:kilikili/plugin/pl_player/player_port/mpv_observer.dart';

class MediaKitBackend implements PlayerPortBackend {
  MediaKitBackend(this.player) : _observer = MpvObserver(player);
  final NativePlayer player;
  final MpvObserver _observer;
  @override
  List<PlayerObservation> drain() => _observer.drain();
  @override
  void sample(BigInt generation) => _observer.sample(generation);
  @override
  void detachObserver() => _observer.dispose();
  @override
  Future<void> dispose() => player.dispose();

  @override
  Future<void> execute(
    PlayerCommandTicket ticket, {
    Duration? start,
    Map<String, String>? extras,
    bool play = false,
  }) async {
    final intent = ticket.intent;
    switch (intent.kind) {
      case PlayerIntentKind.open:
        await player.command(['set', 'user-agent', intent.userAgent ?? '']);
        await player.command(['set', 'referrer', intent.referer ?? '']);
        await player.open(
          Media(ticket.mediaUri!, start: start, extras: extras),
          play: play,
        );
      case PlayerIntentKind.play:
        await player.play();
      case PlayerIntentKind.pause:
        await player.pause();
      case PlayerIntentKind.seek:
        await player.seek(Duration(milliseconds: intent.positionMs!.toInt()));
      case PlayerIntentKind.setSpeed:
        await player.setRate(intent.speed!);
      case PlayerIntentKind.selectTracks:
        if (intent.videoTrack case final video?) {
          await player.setVideoTrack(VideoTrack(video.toString(), null, null));
        }
        if (intent.audioTrack case final audio?) {
          await player.setAudioTrack(AudioTrack(audio.toString(), null, null));
        }
      case PlayerIntentKind.attachSurface:
      case PlayerIntentKind.detachSurface:
      case PlayerIntentKind.setLifecycle:
        // Flutter 的纹理/PiP 生命周期由 VideoController 和平台插件实现，
        // 不套用 Slint 的 vid=no 策略，避免与 media-kit 的表面恢复互相覆盖。
        break;
    }
  }
}
