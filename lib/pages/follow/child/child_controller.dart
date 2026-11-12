import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/member.dart';
import 'package:kilikili/http/user.dart';
import 'package:kilikili/models_new/follow/data.dart';
import 'package:kilikili/models_new/follow/list.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/follow/controller.dart';
import 'package:kilikili/services/rust/follow_compat.dart';
import 'package:kilikili/services/rust/rust_misc.dart';
import 'package:get/get.dart';

class FollowChildController
    extends CommonListController<FollowData, FollowItemModel> {
  FollowChildController(this.controller, this.mid, this.tagid);
  final FollowController? controller;
  final int? tagid;
  final int mid;
  int? total;

  late final loadSameFollow = controller?.isOwner == false;
  late final Rx<LoadingState<List<FollowItemModel>?>> sameState =
      LoadingState<List<FollowItemModel>?>.loading().obs;

  @override
  void onInit() {
    super.onInit();
    queryData();
    if (loadSameFollow) {
      _loadSameFollow();
    }
  }

  @override
  List<FollowItemModel>? getDataList(FollowData response) {
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
  bool customHandleResponse(bool isRefresh, Success<FollowData> response) {
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
  Future<LoadingState<FollowData>> customGetData() async {
    // 分组列表（`/x/relation/tag`）还在 Dart，归 S6 #56；它与下面这条共用
    // `FollowData`，所以这里暂时两种来源并存，见 `follow_compat.dart`。
    if (tagid != null) {
      return MemberHttp.followUpGroup(
        mid: mid,
        tagid: tagid,
        pn: page,
        orderType: controller?.orderType.value.type ?? '',
      );
    }

    final res = await RustMisc.followings(
      vmid: mid,
      pn: page,
      orderType: controller?.orderType.value.type ?? '',
    );
    return res.toFollowDataState();
  }

  Future<void> _loadSameFollow() async {
    final res = await UserHttp.sameFollowing(mid: mid);
    if (res case Success(:final response)) {
      sameState.value = Success(response.list);
    }
  }
}
