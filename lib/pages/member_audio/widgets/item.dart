import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/grpc/bilibili/app/listener/v1.pbenum.dart'
    show PlaylistSource;
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/pages/audio/view.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

/// 一条音频。
///
/// 收的是几个标量而不是某个取数模型：空间主页那一块已经来自 Rust，
/// 音频列表页那条接口还在 Dart 模型上，两边共用这个组件。
///
/// [ctime] 一律是**Unix 秒**。基线靠「有没有 `statistic` 块」来决定除不除
/// 1000，那个判断挪到了两边的调用点——单位是各自接口的事实，组件不该
/// 替它们猜。
class MemberAudioItem extends StatelessWidget {
  const MemberAudioItem({
    super.key,
    required this.id,
    required this.uid,
    required this.title,
    required this.cover,
    required this.ctime,
    this.play,
    this.comment,
  });

  final int? id;
  final int? uid;
  final String? title;
  final String? cover;

  /// Unix 秒。
  final int? ctime;

  /// 播放与评论数。空间主页那一块服务端不给，此时整行不显示。
  final int? play;
  final int? comment;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final hasStat = play != null || comment != null;
    void onLongPress() => imageSaveDialog(title: title, cover: cover);
    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: () => AudioPage.toAudioPage(
          itemType: 3,
          id: uid!,
          oid: id!,
          from: PlaylistSource.MEM_SPACE,
        ),
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
                aspectRatio: 1,
                child: LayoutBuilder(
                  builder:
                      (BuildContext context, BoxConstraints boxConstraints) {
                        return NetworkImgLayer(
                          src: cover,
                          width: boxConstraints.maxWidth,
                          height: boxConstraints.maxHeight,
                          borderRadius: const BorderRadius.all(
                            Radius.circular(4),
                          ),
                        );
                      },
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title!,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 3),
                    Text(
                      DateFormatUtils.dateFormat(ctime),
                      style: TextStyle(
                        fontSize: 13,
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                    if (hasStat)
                      Row(
                        spacing: 16,
                        children: [
                          StatWidget(
                            type: StatType.listen,
                            value: play,
                          ),
                          StatWidget(
                            type: StatType.reply,
                            value: comment,
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
