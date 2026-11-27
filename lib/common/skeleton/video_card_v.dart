import 'package:kilikili/common/skeleton/skeleton.dart';
import 'package:kilikili/common/style.dart';
import 'package:material_ui/material_ui.dart';

/// 封面卡片（`CoverScrimCard`）的骨架。
///
/// 和卡片同一个结构：封面块铺满格子扣除文字区之后的高度，下方两条占位对应
/// 「UP 主名 + 标题」那两行（#295），否则加载完的一瞬间会有一次明显的重排。
///
/// **刻意不画遮罩**：遮罩的颜色是从封面算出来的，封面还没到就没有那个颜色
/// 可画，遮罩跟着封面一起淡入，见 `CoverScrimCard`。
///
/// 不自己定宽高比：调用方的 grid delegate 已经按「封面比例 + 文字区」切好
/// 格子，这里铺满即可，番剧（3:4）和视频（16:10）才能共用同一个骨架。
class VideoCardVSkeleton extends StatelessWidget {
  const VideoCardVSkeleton({super.key, this.skeleton = true});

  final bool skeleton;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final block = colorScheme.onInverseSurface;
    final child = ClipRRect(
      borderRadius: Style.coverCardRadius,
      child: Column(
        crossAxisAlignment: .stretch,
        children: [
          Expanded(child: ColoredBox(color: block)),
          Padding(
            padding: const .fromLTRB(8, 9, 8, 9),
            child: Column(
              crossAxisAlignment: .start,
              children: [
                Container(height: 13, color: block),
                Container(
                  height: 13,
                  width: 120,
                  color: block,
                  margin: const .only(top: 6),
                ),
              ],
            ),
          ),
        ],
      ),
    );
    if (skeleton) return Skeleton(child: child);
    return child;
  }
}
