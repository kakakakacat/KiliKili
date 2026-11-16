import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/pages/common/search/common_search_controller.dart';
import 'package:kilikili/pages/later/controller.dart' show BaseLaterController;
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:get/get.dart';

class LaterSearchController
    extends CommonSearchController<LaterPage, LaterItem>
    with
        IdMultiSelectMixin<LaterPage, LaterItem, int>,
        BaseLaterController {
  dynamic mid;
  dynamic count;

  /// 搜索页的多选条是自己的，不与稍后再看主页的那几个 tab 共用。
  @override
  final RxBool enableMultiSelect = false.obs;

  @override
  final RxInt rxCount = 0.obs;

  @override
  int idOf(LaterItem item) => item.aid ?? 0;

  @override
  void onInit() {
    final args = Get.arguments;
    mid = args['mid'];
    count = args['count'];
    super.onInit();
  }

  @override
  Future<LoadingState<LaterPage>> customGetData() => RustHistory.seeYouLater(
    page: page,
    keyword: editController.value.text,
  );

  @override
  List<LaterItem>? getDataList(LaterPage response) {
    return response.list;
  }
}
