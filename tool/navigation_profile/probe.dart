import 'dart:convert';
import 'dart:developer';
import 'dart:ui';

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

/// 只复制进独立 profile 产品包，正常产品入口不引用本文件。
abstract final class NavigationProfile {
  static int target = -1;
  static int sample = 0;
  static bool armed = false;
  static bool painted = false;
  static bool fullPainted = false;
  static bool Function()? settled;
  static final pages = <int, RenderBox>{};
  static final minePending = <String>{};

  static void event(String event, [Map<String, Object?> extra = const {}]) {
    final record = <String, Object?>{
      'event': event,
      'us': Timeline.now,
      'sample': sample,
      'target': target,
      ...extra,
    };
    Timeline.instantSync('nav.$event', arguments: record);
    // 不记录账号、响应正文或 URL，原始数据可直接用于测量报告。
    // ignore: avoid_print
    print('NAV_PROFILE ${jsonEncode(record)}');
  }

  static void select(int index) {
    if (!armed || index != target) return;
    painted = fullPainted = false;
    event('callback');
  }

  static void sampleFrame() {
    final page = pages[target];
    if (!armed || page == null || !page.attached || !page.hasSize) return;
    final view = PlatformDispatcher.instance.views.first;
    final viewport = Offset.zero & (view.physicalSize / view.devicePixelRatio);
    final bounds = MatrixUtils.transformRect(
      page.getTransformTo(null),
      Offset.zero & page.size,
    );
    final visible = bounds.intersect(viewport);
    if (visible.width <= 1 || visible.height <= 1) return;
    paint(target);
  }

  static void paint(int index) {
    if (!armed || index != target) return;
    final frame = PlatformDispatcher.instance.frameData.frameNumber;
    if (!painted) {
      painted = true;
      event('first_paint', {'frame': frame});
    }
    if (!fullPainted && (settled?.call() ?? true)) {
      fullPainted = true;
      event('full_paint', {'frame': frame});
    }
  }

  static void frames(List<FrameTiming> frames) {
    if (!armed) return;
    for (final frame in frames) {
      event('frame', {
        'frame': frame.frameNumber,
        'build_us': frame.buildDuration.inMicroseconds,
        'raster_us': frame.rasterDuration.inMicroseconds,
        'vsync_us': frame.timestampInMicroseconds(FramePhase.vsyncStart),
        'raster_finish_us': frame.timestampInMicroseconds(
          FramePhase.rasterFinish,
        ),
      });
    }
  }
}

class NavigationProfilePage extends SingleChildRenderObjectWidget {
  const NavigationProfilePage({
    super.key,
    required this.index,
    required super.child,
  });
  final int index;

  @override
  RenderObject createRenderObject(BuildContext context) {
    NavigationProfile.event('page_mount', {'page': index});
    final render = RenderProxyBox();
    NavigationProfile.pages[index] = render;
    return render;
  }
}
