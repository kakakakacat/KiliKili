import 'dart:async';

import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/fav_order_type.dart';
import 'package:kilikili/models/common/video/source_type.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/pages/common/page_order/page_order_mixin.dart';
import 'package:kilikili/pages/fav_sort/view.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_key.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:flutter/widgets.dart' show Text, ValueChanged;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';

/// 收藏夹详情与收藏夹内搜索共用的那一半（S8 / issue #58）。
///
/// 选中态改用 [IdMultiSelectMixin]：列表项来自 Rust，不可就地改。按
/// `id` 记——同一个收藏夹里一个资源只会有一条。
mixin BaseFavController
    on
        CommonListController<FavDetail, FavMedia>,
        IdMultiSelectMixin<FavDetail, FavMedia, int> {
  bool get isOwner;
  int get mediaId;

  ValueChanged<int>? updateCount;

  void onViewFav(FavMedia item, int? index);

  Future<void> onCancelFav(int index, int id, int type) async {
    final res = await RustFav.favVideo(
      resources: '$id:$type',
      delIds: mediaId.toString(),
    );
    if (res.isSuccess) {
      loadingState
        ..value.data!.removeAt(index)
        ..refresh();
      updateCount?.call(1);
      SmartDialog.showToast('取消收藏');
    } else {
      res.toast();
    }
  }

  @override
  void onRemove() {
    showConfirmDialog(
      context: Get.context!,
      title: const Text('提示'),
      content: const Text('确认删除所选收藏吗？'),
      onConfirm: () async {
        final removeList = allChecked.toList();
        if (removeList.isEmpty) return;
        final res = await RustFav.favVideo(
          resources: removeList
              .map((item) => '${item.id}:${item.kind}')
              .join(','),
          delIds: mediaId.toString(),
        );
        if (res.isSuccess) {
          updateCount?.call(removeList.length);
          afterDeleteIds(removeList.map(idOf).toSet());
          SmartDialog.showToast('取消收藏');
        } else {
          res.toast();
        }
      },
    );
  }
}

class FavDetailController extends CommonListController<FavDetail, FavMedia>
    with
        IdMultiSelectMixin<FavDetail, FavMedia, int>,
        BaseFavController,
        PageOrderMixin {
  @override
  late int mediaId;
  late String heroTag;

  /// 夹子本身的信息。**不可变**——它是 Rust 的取数结果。
  ///
  /// 会被本地改动的那两样（内容数、是否已收藏这个夹子）单列成下面两个
  /// Rx：写操作成功后要把界面上的数字跟着动一下，而往取数结果里塞一个
  /// 本地值会让「这是服务端说的」和「这是我们刚算的」混在一起。
  final Rxn<FavFolder> folderInfo = Rxn<FavFolder>();

  /// 内容数。随取消收藏/批量删除递减。
  final RxInt mediaCount = 0.obs;

  /// 是否已收藏这个收藏夹（订阅别人的夹子）。`1` 为已收藏。
  final RxInt favState = 0.obs;

  final RxBool _isOwner = false.obs;
  final Rx<FavOrderType> order = FavOrderType.mtime.obs;

  @override
  final RxBool enableMultiSelect = false.obs;

  @override
  final RxInt rxCount = 0.obs;

  @override
  int idOf(FavMedia item) => item.id ?? -identityHashCode(item);

  @override
  bool get isOwner => _isOwner.value;

  late final account = Accounts.main;

  late double dx = 0;
  late final RxBool isPlayAll = Pref.enablePlayAll.obs;

  void setIsPlayAll(bool isPlayAll) {
    if (this.isPlayAll.value == isPlayAll) return;
    this.isPlayAll.value = isPlayAll;
    GStorage.setting.put(SettingBoxKey.enablePlayAll, isPlayAll);
  }

  @override
  int get count => mediaCount.value;

  @override
  int get ps => _ps;

  static const _ps = 20;

  @override
  void onInit() {
    super.onInit();

    mediaId = int.parse(Get.parameters['mediaId']!);
    heroTag = Get.parameters['heroTag']!;

    queryData();
  }

  @override
  bool get hasFooter => true;

  @override
  List<FavMedia>? getDataList(FavDetail response) {
    if (pageDesc) {
      if (page == 1) {
        isEnd = true;
      }
    } else if (response.hasMore == false) {
      isEnd = true;
    }
    return response.medias;
  }

  @override
  void checkIsEnd(int length) {
    if (length >= mediaCount.value) {
      isEnd = true;
    }
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<FavDetail> response) {
    if (isRefresh) {
      final info = response.response.info;
      if (info != null) setFolderInfo(info);
      _isOwner.value = info?.mid == account.mid;
    }
    return false;
  }

  /// 换一份夹子信息（首屏、以及编辑收藏夹回来之后），并重置那两个本地值。
  void setFolderInfo(FavFolder info) {
    folderInfo.value = info;
    mediaCount.value = info.mediaCount;
    favState.value = info.favState ?? 0;
  }

  @override
  ValueChanged<int>? get updateCount =>
      (count) => mediaCount.value -= count;

  @override
  Future<LoadingState<FavDetail>> customGetData() => RustFav.favDetail(
    pn: page,
    ps: _ps,
    mediaId: mediaId,
    order: order.value,
    pageDesc: pageDesc,
  );

  void toViewPlayAll() {
    if (loadingState.value case Success(:final response)) {
      if (response == null || response.isEmpty) return;

      for (final element in response) {
        if (element.ugcFirstCid == null) {
          continue;
        } else {
          onViewFav(element, null);
          break;
        }
      }
    }
  }

  @override
  Future<void> onReload() {
    scrollController.jumpToTop();
    return super.onReload();
  }

  Future<void> onFav(bool isFav) async {
    if (!account.isLogin) {
      SmartDialog.showToast('账号未登录');
      return;
    }
    // isFav 是**当前**状态，目标是它的反面。
    final res = await RustFav.favFavFolder(mediaId: mediaId, fav: !isFav);

    if (res.isSuccess) {
      favState.value = isFav ? 0 : 1;
      SmartDialog.showToast('${isFav ? '取消' : ''}收藏成功');
    } else {
      res.toast();
    }
  }

  Future<void> cleanFav() async {
    final res = await RustFav.cleanFav(mediaId: mediaId);
    if (res.isSuccess) {
      SmartDialog.showToast('清除成功');
      Timer(const Duration(milliseconds: 200), onReload);
    } else {
      res.toast();
    }
  }

  void onSort() {
    if (loadingState.value case Success(:final response)) {
      if (response != null && response.isNotEmpty) {
        if (mediaCount.value > 1000) {
          SmartDialog.showToast('内容太多啦！超过1000不支持排序');
          return;
        }
        Get.to(FavSortPage(favDetailController: this));
      }
    }
  }

  @override
  void onViewFav(FavMedia item, int? index) {
    final folder = folderInfo.value;
    // TODO: dimension
    PageUtils.toVideoPage(
      bvid: item.bvid,
      cid: item.ugcFirstCid!,
      cover: item.cover,
      title: item.title,
      extraArguments: isPlayAll.value
          ? {
              'sourceType': SourceType.fav,
              'mediaId': folder?.id,
              'oid': item.id,
              'favTitle': folder?.title,
              'count': mediaCount.value,
              'desc': !pageDesc,
              if (index != null) 'isContinuePlaying': index != 0,
              'isOwner': isOwner,
            }
          : null,
    );
  }
}
