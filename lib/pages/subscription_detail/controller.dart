import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/services/rust/rust_user.dart' show SubItem;
import 'package:kilikili/pages/common/page_order/page_order_mixin.dart';
import 'package:get/get.dart';

class SubDetailController
    extends CommonListController<FavSeasonDetail, FavSeasonMedia>
    with PageOrderMixin {
  late int id;
  String? heroTag;
  SubItem? subInfo;

  @override
  int get count => subInfo?.mediaCount ?? 0;

  @override
  int get ps => _ps;
  static const _ps = 20;

  @override
  void onInit() {
    super.onInit();
    final args = Get.arguments;
    id = args['id'];
    subInfo = args['subInfo'];
    heroTag = args['heroTag'];

    if (count != 0) {
      updatePageOrderOnly(true);
      page = 1;
    }
    queryData();
  }

  @override
  List<FavSeasonMedia>? getDataList(FavSeasonDetail response) {
    // 列表页传进来的 info 与详情接口回的 info 是同一个形状，后者更新。
    subInfo = response.info ?? subInfo;
    return response.medias;
  }

  @override
  void checkIsEnd(int length) {
    if (pageDesc) {
      if (page == 1) isEnd = true;
    } else {
      final count = subInfo?.mediaCount;
      if (count != null && length >= count) {
        isEnd = true;
      }
    }
  }

  @override
  Future<LoadingState<FavSeasonDetail>> customGetData() =>
      RustFav.favSeasonList(
        seasonId: id,
        ps: _ps,
        pn: page,
        pageDesc: pageDesc,
      );
}
