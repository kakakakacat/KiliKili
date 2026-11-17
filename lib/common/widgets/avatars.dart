import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:collection/collection.dart';
import 'package:material_ui/material_ui.dart';

/// 一排叠在一起的小头像。
///
/// 收的是**头像地址**而不是某个用户模型：调用点的用户来自四五种不同的
/// 结构（动态投票、共同关注、充电榜、大航海），其中几种已经是 Rust 的
/// typed 结构，再要求它们先裹成 `Owner` 只是多造一次垃圾。
Widget avatars({
  required ColorScheme colorScheme,
  required Iterable<String?> faces,
  double gap = 6.0,
}) {
  const size = 22.0;
  const padding = 0.8;
  final offset = size - gap;
  const imgSize = size - 2 * padding;
  if (faces.length == 1) {
    return NetworkImgLayer(
      src: faces.first,
      width: imgSize,
      height: imgSize,
      type: .avatar,
    );
  } else {
    final decoration = BoxDecoration(
      shape: .circle,
      border: Border.all(color: colorScheme.surface),
    );
    return SizedBox(
      height: size,
      width: offset * faces.length + gap,
      child: Stack(
        clipBehavior: .none,
        children: faces
            .mapIndexed(
              (i, e) => Positioned(
                top: 0,
                bottom: 0,
                width: size,
                left: i * offset,
                child: DecoratedBox(
                  decoration: decoration,
                  child: Padding(
                    padding: const .all(padding),
                    child: NetworkImgLayer(
                      src: e,
                      width: imgSize,
                      height: imgSize,
                      type: .avatar,
                    ),
                  ),
                ),
              ),
            )
            .toList(),
      ),
    );
  }
}
