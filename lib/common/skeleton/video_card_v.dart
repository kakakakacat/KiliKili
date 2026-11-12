import 'package:kilikili/common/skeleton/skeleton.dart';
import 'package:kilikili/common/style.dart';
import 'package:material_ui/material_ui.dart';

/// 竖版卡片的骨架 / 配色预览。
///
/// 卡片已经改成「整张封面 + 底部叠加文案」（issue #108），骨架也跟着变成
/// 一整块封面加底部两条占位，否则加载完的一瞬间会有一次明显的重排。
///
/// **刻意不画遮罩**：遮罩的颜色是从封面算出来的，封面还没到就没有那个颜色
/// 可画。先铺一条深色带再换成算出来的色，等于让人看两次不同的卡片。遮罩
/// 跟着封面一起淡入，见 `CoverScrimCard`。
///
/// 不自己定宽高比：调用方的 grid delegate 已经按封面比例切好格子，这里
/// 铺满即可，番剧（3:4）和视频（16:10）才能共用同一个骨架。
class VideoCardVSkeleton extends StatelessWidget {
  const VideoCardVSkeleton({super.key, this.skeleton = true});

  final bool skeleton;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final cover = colorScheme.onInverseSurface;
    // 占位条压在封面块上而不是遮罩上，得挑一个在深浅两套主题里都和
    // onInverseSurface 分得开的颜色；surface 在两边正好都在它的另一侧。
    final bar = colorScheme.surface;
    final child = ClipRRect(
      borderRadius: Style.coverCardRadius,
      child: Stack(
        fit: StackFit.expand,
        children: [
          ColoredBox(color: cover),
          Positioned(
            left: 9,
            right: 9,
            bottom: 9,
            child: Column(
              crossAxisAlignment: .start,
              children: [
                Container(height: 13, color: bar),
                Container(
                  height: 13,
                  width: 120,
                  color: bar,
                  margin: const .only(top: 6),
                ),
                Container(
                  height: 11,
                  width: 80,
                  color: bar,
                  margin: const .only(top: 7),
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
