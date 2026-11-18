import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

class FavTopicController
    extends CommonListController<FavTopicList, FavTopicItem> {
  int? total;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  void checkIsEnd(int length) {
    if (total != null && length >= total!) {
      isEnd = true;
    }
  }

  @override
  List<FavTopicItem>? getDataList(FavTopicList response) {
    total = response.total;
    return response.items;
  }

  @override
  Future<void> onRefresh() {
    total = null;
    return super.onRefresh();
  }

  @override
  Future<LoadingState<FavTopicList>> customGetData() =>
      RustFav.favTopic(page: page);

  Future<void> onRemove(int index, int id) async {
    final res = await RustFav.favTopicWrite(topicId: id, fav: false);
    if (res.isSuccess) {
      loadingState
        ..value.data!.removeAt(index)
        ..refresh();
      SmartDialog.showToast('已取消收藏');
    } else {
      res.toast();
    }
  }
}
