import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/model_hot_video_item.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_video_feed.dart';

class PopularPreciousController
    extends CommonListController<PopularPrecious, HotVideoItemModel> {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  int? mediaId;

  @override
  List<HotVideoItemModel>? getDataList(PopularPrecious response) {
    mediaId = response.mediaId?.toInt();
    return response.list?.map(HotVideoItemModel.fromRust).toList();
  }

  @override
  Future<LoadingState<PopularPrecious>> customGetData() =>
      RustVideoFeed.precious(page: page);
}
