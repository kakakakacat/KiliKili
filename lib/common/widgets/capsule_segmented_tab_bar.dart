import 'package:material_ui/material_ui.dart';

/// 一组互斥选项的分段胶囊：整条是一个 [StadiumBorder] 容器，选中项是在里面
/// 滑动的实心胶囊。
///
/// **和首页顶部分类（#138）那种「一颗一颗分开的胶囊」是两种控件，不要混用。**
/// 首页的分类数量不定、需要横向滚动，分开的胶囊是对的；这里是固定几选一，
/// Material 3 的 segmented button 才是这个场景的形态，也只有整条等分才能和
/// 下方的整行主操作按钮（#236）在视觉上成为一组。
///
/// 选中态直接跟 [TabController] 的动画走，不另存一份状态：手势拖动页面时
/// 胶囊跟着滑，而不是等松手才跳过去。
class CapsuleSegmentedTabBar extends StatelessWidget {
  const CapsuleSegmentedTabBar({
    super.key,
    required this.controller,
    required this.items,
    this.height = 46,
  });

  final TabController controller;

  /// (图标, 文字)，顺序与 [TabController] 的页面顺序一致。
  final List<(IconData, String)> items;

  final double height;

  /// 容器内缩，让选中胶囊看起来嵌在轨道里而不是贴着边。
  static const double _inset = 4;

  /// 一格窄于这个宽度就只留文字：图标是辅助信息，标签不是，挤不下时先丢图标。
  static const double _iconBreakpoint = 88;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return LayoutBuilder(
      builder: (context, constraints) {
        final showIcon =
            constraints.maxWidth / items.length >= _iconBreakpoint;
        return Container(
          height: height,
          padding: const EdgeInsets.all(_inset),
          decoration: ShapeDecoration(
            color: scheme.surfaceContainerHighest,
            shape: const StadiumBorder(),
          ),
          child: TabBar(
            controller: controller,
            // 不滚动：几格平分整条。滚动会让控件宽度随内容变，和整行按钮对不齐。
            isScrollable: false,
            indicator: ShapeDecoration(
              color: scheme.primary,
              shape: const StadiumBorder(),
            ),
            indicatorSize: TabBarIndicatorSize.tab,
            dividerHeight: 0,
            dividerColor: Colors.transparent,
            labelColor: scheme.onPrimary,
            unselectedLabelColor: scheme.onSurfaceVariant,
            labelPadding: EdgeInsets.zero,
            labelStyle: const TextStyle(
              fontSize: 14,
              fontWeight: FontWeight.w600,
            ),
            unselectedLabelStyle: const TextStyle(
              fontSize: 14,
              fontWeight: FontWeight.w400,
            ),
            splashBorderRadius: const BorderRadius.all(Radius.circular(100)),
            tabs: [
              for (final (icon, label) in items)
                Tab(
                  height: height - _inset * 2,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (showIcon) ...[
                        Icon(icon, size: 16),
                        const SizedBox(width: 6),
                      ],
                      Flexible(
                        child: Text(
                          label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}
