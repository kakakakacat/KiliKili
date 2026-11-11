import 'package:kilikili/rust/api/search.dart' show SearchTrendingItem;

/// 搜索页默认词的列表容器。
///
/// S9（issue #59）之后这里不再解析任何东西：条目是 Rust 给的
/// [SearchTrendingItem]，本类只是界面要的那个形状。
class SearchRcmdData {
  List<SearchTrendingItem>? list;

  SearchRcmdData({this.list});
}
