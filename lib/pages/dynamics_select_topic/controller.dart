import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/dynamic.dart' as rust;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_dynamic.dart';
import 'package:flutter/widgets.dart';
import 'package:get/get.dart';

class SelectTopicController
    extends
        CommonListController<rust.TopicPubSearchData, rust.TopicItemData> {
  final focusNode = FocusNode();
  final controller = TextEditingController();

  final RxBool enableClear = false.obs;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<rust.TopicItemData>? getDataList(rust.TopicPubSearchData response) {
    if (response.hasMore == false) {
      isEnd = true;
    }
    return response.items;
  }

  @override
  Future<LoadingState<rust.TopicPubSearchData>> customGetData() =>
      RustDynamic.topicPubSearch(
        keywords: controller.text,
        pageNum: page,
      );

  @override
  void onClose() {
    focusNode.dispose();
    controller.dispose();
    super.onClose();
  }
}
