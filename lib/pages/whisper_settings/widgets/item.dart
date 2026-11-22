import 'dart:math';

import 'package:kilikili/services/rust/rust_im.dart';
import 'package:material_ui/material_ui.dart';

/// 私信设置页的一行。
///
/// 三种形态：开关、选择、跳转。基线靠 `hasSwitch_1()` / `hasRedirect()` /
/// `hasSelect()` 分支；投影之后改看哪个字段有值，顺序一样。
///
/// **这里不再就地改 protobuf 对象。** FRB 导出的类型不可变，而且「改一项
/// 要把整张表发回去」那件事已经收进了 Rust（控制器只递一个 key 加新值）。
/// 于是乐观更新也去掉了：点下去等服务端回，成功后控制器重新拉一次。
/// 代价是多等一个往返，换来的是「界面上显示的就是服务端的状态」。
class ImSettingsItem extends StatelessWidget {
  const ImSettingsItem({
    super.key,
    required this.item,
    required this.onSwitch,
    required this.onSelect,
    required this.onRedirect,
  });

  final ImSetting item;
  final ValueChanged<bool> onSwitch;
  final ValueChanged<int> onSelect;
  final VoidCallback onRedirect;

  /// 副标题为空（null 或空串）时不占位。
  static Widget? _subtitle(String? text, TextStyle style) =>
      text == null || text.isEmpty ? null : Text(text, style: style);

  @override
  Widget build(BuildContext context) {
    const titleStyle = TextStyle(fontSize: 14);
    final colorScheme = ColorScheme.of(context);
    final outline = colorScheme.outline;
    final subtitleStyle = TextStyle(fontSize: 13, color: outline);

    if (item.switchOn case final on?) {
      return ListTile(
        dense: true,
        onTap: () => onSwitch(!on),
        title: Text(item.title ?? '', style: titleStyle),
        subtitle: _subtitle(item.subtitle, subtitleStyle),
        trailing: Transform.scale(
          alignment: Alignment.centerRight,
          scale: 0.8,
          child: Switch(value: on, onChanged: (value) => onSwitch(value)),
        ),
      );
    }

    if (item.redirect case final redirect?) {
      return ListTile(
        dense: true,
        onTap: onRedirect,
        title: Text(item.title ?? '', style: titleStyle),
        subtitle: _subtitle(item.subtitle, subtitleStyle),
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            // trailing 的三级回退（子设置里选中的选项 → 打开着的开关的
            // 标题 → selected_summary）在 Rust 侧算完了。
            if (redirect.trailingText case final text?)
              Text(text, style: TextStyle(fontSize: 13, color: outline)),
            Icon(color: outline, Icons.keyboard_arrow_right),
          ],
        ),
      );
    }

    if (item.options.isNotEmpty) {
      final divider = Divider(
        height: 1,
        indent: 16,
        color: outline.withValues(alpha: 0.1),
      );
      return Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: List.generate(
          max(0, item.options.length * 2 - 1),
          (index) {
            if (index.isOdd) {
              return divider;
            }
            final i = index ~/ 2;
            final option = item.options[i];
            final selected = item.selectedIndex == i;
            return ListTile(
              dense: true,
              // 传的是选项的 value，不是下标——写回时服务端按 value 认。
              onTap: selected ? null : () => onSelect(option.value),
              title: Text(option.title ?? '', style: titleStyle),
              trailing: selected
                  ? Icon(size: 20, Icons.check, color: colorScheme.primary)
                  : null,
            );
          },
        ),
      );
    }

    return const SizedBox.shrink();
  }
}
