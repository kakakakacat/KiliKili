import 'dart:async';
import 'dart:ui' as ui;

import 'package:kilikili/rust/api/cover_scrim.dart' as rust_api;
import 'package:kilikili/utils/image_utils.dart';
import 'package:cached_network_image_ce/cached_network_image.dart';
import 'package:material_ui/material_ui.dart';

/// 一张封面对应的底部遮罩配色。
///
/// 颜色由 Rust 的 `bili_core::cover_scrim` 算出（AGENTS.md 第 2 节：阈值
/// 判断与算法下沉 Rust），这边只负责解码、缓存与上色。
@immutable
class CoverScrimColors {
  const CoverScrimColors({
    required this.scrim,
    required this.title,
    required this.subtitle,
  });

  CoverScrimColors.fromRust(rust_api.CoverScrimStyle style)
    : scrim = Color(style.scrimArgb),
      title = Color(style.titleArgb),
      subtitle = Color(style.subtitleArgb);

  /// 文案那一条的遮罩色，含 alpha。是封面底部区域的**主色**按对比度需要
  /// 调过明暗的结果，直接铺成一整块纯色半透明，不做渐变。
  final Color scrim;
  final Color title;
  final Color subtitle;

  static CoverScrimColors lerp(
    CoverScrimColors a,
    CoverScrimColors b,
    double t,
  ) => CoverScrimColors(
    scrim: Color.lerp(a.scrim, b.scrim, t)!,
    title: Color.lerp(a.title, b.title, t)!,
    subtitle: Color.lerp(a.subtitle, b.subtitle, t)!,
  );

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CoverScrimColors &&
          scrim == other.scrim &&
          title == other.title &&
          subtitle == other.subtitle;

  @override
  int get hashCode => Object.hash(scrim, title, subtitle);
}

/// 封面取色：解码一张极小的缩略图交给 Rust，结果按 URL 缓存。
///
/// ## 为什么是缩略图而不是原图
///
/// 列表里一屏就有十几张封面。按原图逐像素统计必然掉帧，而且
/// `ui.Image.toByteData` 要把纹理从 GPU 读回内存，代价随像素数线性增长。
/// 这里让解码器直接按 [_sampleSize] 出图（`maxWidth`/`maxHeight` 只影响解
/// 码尺寸，**不换** `cacheKey`，所以和正常显示共用同一份下载好的文件，不
/// 会多发一次请求）。
///
/// ## 缓存
///
/// 结果按**原始封面 URL** 缓存，容量 [_maxEntries] 的 LRU。滚动回来的卡片
/// 能同步命中，不会先闪一帧回退色。换了封面的卡片（同一个 widget 复用）
/// 由调用方按 URL 重新取，见 `CoverScrimCard`。
abstract final class CoverScrimCache {
  /// 采样边长。16px 下统计量是几百个像素，缩放本身又做了一次低通，正好压
  /// 掉单像素高光对分位数的干扰。
  static const _sampleSize = 16;

  /// 约等于「几屏卡片」。再大没有收益：滚远了的封面连文件缓存都未必还在。
  static const _maxEntries = 256;

  static final _cache = <String, CoverScrimColors>{};
  static final _pending = <String, Future<CoverScrimColors>>{};

  static CoverScrimColors? _fallback;

  /// 封面缺失 / 加载中 / 解码失败时的固定回退。
  static CoverScrimColors get fallback =>
      _fallback ??= CoverScrimColors.fromRust(rust_api.fallbackCoverScrim());

  /// 已经算好就同步返回，否则 null。
  static CoverScrimColors? peek(String? url) {
    if (url == null || url.isEmpty) return null;
    final hit = _cache.remove(url);
    if (hit != null) _cache[url] = hit; // 触达即提到队尾
    return hit;
  }

  /// 取某张封面的配色。失败返回 [fallback]，不抛。
  static Future<CoverScrimColors> of(String? url) {
    if (url == null || url.isEmpty) return Future.value(fallback);
    final hit = peek(url);
    if (hit != null) return Future.value(hit);
    return _pending[url] ??= _resolve(url).then((colors) {
      _pending.remove(url);
      _cache[url] = colors;
      if (_cache.length > _maxEntries) {
        _cache.remove(_cache.keys.first);
      }
      return colors;
    });
  }

  static Future<CoverScrimColors> _resolve(String url) async {
    ui.Image? image;
    try {
      image = await _decodeThumbnail(url);
      final data = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
      if (data == null) return fallback;
      return CoverScrimColors.fromRust(
        rust_api.computeCoverScrim(
          rgba: data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes),
          width: image.width,
          height: image.height,
        ),
      );
    } catch (_) {
      // 取不到配色不该影响卡片本身：回退样式照样可读。
      return fallback;
    } finally {
      image?.dispose();
    }
  }

  static Future<ui.Image> _decodeThumbnail(String url) {
    final provider = CachedNetworkImageProvider(
      ImageUtils.thumbnailUrl(url),
      maxWidth: _sampleSize,
      maxHeight: _sampleSize,
    );
    final completer = Completer<ui.Image>();
    final stream = provider.resolve(ImageConfiguration.empty);
    late final ImageStreamListener listener;
    listener = ImageStreamListener(
      (info, _) {
        stream.removeListener(listener);
        // clone：info.image 的生命周期归 ImageStream，这里要在监听器之外用。
        if (!completer.isCompleted) completer.complete(info.image.clone());
      },
      onError: (error, stack) {
        stream.removeListener(listener);
        if (!completer.isCompleted) completer.completeError(error, stack);
      },
    );
    stream.addListener(listener);
    return completer.future;
  }
}
