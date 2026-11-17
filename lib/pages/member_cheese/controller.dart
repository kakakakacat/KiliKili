import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/http/loading_state.dart';

import 'package:kilikili/pages/common/common_list_controller.dart';

class MemberCheeseController
    extends CommonListController<SpaceCheesePage, SpaceCheeseItem> {
  MemberCheeseController(this.mid);

  final int mid;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<SpaceCheeseItem>? getDataList(SpaceCheesePage response) {
    isEnd = response.hasNext == false;
    return response.items;
  }

  @override
  Future<LoadingState<SpaceCheesePage>> customGetData() => RustSpace.cheese(
    page: page,
    mid: mid,
  );
}
