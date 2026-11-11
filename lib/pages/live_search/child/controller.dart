import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/live/live_search_type.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/live_search/controller.dart';

class LiveSearchChildController
    extends CommonListController<LiveSearchData, dynamic> {
  LiveSearchChildController(this.controller, this.searchType);

  final LiveSearchController controller;
  final LiveSearchType searchType;

  @override
  void checkIsEnd(int length) {
    switch (searchType) {
      case LiveSearchType.room:
        if (controller.counts.first != -1 &&
            length >= controller.counts.first) {
          isEnd = true;
        }
        break;
      case LiveSearchType.user:
        if (controller.counts[1] != -1 && length >= controller.counts[1]) {
          isEnd = true;
        }
        break;
    }
  }

  @override
  List? getDataList(response) {
    switch (searchType) {
      case LiveSearchType.room:
        controller.counts[searchType.index] = response.totalRoom ?? 0;
        return response.rooms;
      case LiveSearchType.user:
        controller.counts[searchType.index] = response.totalUser ?? 0;
        return response.users;
    }
  }

  @override
  Future<LoadingState<LiveSearchData>> customGetData() {
    return RustLive.search(
      page: page,
      keyword: controller.editingController.text,
      kind: searchType.name,
    );
  }
}
