import 'package:material_ui/material_ui.dart';

/// 背景先覆盖完整视口，只有交互内容避让系统状态栏。
class MinePageBackground extends StatelessWidget {
  const MinePageBackground({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topRight,
          end: Alignment.bottomLeft,
          colors: [
            colors.primary.withValues(alpha: 0.055),
            colors.surface,
            colors.surface,
          ],
        ),
      ),
      child: SafeArea(left: false, right: false, bottom: false, child: child),
    );
  }
}

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
  Widget build(BuildContext context) => Material(
    color: Theme.of(context).colorScheme.surfaceContainerLow,
    borderRadius: BorderRadius.circular(28),
    clipBehavior: Clip.antiAlias,
    child: Padding(
      padding: const EdgeInsets.all(5),
      // 窄屏保留实际点击区域，不能像旧 PlayerBar 那样缩放整组工具。
      child: Wrap(
        spacing: 2,
        runSpacing: 2,
        alignment: WrapAlignment.end,
        children: children,
      ),
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

class MineInfoPill extends StatelessWidget {
  const MineInfoPill({
    super.key,
    required this.icon,
    required this.label,
    required this.value,
  });
  final IconData icon;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
      decoration: BoxDecoration(
        color: theme.colorScheme.primaryContainer.withValues(alpha: 0.4),
        borderRadius: BorderRadius.circular(24),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 17, color: theme.colorScheme.primary),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              '$label $value',
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.labelMedium?.copyWith(
                color: theme.colorScheme.onSurface,
              ),
            ),
          ),
        ],
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
    this.tools = const [],
    this.onLongPress,
    this.onSecondaryTap,
    this.nameColor,
  });
  final Widget avatar;
  final String name;
  final Widget level;
  final List<Widget> details;
  final List<Widget> statistics;
  final List<Widget> tools;
  final VoidCallback onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onSecondaryTap;
  final Color? nameColor;

  Widget _profileTap(Widget child) => GestureDetector(
    behavior: HitTestBehavior.opaque,
    onTap: onTap,
    onLongPress: onLongPress,
    onSecondaryTap: onSecondaryTap,
    child: child,
  );

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final nameStyle = theme.textTheme.titleLarge?.copyWith(
      fontWeight: FontWeight.w600,
      color: nameColor,
    );
    final painter = TextPainter(
      text: TextSpan(text: name, style: nameStyle),
      textDirection: Directionality.of(context),
      textScaler: MediaQuery.textScalerOf(context),
      maxLines: 1,
    )..layout();
    final nameWidth = painter.width;
    painter.dispose();
    // 工具保留 44 点击区，布局只按昵称字宽和可用宽度决定，不缩小图标挤进同一行。
    final toolsWidth = tools.isEmpty
        ? 0.0
        : tools.length * 44.0 + (tools.length - 1) * 2 + 10;
    return MineSurface(
      padding: const EdgeInsets.fromLTRB(22, 22, 16, 22),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final inlineTools =
              tools.isNotEmpty &&
              constraints.maxWidth >=
                  64 + 16 + nameWidth + 38 + 12 + toolsWidth;
          final nameWidget = _profileTap(
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Flexible(
                  child: Text(
                    name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: nameStyle,
                  ),
                ),
                const SizedBox(width: 6),
                level,
              ],
            ),
          );
          return Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (tools.isNotEmpty && !inlineTools) ...[
                Align(
                  alignment: Alignment.centerRight,
                  child: ConstrainedBox(
                    constraints: BoxConstraints(maxWidth: toolsWidth),
                    child: MineToolCapsule(children: tools),
                  ),
                ),
                const SizedBox(height: 18),
              ],
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _profileTap(avatar),
                  const SizedBox(width: 16),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (inlineTools)
                          Row(
                            children: [
                              Expanded(
                                child: ConstrainedBox(
                                  constraints: const BoxConstraints(
                                    minHeight: 54,
                                  ),
                                  child: Align(
                                    alignment: Alignment.centerLeft,
                                    child: nameWidget,
                                  ),
                                ),
                              ),
                              const SizedBox(width: 12),
                              SizedBox(
                                width: toolsWidth,
                                child: MineToolCapsule(children: tools),
                              ),
                            ],
                          )
                        else
                          nameWidget,
                        const SizedBox(height: 10),
                        _profileTap(
                          Wrap(
                            spacing: 8,
                            runSpacing: 8,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            children: details,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              Padding(
                padding: const EdgeInsets.only(top: 20, bottom: 14),
                child: Divider(
                  height: 1,
                  color: theme.colorScheme.outlineVariant.withValues(
                    alpha: 0.4,
                  ),
                ),
              ),
              Row(
                children: [
                  for (var i = 0; i < statistics.length; i++) ...[
                    if (i > 0) const SizedBox(width: 8),
                    Expanded(child: statistics[i]),
                  ],
                ],
              ),
            ],
          );
        },
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
    return Semantics(
      button: true,
      child: Tooltip(
        message: '查看$label',
        child: Material(
          color: theme.colorScheme.surfaceContainerLow,
          borderRadius: BorderRadius.circular(22),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: onTap,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 14),
              child: Wrap(
                alignment: WrapAlignment.center,
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: 7,
                runSpacing: 5,
                children: [
                  Text(
                    count,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Flexible(
                        child: Text(
                          label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ),
                      const SizedBox(width: 4),
                      Icon(
                        Icons.chevron_right,
                        size: 12,
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ],
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
