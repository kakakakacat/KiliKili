import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';

class MemberCoinArcController
    extends CommonListController<CoinLikeArcPage, CoinLikeArcItem> {
  final int mid;
  MemberCoinArcController({required this.mid});

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<CoinLikeArcItem>? getDataList(CoinLikeArcPage response) {
    return response.items;
  }

  @override
  Future<LoadingState<CoinLikeArcPage>> customGetData() =>
      RustSpace.coinLikeArc(coin: true, mid: mid, page: page);
}
