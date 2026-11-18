import 'dart:async' show FutureOr, Timer;

import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/models/common/video/source_type.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/models_new/video/video_detail/data.dart';
import 'package:kilikili/models_new/video/video_detail/episode.dart';
import 'package:kilikili/models_new/video/video_detail/stat_detail.dart';
import 'package:kilikili/pages/video/controller.dart';
import 'package:kilikili/pages/video/introduction/ugc/widgets/triple_mixin.dart';
import 'package:kilikili/services/rust/rust_video_page.dart';
import 'package:kilikili/services/rust/rust_video_write.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/global_data.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_key.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/services/rust/rust_user.dart';
import 'package:collection/collection.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

abstract class CommonIntroController extends GetxController
    with GetSingleTickerProviderStateMixin, TripleMixin, FavMixin {
  late final String heroTag;

  int get aid => videoDetailCtr.aid;
  String get bvid => videoDetailCtr.bvid;
  RxInt get cid => videoDetailCtr.cid;
  late final videoDetailCtr = Get.find<VideoDetailController>(tag: heroTag);

  // 是否稍后再看
  late final RxBool hasLater;

  final Rx<List<VideoTag>?> videoTags = Rx<List<VideoTag>?>(null);

  bool isProcessing = false;
  Future<void> handleAction(FutureOr Function() action) async {
    if (!isProcessing) {
      isProcessing = true;
      await action();
      isProcessing = false;
    }
  }

  @override
  late final isLogin = Accounts.main.isLogin;

  StatDetail? getStat();

  @override
  void updateFavCount(int count) {
    getStat()?.favorite += count;
  }

  final Rx<VideoDetailData> videoDetail = VideoDetailData().obs;

  void queryVideoIntro();

  bool prevPlay();
  bool nextPlay();

  void actionShareVideo(BuildContext context);

  void onChangeEpisode(BaseEpisodeItem episode);

  // 同时观看
  final bool isShowOnlineTotal = Pref.enableOnlineTotal;
  late final RxString total = '1'.obs;
  Timer? timer;

  @override
  void onInit() {
    super.onInit();
    final args = Get.arguments;
    heroTag = args['heroTag'];
    hasLater = RxBool(
      args['viewLater'] ?? args['sourceType'] == SourceType.watchLater,
    );

    queryVideoIntro();
    startTimer();
  }

  void startTimer() {
    if (isShowOnlineTotal) {
      queryOnlineTotal();
      timer ??= Timer.periodic(const Duration(seconds: 10), (Timer timer) {
        queryOnlineTotal();
      });
    }
  }

  void cancelTimer() {
    timer?.cancel();
    timer = null;
  }

  // 查看同时在看人数
  Future<void> queryOnlineTotal() async {
    if (!isShowOnlineTotal) {
      return;
    }
    final result = await RustVideoPage.onlineTotal(
      aid: aid,
      bvid: bvid,
      cid: cid.value,
    );
    if (result case Success(:final response)) {
      total.value = response;
    }
  }

  @override
  void onClose() {
    cancelTimer();
    super.onClose();
  }

  @override
  Future<void> onPayCoin(int coin, bool coinWithLike) async {
    final stat = getStat();
    if (stat == null) {
      return;
    }
    final res = await RustVideoWrite.coinVideo(
      bvid: bvid,
      multiply: coin,
      selectLike: coinWithLike ? 1 : 0,
    );
    if (res.isSuccess) {
      SmartDialog.showToast('投币成功');
      coinNum.value += coin;
      GlobalData().afterCoin(coin);
      stat.coin += coin;
      if (coinWithLike && !hasLike.value) {
        stat.like++;
        hasLike.value = true;
      }
    } else {
      res.toast();
    }
  }

  Future<void> queryVideoTags() async {
    final result = await RustUser.videoTags(bvid: bvid, cid: cid.value);
    videoTags.value = result.dataOrNull;
  }

  Future<void> viewLater() async {
    final res = await (hasLater.value
        ? RustHistory.toViewDel(aids: '$aid')
        : RustHistory.toViewLater(bvid: bvid));
    if (res.isSuccess) hasLater.toggle();
  }
}

mixin FavMixin on TripleMixin {
  /// 服务端说的「这个资源已经在哪几个夹子里」。`null` 表示还没查过。
  Set<int>? favIds;

  /// 收藏面板上**当前的勾选**。初值就是 [favIds]。
  ///
  /// 基线把勾选态直接写回列表项的 `favState`，列表项迁到 Rust 之后字段是
  /// final 的——而勾选本来也不是协议的一部分，它是界面状态（AGENTS.md §2）。
  final Set<int> selectedFavIds = <int>{};

  /// 面板上新建出来的收藏夹 id。
  ///
  /// 它们的 `mediaCount` 来自新建时的响应（0），而界面要显示「1 个内容」
  /// ——那个 1 是本次勾选带来的，和已有夹子的 +1 是同一回事，所以统一由
  /// [favFolderDisplayCount] 算，不单独记。
  int? quickFavId;
  late final enableQuickFav = Pref.enableQuickFav;
  final Rxn<FavFolderList> favFolderData = Rxn<FavFolderList>();

  (Object, int) get getFavRidType;

  Future<LoadingState<FavFolderList>> queryVideoInFolder() async {
    favIds = null;
    final (rid, type) = getFavRidType;
    final res = await RustFav.videoInFolder(
      mid: Accounts.main.mid,
      rid: rid,
      type: type,
    );
    if (res case Success(:final response)) {
      favFolderData.value = response;
      favIds = response.list
          .where((item) => item.favState == 1)
          .map((item) => item.id)
          .toSet();
      selectedFavIds
        ..clear()
        ..addAll(favIds!);
    }
    return res;
  }

  /// 面板上该显示的内容数：服务端给的那个，加上本次勾选的增减。
  int favFolderDisplayCount(FavFolder item) {
    final wasFaved = favIds?.contains(item.id) == true;
    final isSelected = selectedFavIds.contains(item.id);
    if (isSelected == wasFaved) return item.mediaCount;
    return item.mediaCount + (isSelected ? 1 : -1);
  }

  int get favFolderId {
    if (this.quickFavId != null) {
      return this.quickFavId!;
    }
    final quickFavId = Pref.quickFavId;
    final list = favFolderData.value!.list;
    if (quickFavId != null) {
      final folderInfo = list.firstWhereOrNull((e) => e.id == quickFavId);
      if (folderInfo != null) {
        return this.quickFavId = quickFavId;
      } else {
        GStorage.setting.delete(SettingBoxKey.quickFavId);
      }
    }
    return this.quickFavId = list.first.id;
  }

  // 收藏
  void showFavBottomSheet(BuildContext context, {bool isLongPress = false}) {
    if (!Accounts.main.isLogin) {
      SmartDialog.showToast('账号未登录');
      return;
    }
    // 快速收藏 &
    // 点按 收藏至默认文件夹
    // 长按选择文件夹
    if (enableQuickFav) {
      if (!isLongPress) {
        actionFavVideo(isQuick: true);
      } else {
        PageUtils.showFavBottomSheet(context: context, ctr: this);
      }
    } else if (!isLongPress) {
      PageUtils.showFavBottomSheet(context: context, ctr: this);
    }
  }

  void updateFavCount(int count);

  Future<void> actionFavVideo({bool isQuick = false}) async {
    final (rid, type) = getFavRidType;
    // 收藏至默认文件夹
    if (isQuick) {
      SmartDialog.showLoading(msg: '请求中');
      queryVideoInFolder().then((res) async {
        if (res.isSuccess) {
          final hasFav = this.hasFav.value;
          final result = hasFav
              ? await RustFav.unfavAll(rid: rid, type: type)
              : await RustFav.favVideo(
                  resources: '$rid:$type',
                  addIds: favFolderId.toString(),
                );
          SmartDialog.dismiss();
          if (result.isSuccess) {
            updateFavCount(hasFav ? -1 : 1);
            this.hasFav.toggle();
            SmartDialog.showToast('${hasFav ? '取消' : ''}收藏成功');
          } else {
            result.toast();
          }
        } else {
          SmartDialog.dismiss();
        }
      });
      return;
    }

    final List<int> addMediaIdsNew = [];
    final List<int> delMediaIdsNew = [];
    try {
      for (final i in favFolderData.value!.list) {
        final isFaved = favIds?.contains(i.id) == true;
        final isSelected = selectedFavIds.contains(i.id);
        if (isSelected && !isFaved) {
          addMediaIdsNew.add(i.id);
        } else if (!isSelected && isFaved) {
          delMediaIdsNew.add(i.id);
        }
      }
    } catch (e) {
      if (kDebugMode) debugPrint(e.toString());
    }
    SmartDialog.showLoading(msg: '请求中');
    final result = await RustFav.favVideo(
      resources: '$rid:$type',
      addIds: addMediaIdsNew.join(','),
      delIds: delMediaIdsNew.join(','),
    );
    SmartDialog.dismiss();
    if (result.isSuccess) {
      Get.back();
      final newVal =
          addMediaIdsNew.isNotEmpty || favIds?.length != delMediaIdsNew.length;
      if (hasFav.value != newVal) {
        updateFavCount(newVal ? 1 : -1);
        hasFav.value = newVal;
      }
      SmartDialog.showToast('${newVal ? '' : '取消'}收藏成功');
    } else {
      result.toast();
    }
  }
}
