import 'package:kilikili/services/rust/rust_space.dart' as rust;

/// 网页端的「合集」与「列表」。
///
/// **只剩路由用途**：两条接口的路径与参数名已经随 S6 #56 下沉到 Rust
/// （`bili_api::space::SpaceSeasonWebKind`）。
enum WebSsType {
  season,
  series;

  rust.SpaceSeasonWebKind get kind => switch (this) {
    season => rust.SpaceSeasonWebKind.season,
    series => rust.SpaceSeasonWebKind.series,
  };
}
