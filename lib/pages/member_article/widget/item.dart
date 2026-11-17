import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/utils/app_scheme.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

/// 一条专栏。
///
/// 收的是几个标量而不是某个取数模型：空间主页那一块已经来自 Rust，
/// 专栏列表页那条接口还在 Dart 模型上，两边共用这个组件。
class MemberArticleItem extends StatelessWidget {
  const MemberArticleItem({
    super.key,
    required this.title,
    required this.cover,
    required this.publishTimeText,
    required this.view,
    required this.reply,
    required this.uri,
  });

  final String? title;
  final String? cover;
  final String? publishTimeText;
  final int? view;
  final int? reply;
  final String? uri;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final outline = theme.colorScheme.outline;
    void onLongPress() => imageSaveDialog(title: title, cover: cover);
    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: () {
          if (uri?.isNotEmpty == true) {
            PiliScheme.routePushFromUrl(uri!);
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
              if (cover?.isNotEmpty == true) ...[
                AspectRatio(
                  aspectRatio: Style.aspectRatio,
                  child: LayoutBuilder(
                    builder:
                        (BuildContext context, BoxConstraints boxConstraints) {
                          return NetworkImgLayer(
                            src: cover!,
                            width: boxConstraints.maxWidth,
                            height: boxConstraints.maxHeight,
                          );
                        },
                  ),
                ),
                const SizedBox(width: 10),
              ],
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (title?.isNotEmpty == true) ...[
                      Expanded(
                        child: Text(
                          title!,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontSize: theme.textTheme.bodyMedium!.fontSize,
                            height: 1.42,
                            letterSpacing: 0.3,
                          ),
                        ),
                      ),
                    ],
                    Text(
                      '$publishTimeText',
                      style: TextStyle(
                        fontSize: 12,
                        height: 1,
                        color: outline,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Row(
                      spacing: 16,
                      children: [
                        StatWidget(
                          type: StatType.view,
                          value: view,
                          color: outline,
                        ),
                        StatWidget(
                          type: StatType.reply,
                          value: reply,
                          color: outline,
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
