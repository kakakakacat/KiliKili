import 'package:kilikili/common/widgets/pair.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:kilikili/pages/common/common_controller.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/account_service.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:flutter/widgets.dart' show ScrollController;
import 'package:get/get.dart';

class LiveController extends CommonListController
    with AccountMixin, RefreshImplMixin {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  int? count;

  // area
  int? areaId;
  String? sortType;
  int? parentAreaId;
  final RxInt areaIndex = 0.obs;

  // tag
  final RxInt tagIndex = 0.obs;
  List<LiveSecondTag>? newTags;

  double _topHeight = 0.0;
  void onTopHeightChanged(double value) {
    _topHeight = value;
  }

  /// 顶部两块：关注区块、分区入口。
  ///
  /// 分区入口在 Rust 侧已对「服务端不下发」兜过底，为 null 只可能是
  /// 还没取过。
  final Rx<Pair<LiveFollowSection?, List<LiveRoomCard>?>> topState =
      Pair<LiveFollowSection?, List<LiveRoomCard>?>(
        first: null,
        second: null,
      ).obs;

  final followController = ScrollController();

  bool showFirstFrame = false;

  @override
  void checkIsEnd(int length) {
    if (count != null && length >= count!) {
      isEnd = true;
    }
  }

  @override
  List? getDataList(response) {
    return response.cards;
  }

  @override
  bool customHandleResponse(bool isRefresh, Success response) {
    if (isRefresh) {
      final res = response.response;
      if (res is LiveIndexPage) {
        if (!res.hasMore) {
          isEnd = true;
        }
        topState.value = Pair(
          first: res.follow,
          second: res.areaEntrance,
        );
      } else if (res is LiveSecondPage) {
        count = res.count;
        newTags = res.tags;
        if (sortType != null) {
          tagIndex.value =
              newTags?.indexWhere((e) => e.sortType == sortType) ?? -1;
        }
      }
    }
    return false;
  }

  @override
  Future<LoadingState> customGetData() {
    if (areaIndex.value != 0) {
      return RustLive.secondList(
        pn: page,
        areaId: areaId,
        parentAreaId: parentAreaId,
        sortType: sortType,
      );
    }
    return RustLive.feedIndex(pn: page);
  }

  @override
  Future<void> onRefresh() {
    count = null;
    page = 1;
    isEnd = false;
    if (areaIndex.value != 0) {
      queryTop().whenComplete(followController.jumpToTop);
      return queryData();
    }
    return queryData().whenComplete(followController.jumpToTop);
  }

  Future<void> queryTop() async {
    final res = await RustLive.feedIndex(pn: page, moduleSelect: true);
    if (res case Success(:final response)) {
      topState.value = Pair(
        first: response.follow,
        second: response.areaEntrance,
      );
      // +1 让出列表头部那个「推荐」；找不到时 indexWhere 回 -1，
      // +1 后正好落在「推荐」上。基线另有一条「列表为 null → -1，
      // 两者都不高亮」的分支，分区入口现在由 Rust 兜底，不会再为 null。
      areaIndex.value =
          response.areaEntrance.indexWhere(
            (e) => e.areaV2Id == areaId && e.areaV2ParentId == parentAreaId,
          ) +
          1;
    }
  }

  void onSelectArea(int index, LiveRoomCard? cardLiveItem) {
    if (isLoading) {
      return; // areaIndex conflict
    }
    if (index == areaIndex.value) {
      return;
    }
    tagIndex.value = 0;
    newTags = null;
    sortType = null;
    areaIndex.value = index;
    areaId = cardLiveItem?.areaV2Id;
    parentAreaId = cardLiveItem?.areaV2ParentId;

    _onSelectAreaTag();
  }

  void onSelectTag(int index, String? sortType) {
    if (isLoading) {
      return;
    }
    tagIndex.value = index;
    this.sortType = sortType;

    _onSelectAreaTag();
  }

  void _onSelectAreaTag() {
    count = null;
    page = 1;
    isEnd = false;
    queryData().whenComplete(_jumpToListTop);
  }

  void _jumpToListTop() {
    if (scrollController.hasClients &&
        scrollController.position.pixels > _topHeight) {
      scrollController.jumpTo(_topHeight);
    }
  }

  @override
  void onChangeAccount(bool isLogin) => onReload();

  @override
  void onClose() {
    followController.dispose();
    super.onClose();
  }
}
