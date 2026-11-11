import 'dart:math';

import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:get/get.dart';

class LiveAreaChildController
    extends CommonListController<LiveSecondPage, LiveRoomCard> {
  LiveAreaChildController(this.areaId, this.parentAreaId);
  final int areaId;
  final int parentAreaId;

  int? count;

  String? sortType;

  // tag
  final RxInt tagIndex = 0.obs;
  List<LiveSecondTag>? newTags;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  void checkIsEnd(int length) {
    if (count != null && length >= count!) {
      isEnd = true;
    }
  }

  @override
  List<LiveRoomCard>? getDataList(LiveSecondPage response) {
    count = response.count;
    newTags = response.tags;
    tagIndex.value = max(
      0,
      newTags?.indexWhere((e) => e.sortType == sortType) ?? 0,
    );
    return response.cards;
  }

  @override
  Future<LoadingState<LiveSecondPage>> customGetData() => RustLive.secondList(
    pn: page,
    areaId: areaId,
    parentAreaId: parentAreaId,
    sortType: sortType,
  );

  void onSelectTag(int index, String? sortType) {
    if (isLoading) {
      return;
    }
    tagIndex.value = index;
    this.sortType = sortType;

    onRefresh().whenComplete(scrollController.jumpToTop);
  }
}
