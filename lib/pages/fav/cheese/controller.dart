import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

class FavCheeseController
    extends CommonListController<FavPugvList, FavPugvItem> {
  final mid = Accounts.main.mid;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<FavPugvItem>? getDataList(FavPugvList response) {
    isEnd = response.hasNext == false;
    return response.items;
  }

  @override
  Future<LoadingState<FavPugvList>> customGetData() =>
      RustFav.favPugv(mid: mid, page: page);

  Future<void> onRemove(int index, int sid) async {
    final res = await RustFav.favPugvWrite(seasonId: sid, fav: false);
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
