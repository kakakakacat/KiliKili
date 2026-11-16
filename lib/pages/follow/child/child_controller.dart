import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/follow/controller.dart';
import 'package:kilikili/services/rust/rust_misc.dart';
import 'package:kilikili/services/rust/rust_relation.dart';
import 'package:get/get.dart';

class FollowChildController
    extends CommonListController<FollowList, FollowUser> {
  FollowChildController(this.controller, this.mid, this.tagid);
  final FollowController? controller;
  final int? tagid;
  final int mid;
  int? total;

  late final loadSameFollow = controller?.isOwner == false;
  late final Rx<LoadingState<List<FollowUser>?>> sameState =
      LoadingState<List<FollowUser>?>.loading().obs;

  @override
  void onInit() {
    super.onInit();
    queryData();
    if (loadSameFollow) {
      _loadSameFollow();
    }
  }

  @override
  List<FollowUser>? getDataList(FollowList response) {
    total = response.total;
    return response.list;
  }

  @override
  void checkIsEnd(int length) {
    if (total != null && length >= total!) {
      isEnd = true;
    }
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<FollowList> response) {
    if (controller != null) {
      try {
        if (controller!.isOwner &&
            tagid == null &&
            isRefresh &&
            controller!.followState.value.isSuccess) {
          controller!.tabs
            ..[0].count = response.response.total
            ..refresh();
        }
      } catch (_) {}
    }
    return false;
  }

  @override
  Future<LoadingState<FollowList>> customGetData() {
    // 两条路径都已在 Rust：分组内用 `/x/relation/tag`（S6 #56），全部关注
    // 用 `/x/relation/followings`（#66）。响应形状相同，区别只在分组那条
    // 不给 total——`checkIsEnd` 因此对它永远走不到「按总数判结束」那支，
    // 靠 `CommonListController` 的「这一页不满就是最后一页」收尾。
    if (tagid != null) {
      return RustRelation.followUpGroup(
        mid: mid,
        tagid: tagid,
        pn: page,
        orderType: controller?.orderType.value.type ?? '',
      );
    }

    return RustMisc.followings(
      vmid: mid,
      pn: page,
      orderType: controller?.orderType.value.type ?? '',
    );
  }

  Future<void> _loadSameFollow() async {
    final res = await RustRelation.sameFollowing(mid: mid);
    if (res case Success(:final response)) {
      sameState.value = Success(response.list);
    }
  }
}
