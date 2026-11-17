import 'package:kilikili/models/common/enum_with_label.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;

/// 合集 / 列表的排序方向。文案留在宿主，发给服务端的那个值在 Rust。
enum ArchiveSortTypeApp with EnumWithLabel {
  desc('默认'),
  asc('倒序'),
  ;

  @override
  final String label;
  const ArchiveSortTypeApp(this.label);

  rust.SpaceArchiveSort get kind => switch (this) {
    desc => rust.SpaceArchiveSort.desc,
    asc => rust.SpaceArchiveSort.asc,
  };
}
