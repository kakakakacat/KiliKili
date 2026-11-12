import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_card/live_subtitle.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/services/rust/rust_search.dart';
import 'package:kilikili/utils/num_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class LiveItem extends StatelessWidget {
  final SearchLiveItem liveItem;

  const LiveItem({super.key, required this.liveItem});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    void onLongPress() => imageSaveDialog(
      title: liveItem.title,
      cover: liveItem.cover,
    );
    return CoverScrimCard(
      cover: liveItem.cover,
      title: liveItem.titleSpans.map((e) => e.text).join(),
      upName: liveItem.uname,
      onTap: () => PageUtils.toLiveRoom(liveItem.roomid),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      // 命中词上色。高亮色压在随封面变化的遮罩上，对比度不像 colors.title
      // 那样由 Rust 保证，所以按遮罩的明暗在主题色的两端里挑一个：浅遮罩
      // （深文字）配深色调，深遮罩（浅文字）配浅色调。
      titleBuilder: (context, colors) => Text.rich(
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
        TextSpan(
          style: TextStyle(height: 1.32, color: colors.title),
          children: liveItem.titleSpans
              .map(
                (e) => TextSpan(
                  text: e.text,
                  style: e.isEm
                      ? TextStyle(
                          color: colors.title.computeLuminance() > 0.5
                              ? theme.colorScheme.primaryFixed
                              : theme.colorScheme.onPrimaryFixedVariant,
                        )
                      : null,
                ),
              )
              .toList(),
        ),
      ),
      subtitleBuilder: (context, colors) => liveSubtitle(
        colors: colors,
        areaName: liveItem.cateName,
        watched: '${NumUtils.numFormat(liveItem.online)}围观',
      ),
    );
  }
}
