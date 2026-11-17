import 'package:kilikili/services/rust/rust_space.dart' as rust;

/// 投稿页的六个 tab。
///
/// **只剩路由用途**：接口路径与封面档位已经随 S6 #56 下沉到 Rust
/// （`bili_api::space::SpaceArchiveKind`），这里留一个枚举是因为界面要
/// 按 tab 决定排序控件、标题文案与列表样式。[kind] 是两边的接头。
enum ContributeType {
  video,
  charging,
  season,
  series,
  bangumi,
  comic;

  rust.SpaceArchiveKind get kind => switch (this) {
    video => rust.SpaceArchiveKind.video,
    charging => rust.SpaceArchiveKind.charging,
    season => rust.SpaceArchiveKind.season,
    series => rust.SpaceArchiveKind.series,
    bangumi => rust.SpaceArchiveKind.bangumi,
    comic => rust.SpaceArchiveKind.comic,
  };
}
