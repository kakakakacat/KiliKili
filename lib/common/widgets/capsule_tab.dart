import 'package:material_ui/material_ui.dart';

/// 带图标的胶囊 Tab：跟随 [TabController] 的动画实时切换选中态。
///
/// 首页分类（#138）先用上了这套样式，但它直接写在 `lib/pages/home/view.dart`
/// 里。那是上游文件，把它就地抽出来会让分叉面多出一块与本次无关的改动，
/// 所以这里另起一份给登录页用（#162）；首页那份等有单子统一时再合并过来。
class CapsuleTab extends StatelessWidget {
  const CapsuleTab({
    super.key,
    required this.controller,
    required this.index,
    required this.icon,
    required this.label,
  });

  /// 这个 Tab 所属的 TabBar 控制器，用来判断自己是不是当前选中项。
  final TabController controller;

  /// 自己在 TabBar 中的下标。
  final int index;

  final IconData icon;
  final String label;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return AnimatedBuilder(
      animation: controller.animation!,
      builder: (context, child) {
        // 点击时跟随目标页，手势滑动时跟随页面位置，避免再另存一份选中状态。
        final selectedIndex = controller.indexIsChanging
            ? controller.index
            : controller.animation!.value.round();
        final selected = selectedIndex == index;
        final foreground = selected
            ? colorScheme.onPrimary
            : colorScheme.onSurfaceVariant;
        return Container(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
          alignment: Alignment.center,
          decoration: ShapeDecoration(
            color: selected
                ? colorScheme.primary
                : colorScheme.surfaceContainerHighest,
            shape: const StadiumBorder(),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 18, color: foreground),
              const SizedBox(width: 6),
              Text(
                label,
                style: TextStyle(
                  color: foreground,
                  fontWeight: selected ? FontWeight.w600 : FontWeight.w400,
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}
