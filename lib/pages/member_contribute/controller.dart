import 'dart:math';

import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/pages/member/controller.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class MemberContributeCtr extends GetxController
    with GetSingleTickerProviderStateMixin {
  MemberContributeCtr({
    required this.heroTag,
    required this.initialIndex,
  });
  final String? heroTag;
  final int? initialIndex;

  TabController? tabController;
  List<Tab>? tabs;
  late final _ctr = Get.find<MemberController>(tag: heroTag);
  List<SpaceTab2Item>? items;

  @override
  void onInit() {
    super.onInit();
    SpaceTab2 contribute = _ctr.tab2!.firstWhere(
      (item) => item.param == 'contribute',
    );
    // FRB 的列表字段是非空的，但它是解析时现造的那一份，往里 add 不会
    // 影响别的持有者。
    final items = contribute.items;
    if (items.isNotEmpty) {
      this.items = items;
      if (items.length > 1) {
        // show if exist
        if (_ctr.hasSeasonOrSeries == true) {
          items.add(
            const SpaceTab2Item(
              param: 'ugcSeason',
              title: '全部合集/列表',
              filters: [],
            ),
          );
        }
        tabs = items.map((item) => Tab(text: item.title)).toList();
        tabController = TabController(
          vsync: this,
          length: items.length,
          initialIndex: max(0, initialIndex ?? 0),
        );
      }
    }
  }

  @override
  void onClose() {
    tabController?.dispose();
    super.onClose();
  }
}
