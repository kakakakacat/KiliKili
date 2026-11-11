import 'dart:math';

import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:get/get_state_manager/src/rx_flutter/rx_ticket_provider_mixin.dart';
import 'package:material_ui/material_ui.dart' show TabController;

class LiveAreaDetailController
    extends CommonListController<List<LiveArea>?, LiveArea>
    with GetSingleTickerProviderStateMixin {
  LiveAreaDetailController(this.areaId, this.parentAreaId);
  final int areaId;
  final int parentAreaId;

  TabController? tabController;

  bool showFirstFrame = false;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<LiveArea>? getDataList(List<LiveArea>? response) {
    if (response != null && response.isNotEmpty) {
      assert(tabController == null);
      final initialIndex = max(0, response.indexWhere((e) => e.id == areaId));
      tabController = TabController(
        length: response.length,
        initialIndex: initialIndex,
        vsync: this,
      );
    }
    return response;
  }

  @override
  Future<LoadingState<List<LiveArea>?>> customGetData() =>
      RustLive.roomAreaList(parentId: parentAreaId);

  @override
  void onClose() {
    tabController?.dispose();
    tabController = null;
    super.onClose();
  }
}
