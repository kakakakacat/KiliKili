import 'package:kilikili/http/fav.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/home_tab_type.dart';
import 'package:kilikili/models_new/fav/fav_pgc/list.dart';
import 'package:kilikili/pages/common/common_controller.dart'
    show RefreshImplMixin;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/account_service.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/services/rust/rust_pgc.dart';
import 'package:flutter/widgets.dart' show ScrollController;
import 'package:get/get.dart';

class PgcController
    extends CommonListController<List<PgcIndexItem>?, PgcIndexItem>
    with AccountMixin, RefreshImplMixin {
  PgcController({required this.tabType})
    : indexType = tabType == HomeTabType.cinema ? 102 : null;

  final HomeTabType tabType;
  final int? indexType;

  late final showPgcTimeline =
      tabType == HomeTabType.bangumi && Pref.showPgcTimeline;

  @override
  final accountService = Get.find<AccountService>();

  @override
  void onInit() {
    super.onInit();

    queryData();
    queryPgcFollow();
    if (showPgcTimeline) {
      queryPgcTimeline();
    }
  }

  @override
  Future<void> onRefresh() {
    if (accountService.isLogin.value) {
      _refreshPgcFollow();
    }
    if (showPgcTimeline) {
      queryPgcTimeline();
    }
    return super.onRefresh();
  }

  void _refreshPgcFollow() {
    followPage = 1;
    followEnd = false;
    queryPgcFollow();
  }

  // follow
  late int followPage = 1;
  late RxInt followCount = (-1).obs;
  late bool followLoading = false;
  late bool followEnd = false;
  late Rx<LoadingState<List<FavPgcItemModel>?>> followState =
      LoadingState<List<FavPgcItemModel>?>.loading().obs;
  final followController = ScrollController();

  // timeline
  late Rx<LoadingState<List<TimelineDay>?>> timelineState =
      LoadingState<List<TimelineDay>?>.loading().obs;

  Future<void> queryPgcTimeline() async {
    final res = await Future.wait([
      RustPgc.timeline(types: 1, before: 6, after: 6),
      RustPgc.timeline(types: 4, before: 6, after: 6),
    ]);
    final list1 = res.first.dataOrNull;
    final list2 = res[1].dataOrNull;
    if (list1 != null &&
        list2 != null &&
        list1.isNotEmpty &&
        list2.isNotEmpty) {
      timelineState.value = Success(_mergeTimelines(list1, list2));
      return;
    }
    timelineState.value = Success(list1 ?? list2);
  }

  /// 把番剧与国创两条时间轴按天并成一条。
  ///
  /// 这是**页面**决策——这一页要在同一条时间轴上同时显示两类，而不是
  /// 协议的一部分，所以留在 Dart（ADR-011 决策二）。
  ///
  /// 基线原地改 `list1[i]`；Rust 的 typed 结构字段是 final，这里产出新
  /// 对象。对齐规则沿用基线 `TimelineResult.addAll`：**只在 `dateTs`
  /// 相同时才合并**，否则保留左边那天原样——两次查询的天数理论上一致，
  /// 但真对不上时错位拼接比少几集更糟。
  static List<TimelineDay> _mergeTimelines(
    List<TimelineDay> a,
    List<TimelineDay> b,
  ) => [
    for (var i = 0; i < a.length; i++)
      if (i < b.length && a[i].dateTs != null && a[i].dateTs == b[i].dateTs)
        TimelineDay(
          date: a[i].date,
          dateTs: a[i].dateTs,
          dayOfWeek: a[i].dayOfWeek,
          isToday: a[i].isToday,
          episodes: [...a[i].episodes, ...b[i].episodes],
        )
      else
        a[i],
  ];

  // 我的订阅
  Future<void> queryPgcFollow([bool isRefresh = true]) async {
    if (!accountService.isLogin.value ||
        followLoading ||
        (!isRefresh && followEnd)) {
      return;
    }
    followLoading = true;
    final res = await FavHttp.favPgc(
      type: tabType == HomeTabType.bangumi ? 1 : 2,
      pn: followPage,
    );

    if (res case Success(:final response)) {
      final list = response.list;
      followCount.value = response.total ?? -1;

      if (list == null || list.isEmpty) {
        followEnd = true;
        if (isRefresh) {
          followState.value = Success(list);
        }
        followLoading = false;
        return;
      }

      if (isRefresh) {
        if (list.length >= followCount.value) {
          followEnd = true;
        }
        followState.value = Success(list);
        followController.jumpToTop();
      } else if (followState.value case Success(:final response)) {
        final currentList = response!..addAll(list);
        if (currentList.length >= followCount.value) {
          followEnd = true;
        }
        followState.refresh();
      }
      followPage++;
    } else if (isRefresh) {
      followState.value = res as Error;
    }
    followLoading = false;
  }

  @override
  Future<LoadingState<List<PgcIndexItem>?>> customGetData() => RustPgc.index(
    page: page,
    indexType: indexType,
  );

  @override
  void onClose() {
    followController.dispose();
    super.onClose();
  }

  @override
  void onChangeAccount(bool isLogin) {
    if (isLogin) {
      _refreshPgcFollow();
    } else {
      followState.value = LoadingState.loading();
    }
  }
}
