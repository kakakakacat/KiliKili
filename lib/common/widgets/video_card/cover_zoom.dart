import 'dart:async';

import 'package:get/get.dart';
import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:material_ui/material_ui.dart';
import 'package:media_kit_video/media_kit_video.dart';

/// 「封面放大」转场（#265）：点 feed 里的封面进播放页时，封面从卡片的位置
/// 放大到播放器的位置，播放页其余内容随后淡入；返回时反向。
///
/// 三块拼起来：
///
/// - 每张网络封面卡片（`CoverScrimCard`）的封面外包一层 [Hero]，tag 就是
///   这个 [CoverZoomSource]；点下去时记成「刚点的那张」，`toVideoPage`
///   按封面地址认领（[take]），所以所有进播放页的入口都不用单独传参；
/// - 播放页的播放器区域放一个同 tag 的 [CoverZoomHold]，它在视频首帧出来
///   之前一直盖着这张封面，避免放大落地之后先黑一下；
/// - 路由换成 [CoverZoomPageRoute]：页面只做淡入上移，不再整页推进来，
///   放大由 Hero 完成。
///
/// 飞行中、落地后盖着的都是**卡片上那张已经解码好的图**：按卡片当时的尺寸
/// 构造同参数的 [NetworkImgLayer]（内存缓存命中），再用 [FittedBox] 放大，
/// 所以全程不重新加载封面。代价是放大后偏糊，只持续到首帧出来。
class CoverZoomSource {
  CoverZoomSource({required this.cover});

  final String? cover;

  /// 卡片上封面的布局尺寸，由卡片布局时写入。按这个尺寸构造图片才能命中
  /// 卡片那张图的内存缓存（缓存键里有解码尺寸）。
  Size? size;

  /// 播放页那层封面是否已经让开。记在这里而不是 [CoverZoomHold] 的
  /// State 里：横竖屏切换、换布局都会重建播放器区域，State 跟着重来的话，
  /// 封面会重新盖回去半秒。
  bool released = false;

  // Hero tag 用对象本身的身份：每张卡每次构建一个，同一页里不会重复。

  static CoverZoomSource? _pending;
  static DateTime? _pendingAt;

  /// 「刚点的那张」最多认领多久。有的入口点下去之后还要先查 cid 再跳转，
  /// 留够余量；过期就当没点过。
  static const _pendingTtl = Duration(seconds: 4);

  /// 卡片被点（指针抬起）时调用。
  void markTapped() {
    _pending = this;
    _pendingAt = DateTime.now();
  }

  /// 播放页入口认领刚点的那张封面：只有没过期、并且**封面地址对得上**
  /// 才认，否则放大的会是另一张图（例如番剧卡片是季度封面，进的是单集）。
  /// 认领一次就清掉。
  static CoverZoomSource? take(String? cover) {
    final pending = _pending;
    final at = _pendingAt;
    _pending = null;
    _pendingAt = null;
    if (pending == null || at == null || cover == null) return null;
    if (DateTime.now().difference(at) > _pendingTtl) return null;
    if (_normalize(pending.cover) != _normalize(cover)) return null;
    return pending;
  }

  /// 去掉协议和缩略图后缀再比：同一张图在不同接口里可能是 `http:` /
  /// `https:` / `//` 开头，或带着 `@…` 尺寸参数。
  static String? _normalize(String? url) {
    if (url == null) return null;
    var s = url;
    final scheme = s.indexOf('//');
    if (scheme >= 0) s = s.substring(scheme + 2);
    final at = s.indexOf('@');
    if (at >= 0) s = s.substring(0, at);
    final query = s.indexOf('?');
    if (query >= 0) s = s.substring(0, query);
    return s;
  }

  /// 飞行中的那张图：圆角从卡片的圆角过渡到播放器的直角。
  ///
  /// push 时 [animation] 从 0 到 1，pop 时从 1 回到 0，所以同一个插值
  /// 两个方向都对。
  Widget flightShuttle(
    BuildContext flightContext,
    Animation<double> animation,
    HeroFlightDirection flightDirection,
    BuildContext fromHeroContext,
    BuildContext toHeroContext,
  ) => AnimatedBuilder(
    animation: animation,
    builder: (context, child) => ClipRRect(
      borderRadius: BorderRadius.lerp(
        Style.coverCardRadius,
        BorderRadius.zero,
        animation.value,
      )!,
      child: child,
    ),
    child: CoverZoomImage(source: this),
  );

  /// 卡片一侧的 Hero。
  Widget sourceHero(Widget child) =>
      Hero(tag: this, flightShuttleBuilder: flightShuttle, child: child);
}

/// 按卡片尺寸取图、再铺满当前区域的封面。
class CoverZoomImage extends StatelessWidget {
  const CoverZoomImage({super.key, required this.source});

  final CoverZoomSource source;

  @override
  Widget build(BuildContext context) {
    final size = source.size;
    if (size == null || size.isEmpty) {
      return LayoutBuilder(
        builder: (context, constraints) => NetworkImgLayer(
          src: source.cover,
          width: constraints.maxWidth,
          height: constraints.maxHeight,
          borderRadius: BorderRadius.zero,
        ),
      );
    }
    // 参数必须和 `CoverScrimCard` 里那张封面一致，否则缓存不命中。
    return FittedBox(
      fit: BoxFit.cover,
      clipBehavior: Clip.hardEdge,
      child: SizedBox.fromSize(
        size: size,
        child: NetworkImgLayer(
          src: source.cover,
          width: size.width,
          height: size.height,
          borderRadius: BorderRadius.zero,
        ),
      ),
    );
  }
}

/// 播放页一侧的 Hero：放大落地后继续盖着封面，直到视频首帧画出来再淡出。
///
/// 不会无限盖着：不自动播放时转场一结束就淡出（底下有播放页自己的封面
/// 层），拿不到首帧时最多盖 [_maxHold]。
class CoverZoomHold extends StatefulWidget {
  const CoverZoomHold({
    super.key,
    required this.source,
    required this.autoPlay,
    required this.videoController,
  });

  final CoverZoomSource source;
  final bool autoPlay;

  /// 当前的视频控制器；播放器是异步建起来的，所以这里轮询。
  final ValueGetter<VideoController?> videoController;

  @override
  State<CoverZoomHold> createState() => _CoverZoomHoldState();
}

class _CoverZoomHoldState extends State<CoverZoomHold> {
  /// 至少盖到转场结束，否则落地瞬间就开始淡出。
  static const _minHold = Duration(milliseconds: 450);
  static const _maxHold = Duration(seconds: 4);
  static const _poll = Duration(milliseconds: 100);

  bool _minElapsed = false;
  bool _frameReady = false;
  Timer? _minTimer;
  Timer? _maxTimer;
  Timer? _pollTimer;

  @override
  void initState() {
    super.initState();
    if (widget.source.released) return;
    _minTimer = Timer(_minHold, () => _set(() => _minElapsed = true));
    _maxTimer = Timer(_maxHold, () => _set(() => _frameReady = true));
    _pollTimer = Timer.periodic(_poll, (timer) {
      final controller = widget.videoController();
      if (controller == null) return;
      timer.cancel();
      controller.waitUntilFirstFrameRendered.then(
        (_) => _set(() => _frameReady = true),
      );
    });
  }

  void _set(VoidCallback fn) {
    if (mounted) setState(fn);
  }

  @override
  void dispose() {
    _minTimer?.cancel();
    _maxTimer?.cancel();
    _pollTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final source = widget.source;
    final released =
        source.released || (_minElapsed && (!widget.autoPlay || _frameReady));
    if (released && !source.released) {
      source.released = true;
      _maxTimer?.cancel();
      _pollTimer?.cancel();
    }
    return source.sourceHero(
      IgnorePointer(
        child: AnimatedOpacity(
          opacity: released ? 0 : 1,
          duration: const Duration(milliseconds: 200),
          child: CoverZoomImage(source: widget.source),
        ),
      ),
    );
  }
}

/// 封面放大进播放页用的路由。
///
/// 放大本身交给 Hero，这里只让页面淡入并微微上移；内容的淡入排在转场后段，
/// 返回时同一段区间反着走，所以下方内容会先淡出、封面再缩回去。
///
/// 外层仍套一次全局「页面过渡动画」的转场（primary 固定为完成态），只为
/// 保留它对 [secondaryAnimation] 的处理和 iOS 的边缘返回手势。
class CoverZoomPageRoute<T> extends GetPageRoute<T> {
  CoverZoomPageRoute({
    required super.page,
    required super.settings,
    super.routeName,
  });

  static const _contentInterval = Interval(
    0.3,
    1,
    curve: Curves.easeOutCubic,
  );

  @override
  Duration get transitionDuration => const Duration(milliseconds: 380);

  @override
  Duration get reverseTransitionDuration => const Duration(milliseconds: 320);

  /// 不让上一页跟着做推出动画：封面是从上一页「长」出来的，上一页要原地
  /// 不动。
  @override
  DelegatedTransitionBuilder? get delegatedTransition => null;

  @override
  Widget buildTransitions(
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
    Widget child,
  ) {
    final content = CurvedAnimation(
      parent: animation,
      curve: _contentInterval,
    );
    return super.buildTransitions(
      context,
      kAlwaysCompleteAnimation,
      secondaryAnimation,
      FadeTransition(
        opacity: content,
        child: SlideTransition(
          position: Tween(
            begin: const Offset(0, 0.04),
            end: Offset.zero,
          ).animate(content),
          child: child,
        ),
      ),
    );
  }
}
