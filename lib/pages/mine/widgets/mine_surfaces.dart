import 'package:material_ui/material_ui.dart';

/// 只承载视觉和交互；账号、等级和接口规则仍由调用方提供。
class MineSurface extends StatelessWidget {
  const MineSurface({
    super.key,
    required this.child,
    this.padding = EdgeInsets.zero,
    this.radius = 26,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;
  final double radius;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Material(
      color: colors.surfaceContainerLowest,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(radius),
        side: BorderSide(color: colors.outlineVariant.withValues(alpha: 0.35)),
      ),
      clipBehavior: Clip.antiAlias,
      child: Padding(padding: padding, child: child),
    );
  }
}

class MineToolCapsule extends StatelessWidget {
  const MineToolCapsule({super.key, required this.children});
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => MineSurface(
    radius: 28,
    padding: const EdgeInsets.all(5),
    // 窄屏保留实际点击区域，不能像旧 PlayerBar 那样缩放整组工具。
    child: Wrap(
      spacing: 2,
      runSpacing: 2,
      alignment: WrapAlignment.end,
      children: children,
    ),
  );
}

class MineToolButton extends StatelessWidget {
  const MineToolButton({
    super.key,
    required this.tooltip,
    required this.icon,
    required this.onPressed,
    this.selected,
  });
  final String tooltip;
  final IconData icon;
  final VoidCallback onPressed;
  final bool? selected;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return MergeSemantics(
      child: Semantics(
        toggled: selected,
        child: IconButton(
          tooltip: tooltip,
          onPressed: onPressed,
          icon: Icon(icon, size: 21),
          style: IconButton.styleFrom(
            minimumSize: const Size(44, 44),
            maximumSize: const Size(44, 44),
            padding: EdgeInsets.zero,
            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            foregroundColor: selected == true
                ? colors.primary
                : colors.onSurfaceVariant,
            backgroundColor: selected == true
                ? colors.primaryContainer
                : Colors.transparent,
            shape: const CircleBorder(),
          ),
        ),
      ),
    );
  }
}

class MineActionDock extends StatelessWidget {
  const MineActionDock({super.key, required this.actions});
  final List<({IconData icon, String title, VoidCallback onTap})> actions;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return MineSurface(
      padding: const EdgeInsets.all(8),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final largeText = MediaQuery.textScalerOf(context).scale(14) > 20;
          final columns =
              constraints.maxWidth < 300 ||
                  (largeText && constraints.maxWidth < 600)
              ? 2
              : 4;
          return Wrap(
            children: [
              for (final action in actions)
                SizedBox(
                  width: constraints.maxWidth / columns,
                  child: InkWell(
                    onTap: action.onTap,
                    borderRadius: BorderRadius.circular(22),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 6,
                        vertical: 15,
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          DecoratedBox(
                            decoration: BoxDecoration(
                              color: theme.colorScheme.primaryContainer
                                  .withValues(alpha: 0.45),
                              shape: BoxShape.circle,
                            ),
                            child: Padding(
                              padding: const EdgeInsets.all(11),
                              child: Icon(
                                action.icon,
                                size: 23,
                                color: theme.colorScheme.primary,
                              ),
                            ),
                          ),
                          const SizedBox(height: 10),
                          Text(
                            action.title,
                            textAlign: TextAlign.center,
                            style: theme.textTheme.labelLarge,
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
            ],
          );
        },
      ),
    );
  }
}

class MineAccountCard extends StatelessWidget {
  const MineAccountCard({
    super.key,
    required this.avatar,
    required this.name,
    required this.level,
    required this.details,
    required this.statistics,
    required this.onTap,
    this.onLongPress,
    this.onSecondaryTap,
    this.nameColor,
  });
  final Widget avatar;
  final String name;
  final Widget level;
  final List<Widget> details;
  final List<Widget> statistics;
  final VoidCallback onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onSecondaryTap;
  final Color? nameColor;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return MineSurface(
      padding: const EdgeInsets.all(22),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: onTap,
            onLongPress: onLongPress,
            onSecondaryTap: onSecondaryTap,
            child: Row(
              children: [
                avatar,
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              name,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.titleLarge?.copyWith(
                                fontWeight: FontWeight.w600,
                                color: nameColor,
                              ),
                            ),
                          ),
                          const SizedBox(width: 6),
                          level,
                        ],
                      ),
                      const SizedBox(height: 10),
                      Wrap(
                        spacing: 8,
                        runSpacing: 8,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: details,
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.only(top: 20, bottom: 10),
            child: Divider(
              height: 1,
              color: theme.colorScheme.outlineVariant.withValues(alpha: 0.4),
            ),
          ),
          Row(
            children: [
              for (final statistic in statistics) Expanded(child: statistic),
            ],
          ),
        ],
      ),
    );
  }
}

class MineStatButton extends StatelessWidget {
  const MineStatButton({
    super.key,
    required this.count,
    required this.label,
    required this.onTap,
  });
  final String count;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(16),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 12),
        child: Wrap(
          alignment: WrapAlignment.center,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: 7,
          runSpacing: 4,
          children: [
            Text(
              count,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.titleMedium?.copyWith(
                fontWeight: FontWeight.w600,
              ),
            ),
            Text(
              label,
              style: theme.textTheme.labelMedium?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class MineCollectionTile extends StatelessWidget {
  const MineCollectionTile({
    super.key,
    required this.cover,
    required this.title,
    required this.subtitle,
    required this.onTap,
  });
  final Widget cover;
  final String title;
  final String subtitle;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return MineSurface(
      radius: 22,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            children: [
              ClipRRect(
                borderRadius: BorderRadius.circular(14),
                child: SizedBox(width: 112, height: 90, child: cover),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.titleSmall,
                    ),
                    const SizedBox(height: 8),
                    Text(
                      subtitle,
                      maxLines: 3,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.labelMedium?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
