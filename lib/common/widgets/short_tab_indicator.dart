import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

/// 水平 TabBar 的短条；竖排 TabBar 通过 [vertical] 使用同一套尺寸。
class ShortTabIndicator extends Decoration {
  const ShortTabIndicator({
    required this.color,
    this.isVertical = false,
  });

  final Color color;
  final bool isVertical;

  static const double length = 24;
  static const double thickness = 3;
  static const double edgeGap = 2;

  ShortTabIndicator vertical() => ShortTabIndicator(
    color: color,
    isVertical: true,
  );

  @override
  Decoration? lerpFrom(Decoration? a, double t) {
    if (a is ShortTabIndicator && a.isVertical == isVertical) {
      return ShortTabIndicator(
        color: Color.lerp(a.color, color, t)!,
        isVertical: isVertical,
      );
    }
    return super.lerpFrom(a, t);
  }

  @override
  Decoration? lerpTo(Decoration? b, double t) {
    if (b is ShortTabIndicator && b.isVertical == isVertical) {
      return ShortTabIndicator(
        color: Color.lerp(color, b.color, t)!,
        isVertical: isVertical,
      );
    }
    return super.lerpTo(b, t);
  }

  Rect _indicatorRect(Rect bounds) {
    if (isVertical) {
      final height = math.min(length, bounds.height);
      return Rect.fromLTWH(
        bounds.left + edgeGap,
        bounds.center.dy - height / 2,
        thickness,
        height,
      );
    }

    final width = math.min(length, bounds.width);
    return Rect.fromLTWH(
      bounds.center.dx - width / 2,
      bounds.bottom - edgeGap - thickness,
      width,
      thickness,
    );
  }

  @override
  Path getClipPath(Rect rect, TextDirection textDirection) => Path()
    ..addRRect(
      RRect.fromRectAndRadius(_indicatorRect(rect), const Radius.circular(2)),
    );

  @override
  BoxPainter createBoxPainter([VoidCallback? onChanged]) =>
      _ShortTabIndicatorPainter(this, onChanged);
}

class _ShortTabIndicatorPainter extends BoxPainter {
  _ShortTabIndicatorPainter(this.indicator, super.onChanged);

  final ShortTabIndicator indicator;

  @override
  void paint(Canvas canvas, Offset offset, ImageConfiguration configuration) {
    final bounds = offset & configuration.size!;
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        indicator._indicatorRect(bounds),
        const Radius.circular(2),
      ),
      Paint()..color = indicator.color,
    );
  }
}
