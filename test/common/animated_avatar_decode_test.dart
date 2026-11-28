import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter_test/flutter_test.dart';

/// 动态头像的解码底座测试（issue #292）。
///
/// #292 原本怀疑「`.gif`/动态 webp 头像在我们的 App 里被转成了静态图」。实测
/// 把怀疑拆成两层后，URL 层已排除：B 站图片服务把动图转成动态 webp **保留全
/// 部帧**，而头像 URL 走的 `@{q}q.webp`（`ImageUtils.thumbnailUrl`，默认 10q）
/// 正是这种——见 #292 里的参数对照表。
///
/// 剩下唯一没被纯推理定死的，是**解码这一层**：头像组件
/// （`NetworkImgLayer` → `CachedNetworkImage`）给小头像传了 `memCacheWidth`，
/// 底层 `ResizeImage` 会带着 `targetWidth` 去调 `instantiateImageCodec`。
/// 这一步到底是逐帧缩放、还是只取首帧？若是后者，动态头像就会在「尺寸够小」
/// 的所有列表里悄悄变成静态——正是 #292 担心的现象。
///
/// 这个测试用一张真实的 7 帧动态 webp（取自 B 站 CDN 的礼物动图经图片服务
/// 转码，无个人信息）把它钉死：**带目标尺寸解码后，帧数必须一帧不少。**
/// 刻意不碰 widget 层与 `Pref`/`storage`，所以在没打 Flutter SDK 补丁的机器
/// 上也跑得起来（见 AGENTS.md §5 的交集判据）。
void main() {
  const dir = 'test/common/fixtures';

  Future<ui.Codec> decode(String name, {int? targetWidth}) async {
    final bytes = await File('$dir/$name').readAsBytes();
    return ui.instantiateImageCodec(
      bytes,
      // 头像走的就是这一路：`memCacheWidth` → `ResizeImage` →
      // `targetWidth`。给出目标宽度，高度按比例，和 `NetworkImgLayer`
      // 对正方形头像的取值一致。
      targetWidth: targetWidth,
    );
  }

  test('动态 webp 原样解码保留全部帧', () async {
    final codec = await decode('animated_avatar.webp');
    expect(codec.frameCount, 7);
  });

  test('动态 webp 带目标尺寸解码仍保留全部帧（头像走的就是这条）', () async {
    // 48px 模拟评论区/列表里的小头像：尺寸远小于原图，必然触发缩放分支。
    final codec = await decode('animated_avatar.webp', targetWidth: 48);
    expect(
      codec.frameCount,
      7,
      reason: '带 targetWidth 解码把动图压成了单帧——头像会在列表里变静态',
    );
  });

  test('静态 webp 解码为单帧（对照组，确认帧数判据本身可信）', () async {
    final plain = await decode('static_avatar.webp');
    expect(plain.frameCount, 1);
    final resized = await decode('static_avatar.webp', targetWidth: 48);
    expect(resized.frameCount, 1);
  });
}
