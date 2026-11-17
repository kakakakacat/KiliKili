import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/utils/page_utils.dart';

import 'package:material_ui/material_ui.dart';

class SpaceOpusItemGrpc extends StatelessWidget {
  const SpaceOpusItemGrpc({
    super.key,
    required this.item,
  });

  final SpaceOpusFlowItem item;

  @override
  Widget build(BuildContext context) {
    final hasPic = item.cover != null;
    final coverPic = item.cover;
    final hasExtend = item.content != null;
    return Card(
      shape: const RoundedRectangleBorder(borderRadius: Style.borderRadius6),
      child: InkWell(
        onTap: () => PageUtils.pushDynFromId(id: item.oid.toString()),
        borderRadius: Style.borderRadius6,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (hasPic)
              Stack(
                children: [
                  LayoutBuilder(
                    builder: (context, constraints) {
                      final height = constraints.maxWidth * coverPic!.ratio;
                      final isLongPic = coverPic.isLongPic;
                      return NetworkImgLayer(
                        width: constraints.maxWidth,
                        height: height,
                        src: coverPic.src,
                        borderRadius: const .vertical(top: .circular(6)),
                        quality: 60,
                        alignment: isLongPic ? .topCenter : .center,
                      );
                    },
                  ),
                  Positioned(
                    left: 0,
                    bottom: 0,
                    right: 0,
                    child: Container(
                      padding: const .only(left: 8, top: 8, bottom: 4),
                      decoration: const BoxDecoration(
                        gradient: LinearGradient(
                          begin: .bottomCenter,
                          end: .topCenter,
                          colors: [Colors.black54, Colors.transparent],
                        ),
                      ),
                      child: StatWidget(
                        type: StatType.like,
                        value: item.likeText ?? '',
                        color: Colors.white,
                      ),
                    ),
                  ),
                ],
              ),
            if (hasExtend)
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 8,
                  vertical: 10,
                ),
                child: Text(
                  item.content!,
                  maxLines: hasPic ? 4 : 6,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            if (!hasPic)
              Padding(
                padding: const EdgeInsets.only(left: 8, bottom: 8, right: 8),
                child: StatWidget(
                  type: StatType.like,
                  value: item.likeText ?? '',
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
          ],
        ),
      ),
    );
  }
}
