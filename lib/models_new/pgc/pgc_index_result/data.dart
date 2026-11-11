import 'package:kilikili/rust/api/pgc.dart' show PgcIndexItem;

/// 索引结果的列表容器。
///
/// S10 第一批（issue #60）之后这里不再解析任何东西：条目是 Rust 给的
/// [PgcIndexItem]，本类只是界面要的那个形状（`hasNext` + `list`）。
class PgcIndexResult {
  /// 接口给的是 0/1，不是布尔。保持原样，判断留给调用点。
  int? hasNext;
  List<PgcIndexItem>? list;

  PgcIndexResult({this.hasNext, this.list});
}
