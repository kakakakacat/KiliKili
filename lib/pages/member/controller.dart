import 'dart:math';

import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/member/tab_type.dart';
import 'package:kilikili/pages/common/common_data_controller.dart';
import 'package:kilikili/pages/member/reserve_item.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/services/rust/rust_video_write.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/extension/nested_scroll_ext.dart';
import 'package:kilikili/utils/request_utils.dart';
import 'package:kilikili/utils/share_utils.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/services/rust/rust_user.dart';
import 'package:extended_nested_scroll_view/extended_nested_scroll_view.dart'
    show ExtendedNestedScrollViewState;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class MemberController extends CommonDataController<SpacePage, SpacePage?>
    with GetTickerProviderStateMixin {
  MemberController({required this.mid});
  int mid;
  String? username;
  String? userAvatar;

  late final account = Accounts.main;

  SpaceLive? live;
  int? silence;

  int? isFollowed; // 被关注
  RxInt relation = 0.obs;
  bool get isFollow {
    final relation = this.relation.value;
    return relation != 0 && relation != 128 && relation != -1;
  }

  SpaceSetting? spaceSetting;
  List<SpaceTab2>? tab2;
  late List<Tab> tabs;
  TabController? tabController;
  RxInt contributeInitialIndex = 0.obs;

  bool? hasSeasonOrSeries;

  List<SpaceSimpleUser>? charges;
  int? chargeCount;
  bool get hasCharge => chargeCount != null && chargeCount! > 0;

  int? medal;
  List<SpaceSimpleUser>? guards;
  /// 人数是从「2人加入大航海」里抠出来的，所以是字符串。
  String? guardCount;
  bool get hasGuard => guards?.isNotEmpty ?? false;

  List<MemberReserveItem>? reserves;

  final fromViewAid = Get.parameters['from_view_aid'];

  final scrollKey = GlobalKey<ExtendedNestedScrollViewState>();

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<SpacePage> response) {
    final data = response.response;
    final card = data.card;
    username = card?.name ?? '';
    userAvatar = card?.face;

    isFollowed = card?.relation?.isFollowed?.toInt();

    // charge
    final elec = data.elec;
    charges = elec?.list;
    chargeCount = elec?.total?.toInt();
    // guard
    final guard = data.guard;
    guards = guard?.items;
    guardCount = guard?.count;
    // medal
    medal = data.medal?.toInt();

    reserves = data.reservations.map(MemberReserveItem.new).toList();

    switch (data.relation?.toInt()) {
      case -1:
        relation.value = 128;
      case -999:
        if (data.guestRelation == -1) {
          relation.value = -1;
        }
      default:
        relation.value = card?.relation?.isFollow == 1
            ? data.relSpecial == 1
                  ? -10
                  : card?.relation?.status?.toInt() ?? 2
            : data.relation?.toInt() ?? 0;
    }
    tab2 = data.tab2.toList();
    live = data.live;
    silence = card?.silence?.toInt();
    hasSeasonOrSeries = data.hasSeasonOrSeries;
    tab2!.retainWhere((item) => MemberTabType.contains(item.param!));
    if (tab2!.isNotEmpty) {
      if (!data.hasItem && tab2!.first.param == 'home') {
        // remove empty home tab
        tab2!.removeAt(0);
      }
      if (tab2!.isNotEmpty) {
        int initialIndex = -1;
        MemberTabType memberTab = Pref.memberTab;
        if (memberTab != MemberTabType.def) {
          initialIndex = tab2!.indexWhere((item) {
            return item.param == memberTab.name;
          });
        }
        if (initialIndex == -1) {
          // 服务端的 `video` 与 tab2 里的 `contribute` 是同一个 tab。
          // 基线在这里就地改写 `data.defaultTab`；Rust 的结构不可变，
          // 改写落到这个局部变量上。
          final defaultTab = data.defaultTab == 'video'
              ? 'contribute'
              : data.defaultTab;
          initialIndex = tab2!.indexWhere((item) {
            return item.param == defaultTab;
          });
        }
        tabs = tab2!
            .map(
              (item) => Tab(
                child: Text(
                  item.title ?? '',
                  softWrap: false,
                  overflow: .fade,
                ),
              ),
            )
            .toList();
        tabController?.dispose();
        tabController = TabController(
          vsync: this,
          length: tabs.length,
          initialIndex: max(0, initialIndex),
        );
      }
    }
    if (mid == account.mid) {
      spaceSetting = data.setting;
    }
    loadingState.value = response;
    return true;
  }

  @override
  bool handleError(String? errMsg) {
    // 取不到空间信息时的兜底 tab。FRB 的列表字段是非空的，所以这里的
    // `items` 要显式给空表，不能像基线那样省掉。
    tab2 = const [
      SpaceTab2(title: '动态', param: 'dynamic', items: []),
      SpaceTab2(
        title: '投稿',
        param: 'contribute',
        items: [SpaceTab2Item(title: '视频', param: 'video', filters: [])],
      ),
      SpaceTab2(title: '收藏', param: 'favorite', items: []),
      SpaceTab2(title: '追番', param: 'bangumi', items: []),
    ];
    tabs = tab2!.map((item) => Tab(text: item.title)).toList();
    tabController?.dispose();
    tabController = TabController(
      vsync: this,
      length: tabs.length,
    );
    username = errMsg;
    loadingState.value = const Success(null);
    return true;
  }

  @override
  Future<LoadingState<SpacePage>> customGetData() => RustSpace.spacePage(
    mid: mid,
    fromViewAid: fromViewAid,
  );

  void blockUser(BuildContext context) {
    if (!account.isLogin) {
      Pref.setBlackMid(mid);
      SmartDialog.showToast('屏蔽成功');
      return;
    }
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('提示'),
        content: Text(relation.value != 128 ? '确定拉黑UP主?' : '从黑名单移除UP主'),
        actions: [
          TextButton(
            onPressed: () {
              Get.back();
              Pref.setBlackMid(mid);
            },
            child: const Text('本地屏蔽'),
          ),
          TextButton(
            onPressed: Get.back,
            child: Text(
              '取消',
              style: TextStyle(color: Theme.of(context).colorScheme.outline),
            ),
          ),
          TextButton(
            onPressed: () {
              Get.back();
              _onBlock();
            },
            child: const Text('确认'),
          ),
        ],
      ),
    );
  }

  void shareUser() {
    ShareUtils.shareText('https://space.bilibili.com/$mid');
  }

  Future<void> _onBlock() async {
    final isBlocked = relation.value == 128;
    final res = await RustVideoWrite.relationMod(
      mid: mid,
      act: isBlocked ? 6 : 5,
      reSrc: 11,
    );
    if (res.isSuccess) {
      relation.value = isBlocked ? 0 : 128;
    }
  }

  void onFollow(BuildContext context) {
    if (mid == account.mid) {
      Get.toNamed('/editProfile');
    } else if (relation.value == 128) {
      _onBlock();
    } else {
      if (!account.isLogin) {
        SmartDialog.showToast('账号未登录');
        return;
      }
      RequestUtils.actionRelationMod(
        context: context,
        mid: mid,
        isFollow: isFollow,
        afterMod: (attribute) => relation.value = attribute,
      );
    }
  }

  @override
  void onClose() {
    tabController?.dispose();
    super.onClose();
  }

  Future<void> onRemoveFan() async {
    final res = await RustVideoWrite.relationMod(mid: mid, act: 7, reSrc: 11);
    if (res.isSuccess) {
      isFollowed = null;
      if (relation.value == 4) {
        relation.value = 2;
      }
      SmartDialog.showToast('移除成功');
    } else {
      res.toast();
    }
  }

  void onTapTab(int value) {
    if (tabController?.indexIsChanging == false) {
      scrollKey.currentState?.animToTop();
    }
  }

  Future<void> vipExpAdd() async {
    final res = await RustUser.vipExpAdd();
    if (res.isSuccess) {
      SmartDialog.showToast('领取成功');
    } else {
      res.toast();
    }
  }
}
