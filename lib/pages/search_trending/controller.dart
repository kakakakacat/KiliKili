import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/search/search_trending/data.dart';
import 'package:kilikili/services/rust/rust_search.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';

class SearchTrendingController
    extends CommonListController<SearchTrendingData, SearchTrendingItem> {
  int topCount = 0;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<SearchTrendingItem>? getDataList(SearchTrendingData response) {
    topCount = response.topCount;
    return response.list;
  }

  @override
  Future<LoadingState<SearchTrendingData>> customGetData() =>
      RustSearch.searchTrending(needsTop: true);
}
