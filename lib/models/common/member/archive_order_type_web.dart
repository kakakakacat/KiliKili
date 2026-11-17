import 'package:kilikili/models/common/enum_with_label.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;

/// 网页端投稿搜索的排序方式。文案留在宿主，发给服务端的那个值在 Rust。
enum ArchiveOrderTypeWeb with EnumWithLabel {
  pubdate('最新发布'),
  click('最多播放'),
  stow('最多收藏'),
  ;

  @override
  final String label;
  const ArchiveOrderTypeWeb(this.label);

  rust.SpaceArchiveSearchOrder get kind => switch (this) {
    pubdate => rust.SpaceArchiveSearchOrder.pubdate,
    click => rust.SpaceArchiveSearchOrder.click,
    stow => rust.SpaceArchiveSearchOrder.stow,
  };
}
