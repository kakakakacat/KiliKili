import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class LiveEmotePanelController
    extends CommonListController<List<LiveEmotePackage>, LiveEmotePackage>
    with GetSingleTickerProviderStateMixin {
  LiveEmotePanelController(this.roomId);
  final int roomId;
  TabController? tabController;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<List<LiveEmotePackage>> response,
  ) {
    if (response.response.isNotEmpty) {
      tabController = TabController(
        length: response.response.length,
        vsync: this,
      );
    }
    loadingState.value = response;
    return true;
  }

  @override
  Future<LoadingState<List<LiveEmotePackage>>> customGetData() =>
      RustLive.emoticons(roomId: roomId);

  @override
  void onClose() {
    tabController?.dispose();
    super.onClose();
  }
}
