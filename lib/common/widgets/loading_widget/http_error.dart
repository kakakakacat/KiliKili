import 'package:kilikili/common/assets.dart';
import 'package:kilikili/common/widgets/selection_text.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:material_ui/material_ui.dart';

class HttpError extends StatelessWidget {
  const HttpError({
    super.key,
    this.isSliver = true,
    this.errMsg,
    this.onReload,
    this.btnText,
    this.btnIcon,
    this.safeArea = true,
  });

  final bool isSliver;
  final String? errMsg;
  final VoidCallback? onReload;
  final String? btnText;
  final IconData? btnIcon;
  final bool safeArea;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final child = Column(
      mainAxisSize: .min,
      mainAxisAlignment: .center,
      crossAxisAlignment: .center,
      children: [
        const SizedBox(height: 40),
        SvgPicture.asset(Assets.error, height: 200),
        const SizedBox(height: 30),
        Padding(
          padding: const .symmetric(horizontal: 16, vertical: 5),
          child: SelectionText(
            errMsg ?? '没有数据',
            textAlign: .center,
            style: theme.textTheme.titleSmall,
          ),
        ),
        if (onReload != null)
          FilledButton.tonal(
            onPressed: onReload,
            style: FilledButton.styleFrom(
              tapTargetSize: .padded,
              backgroundColor: theme.colorScheme.primary.withAlpha(20),
            ),
            child: Row(
              mainAxisSize: .min,
              children: [
                if (btnIcon case final icon?) ...[
                  Icon(icon, size: 18, color: theme.colorScheme.primary),
                  const SizedBox(width: 8),
                ],
                Text(
                  btnText ?? '点击重试',
                  style: TextStyle(color: theme.colorScheme.primary),
                ),
              ],
            ),
          ),
        if (safeArea)
          SizedBox(height: 40 + MediaQuery.viewPaddingOf(context).bottom),
      ],
    );

    return isSliver
        ? SliverToBoxAdapter(child: child)
        : SizedBox(width: double.infinity, child: child);
  }
}
