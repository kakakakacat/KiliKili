import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

class FavArticleController
    extends CommonListController<FavArticleList, FavArticleItem> {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<FavArticleItem>? getDataList(FavArticleList response) {
    if (response.hasMore == false) {
      isEnd = true;
    }
    return response.items;
  }

  @override
  Future<LoadingState<FavArticleList>> customGetData() =>
      RustFav.favArticle(page: page);

  Future<void> onRemove(int index, String id) async {
    final res = await RustFav.communityAction(opusId: id, action: 4);
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
