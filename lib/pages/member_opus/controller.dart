import 'package:kilikili/grpc/bilibili/app/dynamic/v2.pb.dart'
    show OpusSpaceFlowResp, OpusFlowItem, SectionOpusCollection;
import 'package:kilikili/grpc/space.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/member/controller.dart';
import 'package:collection/collection.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:get/get.dart';

class MemberOpusController
    extends CommonListController<OpusSpaceFlowResp, OpusFlowItem> {
  MemberOpusController({
    required this.heroTag,
    required this.mid,
  });

  final String? heroTag;
  final int mid;

  String? next;
  /// 当前选中的筛选项。服务端不下发筛选条时就用这个兜底。
  ///
  /// **比较一律只看 `meta`**（见 [isSelected]）：基线给
  /// `SpaceTabFilter` 自定义过 `==`，只比 `meta`；Rust 那边是生成的结构，
  /// 三个字段都比。文案由服务端决定，拿它参与相等判断等于把选中态压在
  /// 一句可变的中文上。
  Rx<SpaceTabFilter> type = const SpaceTabFilter(
    text: "全部图文",
    meta: "all",
    tabName: "图文",
  ).obs;

  bool isSelected(SpaceTabFilter filter) => filter.meta == type.value.meta;
  List<SpaceTabFilter>? filter;
  SectionOpusCollection? collection;

  @override
  void onInit() {
    super.onInit();
    filter = Get.find<MemberController>(tag: heroTag).tab2
        ?.firstWhereOrNull((e) => e.param == 'contribute')
        ?.items
        .firstWhereOrNull((e) => e.param == 'opus')
        ?.filters;
    queryData();
  }

  @override
  Future<void> onRefresh() {
    next = null;
    collection = null;
    return super.onRefresh();
  }

  @override
  List<OpusFlowItem>? getDataList(OpusSpaceFlowResp response) {
    final next = response.nextPage.next;
    if (next.isNotEmpty) {
      this.next = next;
    } else {
      isEnd = true;
    }
    return response.itemList;
  }

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<OpusSpaceFlowResp> response,
  ) {
    if (isRefresh && response.response.hasHostUpOpusCollection()) {
      collection = response.response.hostUpOpusCollection;
    }
    return super.customHandleResponse(isRefresh, response);
  }

  @override
  Future<LoadingState<OpusSpaceFlowResp>> customGetData() =>
      SpaceGrpc.opusSpaceFlow(
        hostMid: mid,
        next: next,
        filterType: type.value.meta,
      );

  void onCollapseCollection() {
    try {
      final outerController = Get.find<MemberController>(tag: heroTag)
          .scrollKey
          .currentState!
          .outerController;
      outerController.jumpTo(outerController.offset);
    } catch (_) {
      if (kDebugMode) rethrow;
    }
  }
}
