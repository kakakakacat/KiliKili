import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/follow_order_type.dart';
import 'package:kilikili/models/member/tags.dart';
import 'package:kilikili/pages/follow/child/child_controller.dart';
import 'package:kilikili/services/rust/rust_relation.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_key.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class FollowController extends GetxController with GetTickerProviderStateMixin {
  final String tag;
  FollowController(this.tag);

  late final int mid;
  late final RxnString name;
  late final bool isOwner;

  late final Rx<LoadingState> followState = LoadingState.loading().obs;
  late final RxList<MemberTagItemModel> tabs = <MemberTagItemModel>[].obs;
  TabController? tabController;

  late final Rx<FollowOrderType> orderType = Pref.followOrderType.obs;

  void toggleOrderType() {
    final FollowOrderType type = orderType.value == .def ? .attention : .def;
    orderType.value = type;
    for (var e in tabs) {
      try {
        Get.find<FollowChildController>(tag: '$tag${e.tagid}').onReload();
      } catch (_) {}
    }
    GStorage.setting.put(SettingBoxKey.followOrderType, type.index);
  }

  @override
  void onInit() {
    super.onInit();
    final Map? args = Get.arguments;
    final ownerMid = Accounts.main.mid;
    final int? mid = args?['mid'];
    this.mid = mid ?? ownerMid;
    isOwner = ownerMid == this.mid;
    if (isOwner) {
      queryFollowUpTags();
    } else {
      final String? name = args?['name'];
      this.name = RxnString(name);
      if (name == null) {
        _queryUserName();
      }
    }
  }

  Future<void> _queryUserName() async {
    final res = await RustSpace.memberCard(mid: mid);
    name.value = res.dataOrNull?.name;
  }

  Future<void> queryFollowUpTags() async {
    final res = await RustRelation.followUpTags();
    if (res case Success(:final response)) {
      // `FollowTag` 的字段是 final 的；tab 条要就地改名与改人数，所以在这里
      // 转成界面侧的可变状态（理由见 `MemberTagItemModel` 的类文档）。
      tabs
        ..assign(MemberTagItemModel(name: '全部关注'))
        ..addAll(
          response.map(
            (e) => MemberTagItemModel(
              count: e.count,
              name: e.name,
              tagid: e.tagid,
              tip: e.tip,
            ),
          ),
        );
      onInitTab();
      followState.value = Success(tabs.hashCode);
    } else {
      followState.value = res;
    }
  }

  void onInitTab() {
    int initialIndex = 0;
    if (tabController != null) {
      initialIndex = tabController!.index.clamp(0, tabs.length - 1);
      tabController!.dispose();
    }
    tabController = TabController(
      initialIndex: initialIndex,
      length: tabs.length,
      vsync: this,
    );
  }

  void onCreateFavTag(({int tagid, String tagName}) res) {
    if (isClosed) return;
    if (followState.value.isSuccess) {
      tabs.add(MemberTagItemModel.fromCreate(res));
      onInitTab();
      followState.refresh();
    } else {
      followState.value = LoadingState.loading();
      queryFollowUpTags();
    }
  }

  Future<void> onUpdateTag(MemberTagItemModel item, String tagName) async {
    final res = await RustRelation.updateFollowTag(item.tagid!, tagName);
    if (res.isSuccess) {
      item.name = tagName;
      tabs.refresh();
      SmartDialog.showToast('修改成功');
    } else {
      res.toast();
    }
  }

  Future<void> onDelTag(int index, int tagid) async {
    final res = await RustRelation.delFollowTag(tagid);
    if (res.isSuccess) {
      tabs.removeAt(index);
      onInitTab();
      followState.refresh();
      SmartDialog.showToast('删除成功');
    } else {
      res.toast();
    }
  }

  @override
  void onClose() {
    tabController?.dispose();
    tabController = null;
    super.onClose();
  }
}
