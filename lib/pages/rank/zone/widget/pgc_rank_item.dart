import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/utils/app_scheme.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/services/rust/rust_pgc.dart';
import 'package:material_ui/material_ui.dart';

class PgcRankCard extends StatelessWidget {
  const PgcRankCard({super.key, required this.item});

  final PgcRankItem item;

  void onLongPress() => showPgcCover(title: item.title, cover: item.cover);

  @override
  Widget build(BuildContext context) {
    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: () {
          if (item.url != null) {
            PiliScheme.routePushFromUrl(item.url!);
          }
        },
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: Style.safeSpace,
            vertical: 5,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              AspectRatio(
                aspectRatio: Style.aspectRatio3x4,
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    return NetworkImgLayer(
                      width: constraints.maxWidth,
                      height: constraints.maxHeight,
                      src: item.cover,
                      borderRadius: const BorderRadius.all(
                        Radius.circular(6),
                      ),
                    );
                  },
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: Text(item.title!),
                    ),
                    if (item.newEpIndexShow?.isNotEmpty == true) ...[
                      Text(
                        item.newEpIndexShow!,
                        maxLines: 1,
                        style: TextStyle(
                          fontSize: 12,
                          height: 1,
                          color: Theme.of(context).colorScheme.outline,
                          overflow: TextOverflow.clip,
                        ),
                      ),
                      const SizedBox(height: 4),
                    ],
                    Row(
                      children: [
                        StatWidget(
                          type: StatType.play,
                          value: item.view,
                        ),
                        const SizedBox(width: 8),
                        StatWidget(
                          type: StatType.follow,
                          value: item.follow,
                        ),
                      ],
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
