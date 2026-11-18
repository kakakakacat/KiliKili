import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/utils/accounts.dart';

class FavController extends CommonListController<FavFolderList, FavFolder> {
  late final account = Accounts.main;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<void> queryData([bool isRefresh = true]) {
    if (!account.isLogin) {
      loadingState.value = const Error('账号未登录');
      return Future.syncValue(null);
    }
    return super.queryData(isRefresh);
  }

  @override
  List<FavFolder>? getDataList(FavFolderList response) {
    if (response.hasMore == false) {
      isEnd = true;
    }
    return response.list;
  }

  @override
  Future<LoadingState<FavFolderList>> customGetData() =>
      RustFav.userFavFolder(
        pn: page,
        ps: 20,
        mid: account.mid,
      );
}
