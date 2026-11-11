import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:get/get.dart';

class LiveFollowController
    extends CommonListController<LiveFollowingPage, LiveFollowItem> {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  final count = RxnInt();

  @override
  void checkIsEnd(int length) {
    final count = this.count.value;
    if (count != null && length >= count) {
      isEnd = true;
    }
  }

  @override
  List<LiveFollowItem>? getDataList(LiveFollowingPage response) {
    count.value = response.liveCount;
    return response.items;
  }

  @override
  Future<LoadingState<LiveFollowingPage>> customGetData() =>
      RustLive.follow(page);
}
