import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/dynamic.dart' as rust;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_dynamic.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart' show TabController;

class BubbleController
    extends CommonListController<rust.BubbleData, rust.BubbleDynData>
    with GetSingleTickerProviderStateMixin {
  BubbleController(this.categoryId);

  /// 分类 id 在这条接口上是**字符串**，为 null 时整条参数不发（见 Rust 侧
  /// `bubble_request` 的注释）。
  final String? categoryId;

  late final String tribeId;
  int? sortType;

  final Rxn<rust.BubbleSortInfoData> sortInfo = Rxn<rust.BubbleSortInfoData>();
  TabController? tabController;
  final RxnString tribeName = RxnString();
  final Rxn<List<rust.BubbleCategoryData>> tabs =
      Rxn<List<rust.BubbleCategoryData>>();

  @override
  void onInit() {
    super.onInit();
    tribeId = Get.arguments['id'].toString();
    queryData();
  }

  @override
  List<rust.BubbleDynData>? getDataList(rust.BubbleData response) {
    return response.content?.items;
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<rust.BubbleData> response) {
    if (isRefresh) {
      final data = response.response;
      sortInfo.value = data.sortInfo;
      if (categoryId == null) {
        tribeName.value = data.baseInfo?.title;
        if (tabController == null) {
          final categories = data.categories;
          if (categories.isNotEmpty) {
            tabController = TabController(
              length: categories.length,
              vsync: this,
            );
            tabs.value = categories;
          }
        }
      }
    }
    return false;
  }

  @override
  Future<LoadingState<rust.BubbleData>> customGetData() => RustDynamic.bubble(
    tribeId: tribeId,
    categoryId: categoryId,
    sortType: sortType,
    page: page,
  );

  @override
  void onClose() {
    tabController?.dispose();
    tabController = null;
    super.onClose();
  }

  void onSort(int? sortType) {
    this.sortType = sortType;
    onReload();
  }
}
