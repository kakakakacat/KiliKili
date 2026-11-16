import 'package:flutter/material.dart';

/// 只负责绘制音量比例；手势、系统音量与隐藏时机仍由播放器管理。
class VolumeIndicator extends StatelessWidget {
  const VolumeIndicator({super.key, required this.level});

  final double level;

  @override
  Widget build(BuildContext context) {
    final fill = level.clamp(0.0, 1.0);
    return Align(
      alignment: const Alignment(0.8, 0),
      child: FractionallySizedBox(
        heightFactor: 0.6,
        child: Center(
          widthFactor: 1,
          child: SizedBox(
            width: 44,
            height: 144,
            child: ClipRRect(
              borderRadius: BorderRadius.circular(22),
              child: Stack(
                fit: StackFit.expand,
                children: [
                  const ColoredBox(color: Color(0x99000000)),
                  Positioned(
                    left: 0,
                    right: 0,
                    bottom: 12,
                    child: Icon(
                      fill == 0
                          ? Icons.volume_off_rounded
                          : Icons.volume_up_rounded,
                      color: Colors.white,
                      size: 22,
                    ),
                  ),
                  Align(
                    alignment: Alignment.bottomCenter,
                    child: FractionallySizedBox(
                      widthFactor: 1,
                      heightFactor: fill,
                      // 图标在填充内外分别用深浅色，极低音量时也保留完整轮廓。
                      child: ClipRect(
                        child: ColoredBox(
                          key: const ValueKey('volume-fill'),
                          color: Colors.white,
                          child: Stack(
                            children: [
                              Positioned(
                                left: 0,
                                right: 0,
                                bottom: 12,
                                child: Icon(
                                  fill == 0
                                      ? Icons.volume_off_rounded
                                      : Icons.volume_up_rounded,
                                  color: const Color(0xFF333333),
                                  size: 22,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
