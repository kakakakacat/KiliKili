import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/pages/history/widgets/item.dart'
    show historyLabel, historyProgressIndicator, onPushHistory;
import 'package:material_ui/material_ui.dart';

class MineHistoryItem extends StatelessWidget {
  const MineHistoryItem({
    super.key,
    required this.item,
    required this.onPop,
  });

  final HistoryItem item;
  final VoidCallback onPop;

  @override
  Widget build(BuildContext context) {
    final hasDuration = item.duration != null && item.duration != 0;
    return GestureDetector(
      onTap: () => onPushHistory(item).whenComplete(onPop),
      behavior: .opaque,
      child: Column(
        crossAxisAlignment: .start,
        children: [
          Stack(
            clipBehavior: .none,
            children: [
              NetworkImgLayer(
                src: item.cover,
                width: 180,
                height: 110,
              ),
              if (hasDuration)
                ...historyProgressIndicator(item, ColorScheme.of(context)),
              ?historyLabel(item),
            ],
          ),
          const SizedBox(height: 8),
          Text(
            ' ${item.title}',
            maxLines: 2,
            overflow: .ellipsis,
            style: const TextStyle(height: 1.42, letterSpacing: 0.3),
          ),
        ],
      ),
    );
  }
}
