import 'dart:async' show Timer, StreamSubscription;
import 'dart:io' show Platform;
import 'dart:math' as math;

import 'package:kilikili/common/widgets/dialog/report.dart';
import 'package:kilikili/common/widgets/flutter/text_field/controller.dart';
import 'package:kilikili/http/live.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/super_chat_type.dart';
import 'package:kilikili/models/common/video/base_quality.dart';
import 'package:kilikili/services/rust/live_playurl_compat.dart';
import 'package:kilikili/pages/live_room/superchat/super_chat_state.dart';
import 'package:kilikili/pages/common/publish/publish_route.dart';
import 'package:kilikili/pages/danmaku/danmaku_model.dart';
import 'package:kilikili/pages/live_room/send_danmaku/view.dart';
import 'package:kilikili/pages/video/widgets/header_control.dart';
import 'package:kilikili/plugin/pl_player/controller.dart';
import 'package:kilikili/plugin/pl_player/models/data_source.dart';
import 'package:kilikili/plugin/pl_player/utils/danmaku_options.dart';
import 'package:kilikili/services/rust/live_message_session.dart';
import 'package:kilikili/services/rust/rust_live_danmaku.dart';
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:kilikili/services/rust/rust_video_write.dart';
import 'package:kilikili/services/service_locator.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/android/bindings.g.dart';
import 'package:kilikili/utils/connectivity_utils.dart';
import 'package:kilikili/utils/danmaku_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/extension/iterable_ext.dart';
import 'package:kilikili/utils/extension/rx_ext.dart';
import 'package:kilikili/utils/num_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_key.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/utils/theme_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:kilikili/utils/video_utils.dart';
import 'package:canvas_danmaku/canvas_danmaku.dart';
import 'package:easy_debounce/easy_throttle.dart';
import 'package:flutter/foundation.dart'
    show debugPrint, kDebugMode, kReleaseMode;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

const int _kMaxChatCount = 500;
const int _kTrimCount = _kMaxChatCount + 50;
const int _kSafeTrimIndex = 200;

class LiveRoomController extends GetxController {
  LiveRoomController(this.heroTag);
  final String heroTag;

  int roomId = Get.arguments;
  int? ruid;
  DanmakuController<DanmakuExtra>? danmakuController;
  final plPlayerController = PlPlayerController.getInstance(
    isLive: true,
  );

  final isLoaded = false.obs;
  final roomInfoH5 = Rxn<LiveRoomH5>();

  final liveTime = Rxn<int>();
  Timer? liveTimeTimer;

  void startLiveTimer() {
    if (liveTime.value != null) {
      liveTimeTimer ??= Timer.periodic(
        const Duration(minutes: 5),
        (_) => liveTime.refresh(),
      );
    }
  }

  void cancelLiveTimer() {
    liveTimeTimer?.cancel();
    liveTimeTimer = null;
  }

  Widget get timeWidget => Obx(() {
    final liveTime = this.liveTime.value;
    String text = '';
    if (liveTime != null) {
      final duration = DurationUtils.formatDurationBetween(
        liveTime * 1000,
        DateTime.now().millisecondsSinceEpoch,
      );
      text += duration.isEmpty ? '刚刚开播' : '开播$duration';
    }
    if (text.isEmpty) {
      return const SizedBox.shrink();
    }
    return Text(
      text,
      style: const TextStyle(
        fontSize: 12,
        color: Colors.white,
      ),
    );
  });

  // dm
  List<RichTextItem>? savedDanmaku;
  int builtLength = 0;
  final messages = <dynamic>[].obs;
  bool get shouldRefresh => builtLength != messages.length;
  late final RxList<LiveSuperChatState> superChatMsg =
      <LiveSuperChatState>[].obs;
  final disableAutoScroll = false.obs;
  bool autoScroll = true;
  LiveMessageSession? _msgStream;

  late final fsSC = Rxn<LiveSuperChatState>();
  late final enableFSSC = Pref.enableFullScreenSC;
  late final fsSCScale = Pref.fullScreenSCScale;
  late final fsSCOpacity = Pref.fullScreenSCOpacity;

  List<String> _keywordList = const [];
  Set<int> _shieldUids = const {};

  late final ScrollController scrollController;
  late final RxInt pageIndex = 0.obs;
  PageController? pageController;

  final RxBool isPortrait = false.obs;
  late List<LiveQuality> acceptQnList = [];
  final Rxn<LiveQuality> currentQn = Rxn<LiveQuality>(
    PlatformUtils.isMobile || Platform.isLinux
        ? null
        : .fromCode(Pref.liveQuality),
  );

  late final bool isLogin;
  late final int mid;

  String? videoUrl;
  bool? isPlaying;
  late bool isFullScreen = false;

  final superChatType = Pref.superChatType;
  late final showSuperChat = superChatType != SuperChatType.disable;

  final headerKey = GlobalKey<TimeBatteryMixin>();

  final RxString title = ''.obs;

  final RxnString onlineCount = RxnString();

  final RxnString watchedShow = RxnString();
  Widget get watchedWidget => Obx(() {
    if (watchedShow.value case final watchedShow?) {
      return Text(
        watchedShow,
        style: const TextStyle(
          fontSize: 12,
          color: Colors.white,
        ),
      );
    }
    return const SizedBox.shrink();
  });

  int chatSimpleIndex = 0;
  int _trimDmIndex = 0;
  int get trimDmIndex => _trimDmIndex;
  void _trimDm() {
    final trimCount = messages.length - _trimDmIndex;
    if (trimCount > _kTrimCount) {
      final endIndex = messages.length - _kMaxChatCount;
      final canTrim = (chatSimpleIndex - endIndex) > _kSafeTrimIndex;
      if (canTrim) {
        messages.fillRangeOnly(_trimDmIndex, endIndex);
        _trimDmIndex = endIndex;
      }
    }
  }

  StreamSubscription? _sizeSub;

  void _onSizeChanged((int, int) value) {
    final isVertical = value.$2 > value.$1;
    isPortrait.value = isVertical;
    plPlayerController.isVertical = isVertical;
  }

  void _startSizeSub() {
    if (isPortrait.value) return;
    _stopSizeSub();
    _sizeSub = plPlayerController.videoPlayerController?.stream.size.listen(
      _onSizeChanged,
    );
  }

  void _stopSizeSub() {
    _sizeSub?.cancel();
    _sizeSub = null;
  }

  @override
  void onInit() {
    super.onInit();
    scrollController = ScrollController()..addListener(listener);
    final account = Accounts.main;
    isLogin = account.isLogin;
    mid = account.mid;
    queryLiveUrl(autoFullScreenFlag: true);
    queryLiveInfoH5();
    if (Accounts.heartbeat.isLogin && !Pref.historyPause) {
      RustVideoWrite.roomEntryAction(roomId: roomId);
    }
    if (showSuperChat) {
      pageController = PageController();
    }
  }

  Future<void>? playerInit({
    bool autoplay = true,
    bool autoFullScreenFlag = false,
  }) {
    if (videoUrl == null) {
      return null;
    }
    return plPlayerController.setDataSource(
      NetworkSource(videoSource: videoUrl!, audioSource: null),
      isLive: true,
      autoplay: autoplay,
      isVertical: isPortrait.value,
      autoFullScreenFlag: autoFullScreenFlag,
    );
  }

  Future<void> queryLiveUrl({bool autoFullScreenFlag = false}) async {
    currentQn.value ??= .fromCode(
      await ConnectivityUtils.isWiFi
          ? Pref.liveQuality
          : Pref.liveQualityCellular,
    );
    final res = await RustLive.roomPlayInfo(
      roomId: roomId,
      qn: currentQn.value!.code,
      onlyAudio: plPlayerController.onlyPlayAudio.value,
    );
    if (res case Success(:final response)) {
      if (response.liveStatus != 1) {
        _showDialog('当前直播间未开播');
        return;
      }
      // 基线判的是 `playurlInfo?.playurl == null`。Rust 侧把那两层空壳
      // 摊平了，于是「没有 playurl」与「playurl.stream 是空数组」在这里
      // 都表现为空列表——对调用方本就是同一件事。基线在后一种情况下会
      // 崩在 `stream.getOrFirst(0)` 上，这里一并挡掉。
      if (response.streams.isEmpty) {
        _showDialog('无法获取播放地址');
        return;
      }
      ruid = response.uid;
      if (response.roomId case final roomId?) {
        this.roomId = roomId;
      }
      liveTime.value = response.liveTime;
      startLiveTimer();
      isPortrait.value = response.isPortrait ?? false;
      stream = response.streams;
      _initStreamIndex();
      await Future.wait([
        ?initLiveUrl(
          streamIndex: streamIndex,
          formatIndex: formatIndex,
          codecIndex: codecIndex,
          liveUrlIndex: liveUrlIndex,
        ),
        if (!isLoaded.value && Accounts.heartbeat.isLogin) _fetchBlockRules(),
      ]);
      isLoaded.value = true;
    } else {
      _showDialog(res.toString());
    }
  }

  late List<LiveStream> stream;
  int streamIndex = 0;
  int formatIndex = 0;
  int codecIndex = 0;
  int liveUrlIndex = 0;

  void _initStreamIndex() {
    final pref = Pref.liveStream;
    if (pref != null) {
      try {
        final String protocolName = pref[0];
        final String formatName = pref[1];
        final String codecName = pref[2];
        for (var (i, s) in stream.indexed) {
          if (s.protocolName == protocolName) {
            streamIndex = i;
            for (var (j, f) in s.format.indexed) {
              if (f.formatName == formatName) {
                formatIndex = j;
                for (var (k, c) in f.codec.indexed) {
                  if (c.codecName == codecName) {
                    codecIndex = k;
                    return;
                  }
                }
              }
            }
          }
        }
      } catch (_) {}
    }
  }

  Future<void>? initLiveUrl({
    int streamIndex = 0,
    int formatIndex = 0,
    int codecIndex = 0,
    int liveUrlIndex = 0,
  }) {
    this.streamIndex = streamIndex;
    this.formatIndex = formatIndex;
    this.codecIndex = codecIndex;
    this.liveUrlIndex = liveUrlIndex;

    final LiveCodec item = stream
        .getOrFirst(streamIndex)
        .format
        .getOrFirst(formatIndex)
        .codec
        .getOrFirst(codecIndex);
    acceptQnList = toLiveQualityList(item.acceptQn);
    // 以服务端返回的码率为准
    currentQn.value = toLiveQuality(item.currentQn);
    videoUrl = VideoUtils.getLiveCdnUrl(item, index: liveUrlIndex);
    return playerInit()?.whenComplete(_startSizeSub);
  }

  Future<void> queryLiveInfoH5() async {
    final res = await RustLive.roomInfoH5(roomId: roomId);
    if (res case Success(:final response)) {
      roomInfoH5.value = response;
      title.value = response.title ?? '';
      watchedShow.value = response.watchedText;
      videoPlayerServiceHandler?.onVideoDetailChange(response, roomId, heroTag);
    } else {
      res.toast();
    }
  }

  void _showDialog(String title) {
    showDialog(
      context: Get.context!,
      builder: (_) => AlertDialog(
        title: Text(title),
        actions: [
          TextButton(
            onPressed: Get.back,
            child: Text(
              '关闭',
              style: TextStyle(color: ThemeUtils.theme.colorScheme.outline),
            ),
          ),
          TextButton(
            onPressed: () {
              if (plPlayerController.isDesktopPip) {
                plPlayerController.exitDesktopPip();
              }
              Get
                ..back()
                ..back();
            },
            child: const Text('退出'),
          ),
        ],
      ),
    );
  }

  void scrollToBottom() {
    EasyThrottle.throttle(
      'liveDm',
      const Duration(milliseconds: 500),
      () => WidgetsBinding.instance.addPostFrameCallback(_scrollToBottom),
    );
  }

  void _scrollToBottom([_]) {
    if (scrollController.hasClients) {
      scrollController.animateTo(
        scrollController.position.maxScrollExtent,
        duration: const Duration(milliseconds: 500),
        curve: Curves.linearToEaseOut,
      );
    }
  }

  void handleJumpToBottom() {
    disableAutoScroll.value = false;
    if (shouldRefresh) {
      messages.refresh();
      WidgetsBinding.instance.addPostFrameCallback(_jumpToBottom);
    } else {
      _jumpToBottom();
    }
  }

  void _jumpToBottom([_]) {
    if (scrollController.hasClients) {
      scrollController.jumpTo(scrollController.position.maxScrollExtent);
    }
  }

  void closeLiveMsg() {
    _msgStream?.close();
    _msgStream = null;
  }

  bool _ownsSession(LiveMessageSession owner, int generation) =>
      identical(_msgStream, owner) &&
      owner.active &&
      owner.generation == generation;

  Future<void> prefetch(LiveMessageSession owner, int generation) async {
    final res = await RustLiveDanmaku.prefetch(roomId);
    if (!_ownsSession(owner, generation)) return;
    if (res.data case final response?) {
      final mask = liveDanmakuFilterMask(
        candidates: response
            .map((item) => LiveFilterCandidate(text: item.text, mid: item.mid))
            .toList(),
        rules: RustLiveDanmaku.rules(_keywordList, _shieldUids),
      );
      messages.addAll(
        response.indexed
            .where((entry) => mask[entry.$1])
            .map((entry) => entry.$2),
      );
      scrollToBottom();
    } else if (kDebugMode) {
      Utils.reportError(res.errorDetail ?? '获取历史弹幕失败');
    }
  }

  Future<void> getSuperChatMsg(LiveMessageSession owner, int generation) async {
    final res = await RustLiveDanmaku.superChat(roomId);
    if (!_ownsSession(owner, generation)) return;
    if (res.data case final list?) {
      superChatMsg.addAll(list.map(LiveSuperChatState.new));
    }
  }

  void clearSC() {
    superChatMsg.removeWhere((e) => e.expired);
  }

  Future<void> _fetchBlockRules() async {
    final res = await RustLive.shieldInfo(roomId: roomId);
    if (res case Success(:final response)) {
      // 没有屏蔽设置时 Rust 给空列表而不是 null，直接赋值即可。
      _keywordList = response.keywords;
      _shieldUids = response.shieldUsers.map((e) => e.uid).toSet();
      _pushBlockRules();
    }
  }

  void updateBlockRules(List<String> keywords, Set<int> uids) {
    _keywordList = keywords;
    _shieldUids = uids;
    _pushBlockRules();
  }

  void _pushBlockRules() {
    final handle = _msgStream?.handle;
    if (handle == null) return;
    final result = liveSocketSetRules(
      sessionId: handle.sessionId,
      generation: handle.generation,
      rules: RustLiveDanmaku.rules(_keywordList, _shieldUids),
    );
    if (result.errorKind != null) {
      SmartDialog.showToast(result.errorDetail ?? '更新直播弹幕屏蔽设置失败');
    }
  }

  void startLiveMsg() {
    if (_msgStream?.active ?? false) return;
    closeLiveMsg();
    late final LiveMessageSession owner;
    owner = LiveMessageSession(
      token: () => RustLiveDanmaku.token(roomId),
      open: (token, generation) => liveSocketOpen(
        servers: token.servers,
        roomId: roomId,
        uid: Accounts.heartbeat.mid,
        token: token.token,
        generation: generation,
        transport: RustTransport.current(),
        rules: RustLiveDanmaku.rules(_keywordList, _shieldUids),
      ),
      poll: (handle) => liveSocketPoll(
        sessionId: handle.sessionId,
        generation: handle.generation,
      ),
      closeHandle: (handle) => liveSocketClose(
        sessionId: handle.sessionId,
        generation: handle.generation,
      ),
      onBatch: (batch) {
        if (!_ownsSession(owner, batch.generation)) return;
        if (kDebugMode && batch.dropped > BigInt.zero) {
          debugPrint('直播弹幕队列丢弃 ${batch.dropped} 条消息');
        }
        for (final event in batch.events) {
          if (!_ownsSession(owner, batch.generation)) return;
          _danmakuListener(event);
        }
      },
      onError: (kind, detail) {
        if (identical(_msgStream, owner)) {
          SmartDialog.showToast(detail ?? '直播弹幕连接失败（$kind）');
        }
      },
    );
    _msgStream = owner;
    owner.start();
    final generation = owner.generation;
    if (messages.isEmpty) {
      prefetch(owner, generation);
      if (showSuperChat) getSuperChatMsg(owner, generation);
    }
  }

  void listener() {
    final userScrollDirection = scrollController.position.userScrollDirection;
    if (userScrollDirection == .forward) {
      disableAutoScroll.value = true;
    } else if (userScrollDirection == .reverse) {
      final pos = scrollController.position;
      if (pos.maxScrollExtent - pos.pixels <= 100 && disableAutoScroll.value) {
        disableAutoScroll.value = false;
        refreshMsgIfNeeded();
      }
    }
  }

  void refreshMsgIfNeeded() {
    if (shouldRefresh) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        messages.refresh();
      });
    }
  }

  @override
  void onClose() {
    _stopSizeSub();
    closeLiveMsg();
    cancelLikeTimer();
    cancelLiveTimer();
    savedDanmaku?.clear();
    savedDanmaku = null;
    messages.clear();
    if (showSuperChat) {
      superChatMsg.clear();
      if (enableFSSC) {
        fsSC.value = null;
      }
    }
    scrollController
      ..removeListener(listener)
      ..dispose();
    pageController?.dispose();
    danmakuController = null;
    super.onClose();
  }

  // 修改画质
  Future<void>? changeQn(LiveQuality qn) {
    if (currentQn.value == qn) {
      return null;
    }
    currentQn.value = qn;
    return queryLiveUrl();
  }

  void addDm(dynamic msg, [DanmakuContentItem<DanmakuExtra>? item]) {
    _trimDm();

    if (plPlayerController.showDanmaku) {
      if (item != null && plPlayerController.enableShowLiveDanmaku.value) {
        danmakuController?.addDanmaku(item);
      }
      if (autoScroll && !disableAutoScroll.value) {
        messages.add(msg);
        scrollToBottom();
        return;
      }
    }

    messages.addOnly(msg);
  }

  void _danmakuListener(LiveEvent event) {
    switch (event.kind) {
      case LiveEventKind.danmaku:
        final item = event.danmaku;
        if (item == null) return;
        final extra = LiveDanmaku(
          id: item.id,
          mid: item.mid,
          dmType: item.dmType,
          ts: item.ts,
          ct: item.ct,
        );
        addDm(
          item,
          DanmakuContentItem(
            item.text,
            color: DanmakuOptions.blockColorful
                ? Colors.white
                : DmUtils.decimalToColor(item.color),
            type: DmUtils.getPosition(item.mode),
            selfSend: isLogin && item.mid == mid,
            extra: extra,
          ),
        );
        break;
      case LiveEventKind.superChat:
        if (!showSuperChat || event.superChat == null) return;
        final item = LiveSuperChatState(event.superChat!);
        superChatMsg.insert(0, item);
        addDm(item);
        if (!enableFSSC) return;
        if (Platform.isAndroid && AndroidHelper.isPipMode) return;
        if (plPlayerController.showDanmaku &&
            (isFullScreen || plPlayerController.isDesktopPip)) {
          fsSC.value = LiveSuperChatState(
            item.data,
            displayEndTime: math.min(
              item.data.endTime,
              DateTime.now().millisecondsSinceEpoch ~/ 1000 + 10,
            ),
          );
        }
        break;
      case LiveEventKind.watchedChange:
        watchedShow.value = event.text;
        break;
      case LiveEventKind.onlineRankCount:
        onlineCount.value = NumUtils.numFormat(event.count);
        break;
      case LiveEventKind.roomChange:
        title.value = event.text ?? '';
        break;
    }
  }

  final RxInt likeClickTime = 0.obs;
  Timer? likeClickTimer;

  void cancelLikeTimer() {
    likeClickTimer?.cancel();
    likeClickTimer = null;
  }

  void onLikeTapDown(_) {
    cancelLikeTimer();
    likeClickTime.value++;
  }

  void onLikeTapUp([_]) {
    likeClickTimer ??= Timer(const Duration(milliseconds: 800), onLike);
  }

  Future<void> onLike() async {
    if (!isLogin) {
      likeClickTime.value = 0;
      return;
    }
    final res = await LiveHttp.liveLikeReport(
      clickTime: likeClickTime.value,
      roomId: roomId,
      uid: mid,
      anchorId: roomInfoH5.value?.uid,
    );
    if (res.isSuccess) {
      SmartDialog.showToast('点赞成功');
    } else {
      res.toast();
    }
    likeClickTime.value = 0;
  }

  void toastNotLogin() {
    SmartDialog.showToast('账号未登录');
  }

  void onSendDanmaku([bool fromEmote = false]) {
    if (kReleaseMode && !isLogin) {
      toastNotLogin();
      return;
    }
    Get.key.currentState!.push(
      PublishRoute(
        barrierColor: Colors.transparent,
        pageBuilder: (context, animation, secondaryAnimation) {
          return Theme(
            data: ThemeUtils.darkTheme,
            child: LiveSendDmPanel(
              liveRoomController: this,
              items: savedDanmaku,
              autofocus: !fromEmote,
              panelType: fromEmote ? .emoji : .none,
              onSave: (msg) {
                if (msg.isEmpty) {
                  savedDanmaku?.clear();
                  savedDanmaku = null;
                } else {
                  savedDanmaku = msg.toList();
                }
              },
            ),
          );
        },
        transitionDuration: fromEmote
            ? const Duration(milliseconds: 400)
            : PlatformUtils.isDesktop
            ? const Duration(milliseconds: 350)
            : const Duration(milliseconds: 400),
      ),
    );
  }

  void onAtUser(LiveDanmakuMessage item) {
    savedDanmaku = [
      RichTextItem.fromStart(
        '@${item.name} ',
        rawText: item.mid.toString(),
        type: .at,
        id: item.id.toString(),
      ),
    ];
    onSendDanmaku();
  }

  void reportSC(LiveSuperChatState item) {
    if (!isLogin) {
      toastNotLogin();
      return;
    }
    autoWrapReportDialog(
      Get.context!,
      ReportOptions.liveDanmakuReport,
      withContent: ReportOptions.liveDanmakuReportCheck,
      contentRequired: ReportOptions.liveDanmakuReportCheck,
      (reasonType, reasonDesc, banUid) {
        return LiveHttp.superChatReport(
          id: item.data.id,
          roomId: roomId,
          uid: item.data.uid,
          msg: item.data.message,
          reason: ReportOptions.liveDanmakuReport['']![reasonType]!,
          ts: item.data.ts,
          token: item.data.token,
        );
      },
    );
  }

  void toggleDanmaku() {
    final newVal = !plPlayerController.enableShowLiveDanmaku.rawValue;
    plPlayerController.enableShowLiveDanmaku.value = newVal;
    if (!plPlayerController.tempPlayerConf) {
      GStorage.setting.put(
        SettingBoxKey.enableShowLiveDanmaku,
        newVal,
      );
    }
  }
}
