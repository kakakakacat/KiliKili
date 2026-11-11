import 'package:kilikili/models_new/search/search_rcmd/data.dart';
import 'package:kilikili/rust/api/search.dart' show SearchTrendingItem;

/// 热搜榜的列表容器。
///
/// S9（issue #59）之后这里不再解析任何东西。置顶项的**拼接**（把
/// `top_list` 排到 `list` 前面并记下条数）已经移到 Rust：那是对协议的
/// 理解——服务端把置顶与普通项分两个数组给，合成一个列表是协议语义，
/// 不是排版。
class SearchTrendingData extends SearchRcmdData {
  /// 置顶条数。置顶项已排在 [list] 前面，界面据此给前 N 条画角标。
  int topCount;

  SearchTrendingData({
    List<SearchTrendingItem>? list,
    this.topCount = 0,
  }) : super(list: list);
}
