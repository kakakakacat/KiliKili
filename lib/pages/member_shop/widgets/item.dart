import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/models/common/badge_type.dart';

import 'package:kilikili/utils/extension/theme_ext.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class MemberShopItem extends StatelessWidget {
  const MemberShopItem({
    super.key,
    required this.item,
  });

  final SpaceShopItem item;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final belowLabels = item.belowLabels.join('|');
    return Card(
      shape: const RoundedRectangleBorder(borderRadius: .all(.circular(6))),
      child: InkWell(
        borderRadius: const .all(.circular(6)),
        onTap: () {
          if (item.cardUrl case final cardUrl?) {
            Get.toNamed('/webview', parameters: {'url': cardUrl});
          }
        },
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            LayoutBuilder(
              builder: (context, constraints) => NetworkImgLayer(
                borderRadius: const .vertical(top: .circular(6)),
                src: item.cover,
                width: constraints.maxWidth,
                height: constraints.maxWidth,
              ),
            ),
            Padding(
              padding: const EdgeInsets.all(8),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    item.title!,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  if (belowLabels.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.symmetric(vertical: 2),
                      child: PBadge(
                        text: belowLabels,
                        type: PBadgeType.shop,
                        size: PBadgeSize.small,
                        isStack: false,
                        fontSize: 10,
                        padding: const EdgeInsets.symmetric(
                          horizontal: 3,
                          vertical: 2,
                        ),
                      ),
                    ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      if (item.netPrice case final netPrice?)
                        Text.rich(
                          style: TextStyle(color: colorScheme.vipColor),
                          TextSpan(
                            children: [
                              if (item.pricePrefix?.isNotEmpty == true)
                                TextSpan(
                                  text: '${item.pricePrefix} ',
                                  style: const TextStyle(fontSize: 11),
                                ),
                              TextSpan(
                                text: '${item.priceSymbol}$netPrice',
                                style: const TextStyle(
                                  fontWeight: FontWeight.w500,
                                ),
                              ),
                            ],
                          ),
                        ),
                      if (item.benefits.isNotEmpty)
                        Text(
                          item.benefits.join('|'),
                          style: TextStyle(
                            fontSize: 11,
                            color: colorScheme.outline,
                          ),
                        ),
                    ],
                  ),
                  if (item.itemSourceName?.isNotEmpty == true)
                    Text(
                      '来自${item.itemSourceName}',
                      style: TextStyle(
                        fontSize: 11,
                        color: colorScheme.freeColor,
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
