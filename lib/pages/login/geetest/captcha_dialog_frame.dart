import 'package:material_ui/material_ui.dart';

class CaptchaDialogFrame extends StatelessWidget {
  const CaptchaDialogFrame({required this.child, super.key});

  final Widget child;

  @override
  Widget build(BuildContext context) => Dialog(
    insetPadding: const EdgeInsets.all(16),
    constraints: const BoxConstraints(maxWidth: 360),
    clipBehavior: Clip.antiAlias,
    // Dialog 的父约束包含屏幕及键盘边距；这里仅设置大屏上限。
    child: SizedBox(width: 360, height: 440, child: child),
  );
}
