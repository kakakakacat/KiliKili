import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'probe_server.dart';

import 'package:canvas_danmaku/canvas_danmaku.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:media_kit/media_kit.dart';
import 'package:media_kit_video/media_kit_video.dart';
import 'package:kilikili/rust/frb_generated.dart';
import 'package:kilikili/rust/api/player_port.dart';
import 'package:kilikili/rust/api/danmaku.dart' as dm;
import 'package:kilikili/plugin/pl_player/player_port/flutter_media_kit_adapter.dart';
import 'package:kilikili/plugin/pl_player/player_port/media_kit_backend.dart';

// 与生产路径共用适配器、Rust 游标和真实脱敏 protobuf；此宿主只负责验收表面。
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  const library = String.fromEnvironment('BRIDGE_LIBRARY');
  try {
    await RustLib.init(
      externalLibrary: library.isNotEmpty
          ? ExternalLibrary.open(library)
          : (Platform.isMacOS || Platform.isIOS
                ? ExternalLibrary.process(iKnowHowToUseIt: true)
                : null),
    );
    MediaKit.ensureInitialized();
  } catch (error, stack) {
    runApp(MaterialApp(home: Scaffold(body: SelectableText('$error\n$stack'))));
    return;
  }
  runApp(const MaterialApp(home: Probe()));
}

class Probe extends StatefulWidget {
  const Probe({super.key});
  @override
  State<Probe> createState() => _ProbeState();
}

class _ProbeState extends State<Probe> with WidgetsBindingObserver {
  VideoController? video;
  NativePlayer? player;
  FlutterMediaKitAdapter? adapter;
  StreamSubscription<PlayerPortBatch>? subscription;
  DanmakuController<int>? danmaku;
  PlayerPortSnapshot? snapshot;
  var status = '初始化';
  var lastBucket = -1;
  var seenEpoch = 0;
  var batches = 0;
  var added = 0;
  var cleared = 0;
  var oldCallbacks = 0;
  var active = true;
  var running = false;
  int? dmSession;
  final records = <Map<String, Object?>>[];
  final clock = Stopwatch()..start();
  late Directory dir;
  Directory? outputDir;
  ControlledMediaServer? server;
  static const platform = MethodChannel('kilikili/player-port-probe');
  static const iosPip = MethodChannel('io.github.kakacat.kilikili/pip');
  bool iosPipUpdates = false;
  bool pip = false;
  String? pipError;
  bool seenBackground = false;
  bool resumedAfterBackground = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    if (Platform.isAndroid) {
      platform.setMethodCallHandler((call) async {
        if (call.method == 'pipChanged') {
          pip = call.arguments as bool;
          record(pip ? 'PIP_ENTERED' : 'PIP_EXITED');
        }
      });
    }
    if (Platform.isIOS) {
      iosPip.setMethodCallHandler((call) async {
        if (call.method == 'onStateChanged') {
          final args = call.arguments as Map;
          pip = args['active'] as bool;
          pipError = args['error'] as String?;
          record(pip ? 'PIP_ENTERED' : 'PIP_EXITED', {'error': pipError});
        } else if (call.method == 'setPlaying') {
          await adapter?.send(
            PlayerIntent(kind: call.arguments as bool ? .play : .pause),
          );
        } else if (call.method == 'skip') {
          await adapter?.send(
            PlayerIntent(
              kind: .seek,
              positionMs:
                  snapshot!.positionMs + BigInt.from(call.arguments as int),
            ),
          );
        }
      });
    }
    unawaited(start());
  }

  void record(String action, [Map<String, Object?> extra = const {}]) {
    final row = <String, Object?>{
      'action': action,
      'wall_ms': clock.elapsedMilliseconds,
      'position_ms': snapshot?.positionMs.toInt(),
      'state': snapshot?.state.name,
      'generation': adapter?.session.generation.toString(),
      'batches': batches,
      'rss': ProcessInfo.currentRss,
      'max_rss': ProcessInfo.maxRss,
      ...extra,
    };
    records.add(row);
    if (outputDir case final path?) {
      File('${path.path}/events.jsonl')
          .writeAsStringSync('${jsonEncode(row)}\n', mode: FileMode.append);
    }
    debugPrint('PLAYER_PROBE ${jsonEncode(row)}');
    if (mounted) setState(() => status = action);
  }

  Future<void> start() async {
    try {
      dir = await Directory.systemTemp.createTemp('kilikili-player-port-');
      outputDir = dir;
      for (final name in ['fixture.mp4', 'quality.mp4']) {
        final data = await rootBundle.load('assets/$name');
        await File('${dir.path}/$name').writeAsBytes(data.buffer.asUint8List());
      }
      dmSession = dm
          .danmakuOpen(
            options: const dm.DanmakuEngineOptions(
              isLogin: false,
              selfMidHash: '',
              merge: false,
              blockKeywords: [],
              blockRegexes: [],
              blockUids: [],
            ),
          )
          .sessionId!;
      final fixture = await rootBundle.load('assets/danmaku.pb');
      final pushed = await dm.danmakuPushSegment(
        sessionId: dmSession!,
        segmentIndex: 1,
        bytes: fixture.buffer.asUint8List(),
      );
      check(pushed.ok, '弹幕解码');
      await openPlayer();
      record('READY', {'fixture_items': pushed.total});
      if (const bool.fromEnvironment('AUTORUN')) await script();
    } catch (e, stack) {
      record('FAIL', {'error': '$e', 'stack': '$stack'});
    }
  }

  Future<void> openPlayer() async {
    active = true;
    final p = player = await NativePlayer.create(
      configuration: const PlayerConfiguration(
        options: {
          'hwdec': 'no',
          'ao': 'null',
          'cache': 'yes',
          'demuxer-max-bytes': '1048576',
          'cache-secs': '1',
          'cache-pause-wait': '0.2',
        },
      ),
    );
    final v = await VideoController.create(
      p,
      configuration: const VideoControllerConfiguration(
        enableHardwareAcceleration: false,
        hwdec: 'no',
      ),
    );
    final a = adapter = FlutterMediaKitAdapter(
      session: RustPlayerPortSession(),
      backend: MediaKitBackend(p),
    );
    subscription = a.events.listen((batch) {
      if (!active) {
        oldCallbacks++;
        return;
      }
      batches++;
      final s = batch.snapshot;
      if (s == null) return;
      snapshot = s;
      if (Platform.isIOS && iosPipUpdates) {
        unawaited(iosPip.invokeMethod('update', iosState));
      }
      final step = dm.danmakuProgressStep(
        lastBucketMs: lastBucket,
        seenEpoch: seenEpoch,
        positionMs: s.positionMs.toInt(),
        epoch: batch.generation.toInt(),
      );
      lastBucket = step.lastBucketMs;
      seenEpoch = step.seenEpoch;
      if (step.clear) {
        danmaku?.clear();
        cleared++;
      }
      if (step.emit) {
        for (final item
            in dm.danmakuBucket(
                  sessionId: dmSession!,
                  progressMs: step.bucketMs,
                ) ??
                <dm.DanmakuBucketItem>[]) {
          if (danmaku?.addDanmaku(
                DanmakuContentItem(
                  item.content,
                  color: Color(0xff000000 | item.color),
                  type: item.mode == 5
                      ? DanmakuItemType.top
                      : DanmakuItemType.scroll,
                  extra: step.seenEpoch,
                ),
              ) ??
              false) {
            added++;
          }
        }
      }
      danmaku?.updateOption(
        danmaku!.option.copyWith(
          duration: 10 / s.speed,
          staticDuration: 5 / s.speed,
        ),
      );
      if (s.state == .playing) {
        danmaku?.resume();
      } else {
        danmaku?.pause();
      }
      if (mounted) setState(() {});
    });
    if (mounted) setState(() => video = v);
    await a.send(PlayerIntent(kind: .open, video: '${dir.path}/fixture.mp4'));
  }

  Map<String, Object> get iosState => {
    'isPlaying': snapshot?.state == .playing,
    'isBuffering': snapshot?.state == .buffering,
    'isLive': false,
    'position': snapshot?.positionMs.toInt() ?? 0,
    'duration': snapshot?.durationMs?.toInt() ?? 0,
    'speed': snapshot?.speed ?? 1,
  };

  void check(bool ok, String message) {
    if (!ok) throw StateError(message);
  }

  Future<void> waitFor(bool Function(PlayerPortSnapshot s) accept) async {
    final end = DateTime.now().add(const Duration(seconds: 12));
    while (DateTime.now().isBefore(end)) {
      check(pipError == null, 'PiP: $pipError');
      if (snapshot case final s? when accept(s)) return;
      await Future<void>.delayed(const Duration(milliseconds: 40));
    }
    throw StateError('等待后端超时: $status');
  }

  Future<void> script() async {
    if (running || adapter == null) return;
    running = true;
    try {
      await adapter!.send(
        PlayerIntent(kind: .open, video: '${dir.path}/fixture.mp4'),
      );
      added = 0;
      cleared = 0;
      await adapter!.send(const PlayerIntent(kind: .play));
      await waitFor((s) => s.positionMs >= BigInt.from(2400));
      check(added > 0, '真实 fixture 弹幕未显示');
      record('PLAYING', {'danmaku_added': added});
      await adapter!.send(const PlayerIntent(kind: .pause));
      await waitFor((s) => s.state == .paused);
      final position = snapshot!.positionMs;
      record('PAUSED');
      await Future<void>.delayed(const Duration(milliseconds: 1400));
      check(
        snapshot!.positionMs == position && !(danmaku?.running ?? true),
        '暂停时钟/弹幕未冻结',
      );
      await adapter!.send(
        PlayerIntent(kind: .seek, positionMs: BigInt.from(10000)),
      );
      await waitFor((s) => (s.positionMs.toInt() - 10000).abs() < 150);
      check(cleared > 0, '跳转没有清屏');
      check(
        danmaku!.scrollDanmaku.every((row) => row.isEmpty) &&
            danmaku!.staticDanmaku.every((item) => item == null),
        '旧弹幕残留',
      );
      record('SEEK_FORWARD_CLEAR', {'clear_count': cleared});
      await adapter!.send(
        PlayerIntent(kind: .seek, positionMs: BigInt.from(1000)),
      );
      await adapter!.send(const PlayerIntent(kind: .setSpeed, speed: 2));
      await adapter!.send(const PlayerIntent(kind: .play));
      await waitFor((s) => s.speed == 2 && s.positionMs >= BigInt.from(2000));
      record('SEEK_BACK_2X');
      await adapter!.send(const PlayerIntent(kind: .setSpeed, speed: 0.5));
      await waitFor((s) => s.speed == 0.5);
      await Future<void>.delayed(const Duration(seconds: 2));
      record('SPEED_05X');
      final ticket = adapter!.session.prepare(
        PlayerIntent(kind: .open, video: '${dir.path}/quality.mp4'),
      );
      final start = Duration(milliseconds: snapshot!.positionMs.toInt());
      await adapter!.execute(ticket, start: start, play: true);
      await waitFor(
        (s) =>
            seenEpoch == ticket.generation.toInt() &&
            s.positionMs.toInt() >= start.inMilliseconds,
      );
      check(player!.getProperty('width') == '640', '分辨率未切换');
      record('QUALITY_640');
      server = await ControlledMediaServer.start(
        File('${dir.path}/quality.mp4'),
      );
      final network = adapter!.session.prepare(
        PlayerIntent(kind: .open, video: server!.url),
      );
      final networkStart = snapshot!.positionMs;
      await adapter!.execute(
        network,
        start: Duration(milliseconds: networkStart.toInt()),
        play: true,
      );
      await waitFor(
        (s) =>
            seenEpoch == network.generation.toInt() &&
            s.state == .playing &&
            s.positionMs > networkStart + BigInt.from(400),
      );
      server!.blocked = true;
      await waitFor((s) => s.state == .buffering);
      final frozen = snapshot!.positionMs;
      record('BUFFERING');
      await Future<void>.delayed(const Duration(seconds: 2));
      check(
        (snapshot!.positionMs - frozen).abs() < BigInt.from(100) &&
            !danmaku!.running,
        '缓冲时钟/弹幕仍在推进',
      );
      server!.blocked = false;
      await waitFor(
        (s) => s.state == .playing && s.positionMs > frozen + BigInt.from(200),
      );
      record('BUFFERING_PASS');
      if (const bool.fromEnvironment('LIFECYCLE_PROBE')) {
        seenBackground = false;
        resumedAfterBackground = false;
        record('WAIT_BACKGROUND');
        final deadline = DateTime.now().add(const Duration(seconds: 45));
        while (!resumedAfterBackground && DateTime.now().isBefore(deadline)) {
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
        check(resumedAfterBackground, '没有收到真实前后台往返');
        final beforeResume = snapshot!.positionMs;
        await waitFor(
          (s) =>
              s.state == .playing &&
              s.positionMs > beforeResume + BigInt.from(200),
        );
        check(video!.id.value != null, '恢复后纹理未就绪');
        record('LIFECYCLE_PASS');
      }
      if ((Platform.isAndroid || Platform.isIOS) &&
          const bool.fromEnvironment('PIP_PROBE')) {
        record('WAIT_PIP');
        if (Platform.isAndroid) {
          check(
            await platform.invokeMethod<bool>('enterPip') == true,
            '系统拒绝进入 PiP',
          );
        } else {
          check(
            await iosPip.invokeMethod<bool>('isAvailable') == true,
            '设备不支持 PiP',
          );
          iosPipUpdates = true;
          await iosPip.invokeMethod('enter', {
            'textureId': video!.id.value,
            'width': player!.state.width,
            'height': player!.state.height,
            'autoEnter': false,
            ...iosState,
          });
        }
        final before = snapshot!.positionMs;
        await waitFor(
          (s) =>
              pip &&
              s.state == .playing &&
              s.positionMs > before + BigInt.from(400),
        );
        record('PIP_PLAYING');
        final deadline = DateTime.now().add(const Duration(seconds: 45));
        while (pip && DateTime.now().isBefore(deadline)) {
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
        check(!pip, '没有退出 PiP');
        final after = snapshot!.positionMs;
        await waitFor(
          (s) => s.state == .playing && s.positionMs > after + BigInt.from(200),
        );
        record('PIP_PASS');
      }
      await Future<void>.delayed(const Duration(seconds: 2));
      await closePlayer();
      record('ROUTE_CLOSED');
      check(playerPortSessionCount() == BigInt.zero, '会话未释放');
      await Future<void>.delayed(const Duration(milliseconds: 400));
      snapshot = null;
      await openPlayer();
      await adapter!.send(const PlayerIntent(kind: .play));
      await waitFor((s) => s.positionMs >= BigInt.from(400));
      check(oldCallbacks == 0, '旧会话回写');
      record('PASS', {
        'old_callbacks': oldCallbacks,
        'danmaku_added': added,
        'clear_count': cleared,
      });
    } catch (e, stack) {
      record('FAIL', {'error': '$e', 'stack': '$stack'});
    } finally {
      running = false;
      await File('${dir.path}/result.json')
          .writeAsString(const JsonEncoder.withIndent('  ').convert(records));
      debugPrint('PLAYER_PROBE_RESULT ${dir.path}/result.json');
    }
  }

  Future<void> closePlayer() async {
    active = false;
    setState(() => video = null);
    await WidgetsBinding.instance.endOfFrame;
    await subscription?.cancel();
    if (Platform.isIOS) {
      iosPipUpdates = false;
      await iosPip.invokeMethod('dispose');
    }
    await adapter?.dispose();
    await server?.close();
    server = null;
    danmaku?.clear();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      seenBackground = true;
    }
    if (seenBackground && state == AppLifecycleState.resumed) {
      resumedAfterBackground = true;
    }
    record('LIFECYCLE_${state.name}');
    unawaited(
      adapter?.send(
        PlayerIntent(
          kind: .setLifecycle,
          foreground: state == AppLifecycleState.resumed,
        ),
      ),
    );
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    active = false;
    unawaited(subscription?.cancel());
    unawaited(adapter?.dispose());
    if (dmSession case final id?) dm.danmakuClose(sessionId: id);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: pip
        ? null
        : AppBar(
            title: Text(
              'Player Port · $status',
              style: const TextStyle(fontSize: 18),
            ),
          ),
    body: LayoutBuilder(
      builder: (_, box) => Stack(
        fit: StackFit.expand,
        children: [
          if (video case final v?)
            FittedBox(
              fit: BoxFit.contain,
              child: SimpleVideo(controller: v),
            ),
          DanmakuScreen<int>(
            createdController: (value) => danmaku = value,
            size: box.biggest,
            option: const DanmakuOption(fontSize: 25),
          ),
          // PiP 的可用高度不足以摆诊断面板；它不能挤掉真正要验的视频表面。
          if (!pip)
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: ColoredBox(
                color: Colors.white.withValues(alpha: 0.9),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      '${snapshot?.state.name} · ${snapshot?.positionMs} ms · ${snapshot?.speed}× · batches=$batches',
                      style: const TextStyle(fontSize: 12),
                      maxLines: 1,
                    ),
                    Wrap(
                      spacing: 8,
                      children: [
                        FilledButton(
                          onPressed: script,
                          child: const Text('运行验收'),
                        ),
                        TextButton(
                          onPressed: () =>
                              adapter?.send(const PlayerIntent(kind: .play)),
                          child: const Text('播放'),
                        ),
                        TextButton(
                          onPressed: () =>
                              adapter?.send(const PlayerIntent(kind: .pause)),
                          child: const Text('暂停'),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    ),
  );
}
