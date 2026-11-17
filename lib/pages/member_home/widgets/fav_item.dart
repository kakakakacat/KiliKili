import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

/// 空间主页上的一个收藏夹。
///
/// 收的是几个标量而不是某个取数模型：这一块的数据来自 Rust 的
/// `SpacePage`，而收藏夹列表页那条接口还在 Dart 模型上，两边共用这个
/// 组件。参数化之后它谁都不认识，哪边先迁都不用动它。
class MemberFavItem extends StatelessWidget {
  const MemberFavItem({
    super.key,
    required this.mediaId,
    required this.title,
    required this.cover,
    required this.count,
    required this.isPublic,
  });

  final int? mediaId;
  final String? title;
  final String? cover;

  /// 夹子里有多少条内容。
  final int? count;

  /// `1` 为私密。键名与含义是反的，这是服务端的事实。
  final int? isPublic;

  @override
  Widget build(BuildContext context) {
    void onLongPress() => imageSaveDialog(
      title: title,
      cover: cover,
    );
    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: () {
          Get.toNamed(
            '/favDetail',
            parameters: {
              'mediaId': mediaId.toString(),
              'heroTag': Utils.makeHeroTag(mediaId),
            },
          );
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
              NetworkImgLayer(
                src: cover,
                width: 160,
                height: 100,
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
                    const Spacer(),
                    Text(
                      '$count个内容 · ${isPublic == 1 ? '私密' : '公开'}',
                      style: TextStyle(
                        fontSize: 12,
                        color: Theme.of(context).colorScheme.outline,
                      ),
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
