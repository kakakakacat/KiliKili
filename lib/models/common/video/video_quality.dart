part of 'base_quality.dart';

final class VideoQuality extends BaseQuality {
  final String shortDesc;

  const VideoQuality(super.code, super.desc, this.shortDesc);

  @override
  String toString() => '${super.toString()}, shortDesc: $shortDesc';

  static const VideoQuality hdrVivid = .new(129, 'HDR Vivid', 'HDR Vivid');
  static const VideoQuality super8k = .new(127, '8K 超高清', '8K');
  static const VideoQuality dolbyVision = .new(126, '杜比视界', '杜比');
  static const VideoQuality hdr = .new(125, 'HDR 真彩', 'HDR');
  static const VideoQuality super4K = .new(120, '4K 超高清', '4K');
  static const VideoQuality high108060 = .new(116, '1080P 60帧', '1080P60');
  static const VideoQuality high1080plus = .new(112, '1080P 高码率', '1080P+');
  static const VideoQuality superResolution = .new(100, '智能修复', '智能修复');
  static const VideoQuality high1080 = .new(80, '1080P 高清', '1080P');
  static const VideoQuality high72060 = .new(74, '720P 60帧', '720P60');
  static const VideoQuality high720 = .new(64, '720P 准高清', '720P');
  static const VideoQuality clear480 = .new(32, '480P 标清', '480P');
  static const VideoQuality fluent360 = .new(16, '360P 流畅', '360P');
  static const VideoQuality speed240 = .new(6, '240P 极速', '240P');

  static const List<VideoQuality> values = [
    hdrVivid,
    super8k,
    dolbyVision,
    hdr,
    super4K,
    high108060,
    high1080plus,
    superResolution,
    high1080,
    high72060,
    high720,
    clear480,
    fluent360,
    speed240,
  ];

  static final _codeMap = {for (final i in values) i.code: i};

  VideoQuality.fromId(super.id, super.idStr) : shortDesc = idStr;

  factory VideoQuality.fromCode(int code) =>
      _codeMap[code] ?? .fromId(code, code.toString());

  static VideoQuality? safeFromCode(int code) => _codeMap[code];

  // `fromFormat` 随 `PlayUrlModel` 一起删了（issue #79）。
  // 它只是「响应给了名字就用响应的，否则查静态表」这条三级回退，现在
  // 挂在取流结构旁边：`lib/services/rust/play_url_ext.dart` 的
  // `videoQualityOf`。放在那里而不是这里，是为了让本层级不再 import
  // 取流模型——`BaseQuality` 的三个子类（画质、音质、直播画质）都是
  // 客户端静态表，不该依赖任何一条取数路径。
}
