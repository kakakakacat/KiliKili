import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/http/loading_state.dart';

import 'package:kilikili/pages/common/common_list_controller.dart';

class MemberShopController
    extends CommonListController<SpaceShopPage, SpaceShopItem> {
  MemberShopController(this.mid);

  final int mid;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  bool? showMoreTab;
  String? clickUrl;
  String? showMoreDesc;

  @override
  List<SpaceShopItem>? getDataList(SpaceShopPage response) {
    isEnd = response.hasNextPage == false;
    showMoreTab = response.showMoreTab;
    clickUrl = response.moreUrl;
    showMoreDesc = response.showMoreDesc;
    return response.items;
  }

  @override
  Future<LoadingState<SpaceShopPage>> customGetData() =>
      RustSpace.shop(mid: mid);
}
