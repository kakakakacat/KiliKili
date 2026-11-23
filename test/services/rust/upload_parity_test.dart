import 'package:flutter_test/flutter_test.dart';

/// 图片上传的一次性基线对照（ADR-015 / issue #193），机制见 ADR-011 决策五。
///
/// `UploadBfsResData` 已随本次迁移删除，没有「还活着的基线模型」可借，于是
/// 下面的 `_BaselineUploadBfs` 是它 `fromJson` 的**逐行转写**，取自合并前的
/// `main`（`lib/models_new/upload_bfs/data.dart`）。
///
/// 本测试**刻意不 import `lib/`**，这样它在没打 Flutter SDK 补丁的机器上
/// 也跑得起来（AGENTS.md §5）。
///
/// ## 这份对照抓到过一处真分叉
///
/// 基线 `imgSize` 是 `(json['img_size'] as num?)?.toDouble()`——**double**，
/// 而第一版 Rust 实现把它收成了 `i64`。这个值会**原样发回发布接口**（动态
/// 的图片描述里带着它），截成整数等于悄悄改了一个发给服务端的字段，
/// `204.8` 会变成 `204`。它不报错，界面上也看不出来。Rust 侧已改回 `f64`，
/// 下面第一条用例就是钉这件事的。
///
/// ## 没有 golden fixture，为什么
///
/// 这两条接口的响应各只有一两个字段（`location`；`image_url` + 宽高 + 大小），
/// 而真正难的部分——multipart 的分段编码与流式体——在**请求**侧，由
/// `bili-api/src/upload.rs` 的请求面单测钉住。为一个两字段的响应再立一份
/// raw/rust 快照，证明不了多出来的东西。
void main() {
  test('img_size 是浮点：截成整数会悄悄改掉发回服务端的值', () {
    const raw = <String, dynamic>{
      'image_url': 'https://example.invalid/bfs/new_dyn/a.jpg',
      'image_width': 1920,
      'image_height': 1080,
      'img_size': 204.8,
    };
    final b = _BaselineUploadBfs.fromJson(raw);
    expect(b.imgSize, 204.8);
    expect(
      b.imgSize is double,
      isTrue,
      reason: 'Rust 侧的 UploadedImage.size 必须跟着是 f64',
    );
    expect(b.imgSize!.toInt(), 204, reason: '截断会丢掉 0.8，这正是不能截的理由');
  });

  test('宽高是整数，地址是串', () {
    const raw = <String, dynamic>{
      'image_url': 'https://example.invalid/bfs/new_dyn/b.jpg',
      'image_width': 800,
      'image_height': 600,
      'img_size': 64.0,
    };
    final b = _BaselineUploadBfs.fromJson(raw);
    expect(b.imageUrl, 'https://example.invalid/bfs/new_dyn/b.jpg');
    expect(b.imageWidth, 800);
    expect(b.imageHeight, 600);
  });

  test('字段缺失时基线给 null，不给 0', () {
    final b = _BaselineUploadBfs.fromJson(const <String, dynamic>{});
    expect(b.imageUrl, isNull);
    expect(b.imageWidth, isNull);
    expect(b.imageHeight, isNull);
    expect(b.imgSize, isNull);
  });

  test('整数形态的 img_size 也收成 double（基线的 num.toDouble）', () {
    final b = _BaselineUploadBfs.fromJson(const <String, dynamic>{
      'img_size': 205,
    });
    expect(b.imgSize, 205.0);
  });
}

// ------------------------------------------------------------- oracle
//
// 迁移前 `lib/models_new/upload_bfs/data.dart` 的逐行转写。只为对照存在，
// 不要在这里「改好」任何东西。

class _BaselineUploadBfs {
  _BaselineUploadBfs({
    this.imageUrl,
    this.imageWidth,
    this.imageHeight,
    this.imgSize,
  });

  final String? imageUrl;
  final int? imageWidth;
  final int? imageHeight;
  final double? imgSize;

  factory _BaselineUploadBfs.fromJson(Map<String, dynamic> json) =>
      _BaselineUploadBfs(
        imageUrl: json['image_url'] as String?,
        imageWidth: json['image_width'] as int?,
        imageHeight: json['image_height'] as int?,
        imgSize: (json['img_size'] as num?)?.toDouble(),
      );
}
