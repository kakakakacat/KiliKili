import 'package:kilikili/http/dynamics.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/dynamic/dyn_topic_feed/item.dart';
import 'package:kilikili/models_new/dynamic/dyn_topic_feed/topic_card_list.dart';
import 'package:kilikili/models_new/dynamic/dyn_topic_feed/topic_sort_by_conf.dart';
import 'package:kilikili/rust/api/dynamic.dart' as rust;
import 'package:kilikili/services/rust/rust_dynamic.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';

class DynTopicController
    extends CommonListController<TopicCardList?, TopicCardItem> {
  final topicId = Get.parameters['id']!;
  String topicName = Get.parameters['name'] ?? '';

  int sortBy = 0;
  String? offset;
  final topicSortByConf = Rxn<TopicSortByConf>();

  double? appbarOffset;

  // top
  final isFav = false.obs;
  final isLike = false.obs;

  /// 收藏数与点赞数**单独放在这里**，不跟着 [topState] 走。
  ///
  /// 基线是直接改 `topicItem.fav` 这个可变字段；FRB 的导出结构不可变
  /// （ADR-011 补充七），照抄会在编译期就过不去。而这两个数在按下按钮后
  /// 的本地加减**本来就是界面状态**——它是乐观更新，服务端下一次返回才是
  /// 真值，放在宿主侧正好（AGENTS.md §2）。
  final favCount = 0.obs;
  final likeCount = 0.obs;

  Rx<LoadingState<rust.TopicTopData>> topState =
      LoadingState<rust.TopicTopData>.loading().obs;

  late final isLogin = Accounts.main.isLogin;

  @override
  void onInit() {
    super.onInit();
    queryTop();
    queryData();
  }

  Future<void> queryTop() async {
    topState.value = await RustDynamic.topicTop(topicId: topicId.toString());
    if (topState.value case Success(:final response)) {
      // topic 为空表示服务端没给顶部信息，不是错误（见 Rust 侧注释）。
      if (response.topic case final topicItem?) {
        topicName = topicItem.name;
        isFav.value = topicItem.isFav ?? false;
        isLike.value = topicItem.isLike ?? false;
        favCount.value = topicItem.fav.toInt();
        likeCount.value = topicItem.like.toInt();
      }
    }
  }

  @override
  List<TopicCardItem>? getDataList(TopicCardList? response) {
    if (response != null) {
      offset = response.offset;
      topicSortByConf.value = response.topicSortByConf;
      sortBy = response.topicSortByConf?.showSortBy ?? 0;
      if (response.hasMore == false) {
        isEnd = true;
      }
      return response.items;
    }
    return null;
  }

  @override
  Future<void> onRefresh() {
    offset = '';
    queryTop();
    return super.onRefresh();
  }

  @override
  Future<void> onReload() {
    if (appbarOffset != null) {
      if (scrollController.hasClients &&
          scrollController.offset > appbarOffset!) {
        scrollController.jumpTo(appbarOffset!);
      }
    } else {
      scrollController.jumpToTop();
    }
    return super.onReload();
  }

  @override
  Future<LoadingState<TopicCardList?>> customGetData() =>
      DynamicsHttp.topicFeed(
        topicId: topicId,
        offset: offset,
        sortBy: sortBy,
      );

  void onSort(int sortBy) {
    this.sortBy = sortBy;
    onReload();
  }

  Future<void> onFav() async {
    if (!isLogin) {
      SmartDialog.showToast('账号未登录');
      return;
    }
    final isFav = this.isFav.value;
    // 传目标状态：isFav 是当前状态，目标是它的反面。
    final res = await RustFav.favTopicWrite(topicId: topicId, fav: !isFav);
    if (res.isSuccess) {
      favCount.value += isFav ? -1 : 1;
      this.isFav.toggle();
    } else {
      res.toast();
    }
  }

  Future<void> onLike() async {
    if (!isLogin) {
      SmartDialog.showToast('账号未登录');
      return;
    }
    final isLike = this.isLike.value;
    final res = await RustFav.likeTopic(topicId: topicId, like: !isLike);
    if (res.isSuccess) {
      likeCount.value += isLike ? -1 : 1;
      this.isLike.toggle();
    } else {
      res.toast();
    }
  }

  Future<void> topicFold() async {
    final res = await DynamicsHttp.topicFold(topicId: topicId, sortBy: sortBy);
    if (res case Success(:final response)) {
      if (response?.items case final items? when items.isNotEmpty) {
        loadingState.value.data!
          ..removeLast()
          ..addAll(items);
        loadingState.refresh();
      }
    }
  }
}
