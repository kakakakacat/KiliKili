import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_reply.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class EmotePanelController
    extends CommonListController<List<EmotePackage>?, EmotePackage>
    with GetSingleTickerProviderStateMixin {
  TabController? tabController;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<List<EmotePackage>?> response,
  ) {
    if (response.response?.isNotEmpty == true) {
      tabController = TabController(
        length: response.response!.length,
        vsync: this,
      );
    }
    loadingState.value = response;
    return true;
  }

  @override
  Future<LoadingState<List<EmotePackage>?>> customGetData() =>
      RustReply.emoteList(business: 'reply');

  @override
  void onClose() {
    tabController?.dispose();
    super.onClose();
  }
}
