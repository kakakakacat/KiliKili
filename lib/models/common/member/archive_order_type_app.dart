import 'package:kilikili/models/common/enum_with_label.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;

/// 投稿视频的排序方式。文案留在宿主，发给服务端的那个值在 Rust。
enum ArchiveOrderTypeApp with EnumWithLabel {
  pubdate('最新发布'),
  click('最多播放'),
  ;

  @override
  final String label;
  const ArchiveOrderTypeApp(this.label);

  rust.SpaceArchiveOrder get kind => switch (this) {
    pubdate => rust.SpaceArchiveOrder.pubdate,
    click => rust.SpaceArchiveOrder.click,
  };
}
