import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/model_hot_video_item.dart';
import 'package:kilikili/pages/common/common_controller.dart'
    show RefreshImplMixin;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_video_feed.dart';

class HotController
    extends CommonListController<List<HotVideoItemModel>, HotVideoItemModel>
    with RefreshImplMixin {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<LoadingState<List<HotVideoItemModel>>> customGetData() =>
      RustVideoFeed.hot(pn: page, ps: 20);
}
