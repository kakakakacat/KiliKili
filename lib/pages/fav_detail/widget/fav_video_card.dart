import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/grpc/bilibili/app/listener/v1.pbenum.dart'
    show PlaylistSource;
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/models_new/fav/fav_detail/media.dart';
import 'package:kilikili/pages/audio/view.dart';
import 'package:kilikili/pages/fav_detail/controller.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

// 收藏视频卡片 - 水平布局
class FavVideoCardH extends StatelessWidget {
  final FavDetailItemModel item;
  final int? index;
  final BaseFavController? ctr;

  const FavVideoCardH({
    super.key,
    required this.item,
    this.index,
    this.ctr,
  }) : assert(ctr == null || index != null);

  bool get isSort => ctr == null;

  void _onTap() {
    // 0/4/16/...: 正常；9: up自己删除；1: 其他原因删除
    if (const [1, 9].contains(item.attr)) {
      Get.toNamed('/member?mid=${item.upper?.mid}');
      return;
    }

    switch (item.type) {
      case 12:
        AudioPage.toAudioPage(
          oid: item.id!,
          itemType: 3,
          from: PlaylistSource.AUDIO_CARD,
        );
        break;
      case 24:
        PageUtils.viewPgc(
          seasonId: item.ogv!.seasonId,
          epId: item.id,
        );
        break;
      default:
        ctr!.onViewFav(item, index);
        break;
    }
  }

  @override
  Widget build(BuildContext context) {
    final isOwner = !isSort && ctr!.isOwner;
    final enableMultiSelect = ctr?.enableMultiSelect.value ?? false;
    final colorScheme = ColorScheme.of(context);
    final onLongPress = isSort || enableMultiSelect
        ? null
        : isOwner
        ? () {
            ctr!
              ..enableMultiSelect.value = true
              ..onSelect(item);
          }
        : () => imageSaveDialog(
            title: item.title,
            cover: item.cover,
            bvid: item.bvid,
          );
    final badge = item.type == 12 ? '音频' : item.ogv?.typeName;
    final date = DateFormatUtils.dateFormat(item.favTime);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: item.cover,
        title: item.title ?? '',
        upName: item.upper?.name,
        onTap: isSort
            ? null
            : enableMultiSelect
            ? () => ctr!.onSelect(item)
            : _onTap,
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        topStart: item.type == 24
            ? null
            : CoverStatsPill(
                stats: [
                  (StatType.play, item.cntInfo?.play),
                  (StatType.danmaku, item.cntInfo?.danmaku),
                ],
              ),
        coverOverlays: [
          if (badge != null && badge.isNotEmpty)
            PBadge(text: badge, top: 6, right: 6, type: PBadgeType.gray),
          if (item.duration != null && item.duration! > 0)
            CoverDurationPill(
              text: DurationUtils.formatDuration(item.duration),
              top: badge == null || badge.isEmpty ? 6 : 32,
              right: 6,
            ),
        ],
        foregroundOverlays: [
          if (!isSort)
            Positioned.fill(child: selectMask(colorScheme, item.checked)),
        ],
        metaBuilder: (context, colors) => [
          if (item.type == 24 && item.intro?.isNotEmpty == true)
            Flexible(
              child: Text(
                item.intro!,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 11, color: colors.subtitle),
              ),
            ),
          const Spacer(),
          Text(
            date,
            maxLines: 1,
            style: TextStyle(fontSize: 11, color: colors.subtitle),
          ),
        ],
        trailing: isOwner
            ? SizedBox.square(
                dimension: 29,
                child: IconButton(
                  tooltip: '取消收藏',
                  icon: const Icon(Icons.clear, color: Colors.white, size: 18),
                  onPressed: () => showDialog(
                    context: context,
                    builder: (context) => AlertDialog(
                      title: const Text('提示'),
                      content: const Text('要取消收藏吗?'),
                      actions: [
                        TextButton(
                          onPressed: Get.back,
                          child: Text(
                            '取消',
                            style: TextStyle(color: colorScheme.outline),
                          ),
                        ),
                        TextButton(
                          onPressed: () {
                            Get.back();
                            ctr!.onCancelFav(index!, item.id!, item.type!);
                          },
                          child: const Text('确定取消'),
                        ),
                      ],
                    ),
                  ),
                ),
              )
            : null,
      ),
    );
  }
}
