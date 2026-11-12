import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_pgc.dart';
import 'package:kilikili/models/common/pgc_review_type.dart';
import 'package:kilikili/models_new/pgc/pgc_review/data.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';

class PgcReviewController
    extends CommonListController<PgcReviewData, PgcReviewItem> {
  PgcReviewController({required this.type, required this.mediaId});

  final PgcReviewType type;
  final dynamic mediaId;

  final count = RxnInt();
  String? next;
  final sortType = PgcReviewSortType.def.obs;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<void> onRefresh() {
    next = null;
    return super.onRefresh();
  }

  @override
  void checkIsEnd(int length) {
    final count = this.count.value;
    if (count != null && length >= count) {
      isEnd = true;
    }
  }

  @override
  List<PgcReviewItem>? getDataList(PgcReviewData response) {
    if (type == PgcReviewType.long &&
        sortType.value == PgcReviewSortType.latest) {
      count.value = null;
    } else {
      count.value = response.count;
    }
    next = response.next;

    return response.list;
  }

  @override
  Future<LoadingState<PgcReviewData>> customGetData() => RustPgc.review(
    type: type,
    mediaId: mediaId,
    next: next,
    sort: sortType.value.sort,
  );

  /// 点赞/点踩之后更新本地计数。
  ///
  /// 基线是原地改 `item.stat`。Rust 的 typed 结构字段是 final（FRB 生成的
  /// 类不可变），所以这里换成**替换列表里那一条**。
  ///
  /// 这不是将就。点完赞之后的计数是**客户端状态**——服务端这次调用并不
  /// 回传新的列表，基线原地改一个解析出来的对象，等于把"服务端说的"和
  /// "我刚做的"混在同一个字段里。替换之后两者仍然分得清：列表里那条是
  /// 当前展示值，下次刷新会被服务端的值盖掉。
  void _replaceStat(PgcReviewItem item, PgcReviewStat stat) {
    if (loadingState.value case Success(:final response)) {
      final list = response;
      if (list == null) return;
      final index = list.indexOf(item);
      if (index < 0) return;
      list[index] = PgcReviewItem(
        articleId: item.articleId,
        author: item.author,
        title: item.title,
        content: item.content,
        pushTimeStr: item.pushTimeStr,
        reviewId: item.reviewId,
        score: item.score,
        stat: stat,
      );
      loadingState.refresh();
    }
  }

  Future<void> onLike(PgcReviewItem item, bool isLike, reviewId) async {
    final res = await RustPgc.reviewLike(
      mediaId: mediaId,
      reviewId: reviewId,
    );
    if (res.isSuccess) {
      final likes = item.stat?.likes ?? 0;
      _replaceStat(
        item,
        PgcReviewStat(
          liked: isLike ? 0 : 1,
          likes: isLike ? likes - 1 : likes + 1,
          // 点赞成功即取消点踩；取消点赞时保持原值。
          disliked: isLike ? item.stat?.disliked : 0,
        ),
      );
    } else {
      res.toast();
    }
  }

  Future<void> onDislike(
    PgcReviewItem item,
    bool isDislike,
    reviewId,
  ) async {
    final res = await RustPgc.reviewDislike(
      mediaId: mediaId,
      reviewId: reviewId,
    );
    if (res.isSuccess) {
      final wasLiked = item.stat?.liked == 1;
      final likes = item.stat?.likes;
      _replaceStat(
        item,
        PgcReviewStat(
          disliked: isDislike ? 0 : 1,
          // 点踩成功即取消点赞，并把赞数减回去；取消点踩时保持原值。
          liked: isDislike ? item.stat?.liked : 0,
          likes: !isDislike && wasLiked && likes != null ? likes - 1 : likes,
        ),
      );
    } else {
      res.toast();
    }
  }

  Future<void> onDel(int index, int reviewId) async {
    final res = await RustPgc.reviewDel(
      mediaId: mediaId,
      reviewId: reviewId,
    );
    if (res.isSuccess) {
      loadingState
        ..value.data!.removeAt(index)
        ..refresh();
      SmartDialog.showToast('删除成功');
    } else {
      res.toast();
    }
  }

  void queryBySort() {
    if (isLoading) return;
    sortType.value = sortType.value == PgcReviewSortType.def
        ? PgcReviewSortType.latest
        : PgcReviewSortType.def;
    onReload();
  }
}
