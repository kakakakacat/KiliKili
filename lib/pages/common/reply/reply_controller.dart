import 'package:kilikili/common/widgets/colored_box_transition.dart';
import 'package:kilikili/common/widgets/flutter/text_field/controller.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/reply.dart';
import 'package:kilikili/models/common/reply/reply_sort_type.dart';
import 'package:kilikili/models_new/dynamic/dyn_mention/item.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/common/publish/publish_route.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:kilikili/pages/video/reply_new/view.dart';
import 'package:kilikili/utils/feed_back.dart';
import 'package:kilikili/utils/reply_scheme_helper.dart';
import 'package:kilikili/utils/reply_utils.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:collection/collection.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

mixin MainReplyLocateMixin on ReplyLocateMixin<ReplyMainPage> {
  final Map args = Get.arguments ?? const {};

  int? targetRootRpId; // highlight
  void clearTargetRootRpId() {
    args.remove(rootRpIdKey);
    targetRootRpId = null;
  }

  @override
  void onInit() {
    if (args[rootRpIdKey] case final int rpid) {
      rootRpId = rpid;
    }
    super.onInit();
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<ReplyMainPage> response) {
    final flag = super.customHandleResponse(isRefresh, response);
    if (isRefresh && rootRpId != null && args.containsKey(rootRpIdKey)) {
      final replies = response.response.replies;
      final rootReply = replies.firstWhereOrNull((e) => e.id == rootRpId);
      if (rootReply != null) {
        onAnimRootReply(rootReply);
      }
    }
    return flag;
  }

  void onAnimRootReply(ReplyEntity rootReply) {
    targetRootRpId = rootRpId;
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => colorAnimController
          .forward(from: 0)
          .whenComplete(clearTargetRootRpId),
    );
  }
}

mixin ReplyLocateMixin<T>
    on ReplyController<T>, GetSingleTickerProviderStateMixin {
  AnimationController? _animController;
  bool get hasAnimController => _animController != null;
  AnimationController get colorAnimController =>
      _animController ??= AnimationController(
        duration: const Duration(milliseconds: 1000),
        vsync: this,
      );

  Animation<Color?>? _colorAnim;
  Animation<Color?> colorAnim(ColorScheme colorScheme) =>
      _colorAnim ??= colorAnimController.drive(
        ColorTween(
          begin: colorScheme.onInverseSurface,
          end: colorScheme.surface,
        ).chain(CurveTween(curve: const Interval(0.8, 1.0))),
      );

  Widget colorAnimWrapper(ColorScheme colorScheme, Widget child) =>
      ColoredBoxTransition(color: colorAnim(colorScheme), child: child);

  @override
  void didChangeDependencies(BuildContext context) {
    super.didChangeDependencies(context);
    _colorAnim = null;
  }

  @override
  void onClose() {
    _colorAnim = null;
    _animController?.dispose();
    _animController = null;
    super.onClose();
  }
}

abstract class ReplyController<R> extends CommonListController<R, ReplyEntity> {
  ReplyController({int count = -1}) : count = RxInt(count);

  int get oid;
  int get replyType;

  int? rootRpId;
  late final RxInt count;

  late final Rx<ReplySortType> sortType;
  late ReplySortMode mode;

  final savedReplies = <Object, List<RichTextItem>?>{};

  int? upMid;
  String? nextOffset;
  ReplySubject? subjectControl;
  late bool hasUpTop = false;

  @override
  bool get hasFooter => true;

  // comment antifraud
  late final _enableCommAntifraud = Pref.enableCommAntifraud;
  late final _biliSendCommAntifraud = Pref.biliSendCommAntifraud;
  bool get enableCommAntifraud =>
      _enableCommAntifraud || _biliSendCommAntifraud;
  dynamic get sourceId;

  ReplySortType get prefSortType => Pref.replySortType;

  @override
  void onInit() {
    super.onInit();
    final cacheSortType = prefSortType;
    sortType = cacheSortType.obs;
    mode = cacheSortType == .time ? ReplySortMode.time : ReplySortMode.hot;
  }

  @override
  bool get endIfNullOrEmpty => false;

  @override
  void checkIsEnd(int length) {
    final count = this.count.value;
    if (count != -1 && length >= count) {
      isEnd = true;
    }
  }

  /// 稿件简介。设置里开了「过滤简介复读」时，与它逐字相同的根评论会被
  /// 移除——判定在 Rust（`ReplyFilter::removes_as_intro`），这里只负责把
  /// 当前稿件的简介随请求递过去。
  String? intro;

  @override
  bool customHandleResponse(bool isRefresh, Success<R> response) {
    final data = response.response as ReplyMainPage;
    count.value = data.subject.count;
    if (isRefresh) {
      handleSubject(data.subject);
      hasUpTop = data.hasUpTop;
      if (subjectControl?.title == ReplySortType.select.desc) {
        sortType.value = .select;
      }
    }

    handlePagination(isEnd: data.isEnd, offset: data.nextOffset);

    return false;
  }

  /// UP 置顶的并入、简介复读的剔除、「同一个 offset 翻两次」的判定都已经
  /// 在 Rust 做掉了（见 `bili_api::grpc_reply`）。这里只接住结果。
  void handlePagination({required bool isEnd, required String offset}) {
    if (isEnd) this.isEnd = true;
    nextOffset = offset;
  }

  void handleSubject(ReplySubject subject) {
    subjectControl = subject;
    upMid = subject.upMid;
  }

  @override
  Future<void> onRefresh() {
    nextOffset = null;
    subjectControl = null;
    return super.onRefresh();
  }

  // 排序搜索评论
  void queryBySort() {
    if (isLoading) return;
    switch (sortType.value) {
      case ReplySortType.time:
        sortType.value = ReplySortType.hot;
        mode = ReplySortMode.hot;
        break;
      case ReplySortType.hot:
        sortType.value = ReplySortType.time;
        mode = ReplySortMode.time;
        break;
      case ReplySortType.select:
        return;
    }
    feedBack();
    onReload();
  }

  (bool inputDisable, String? hint) get replyHint {
    String? hint;
    bool inputDisable = false;
    try {
      if (subjectControl case final subjectControl?) {
        inputDisable = subjectControl.inputDisable;
        if (subjectControl.rootText case final rootText?) {
          if (inputDisable) {
            SmartDialog.showToast(rootText);
          }
          if (rootText.contains('可发') || rootText.contains('可见')) {
            hint = rootText;
          }
        }
      }
    } catch (_) {}
    return (inputDisable, hint);
  }

  void onReply(
    ReplyEntity? replyItem, {
    int? oid,
    int? replyType,
    MentionItem? mentionItem,
  }) {
    if (loadingState.value case Error(:final errMsg, :final code)) {
      if (errMsg != null && (code == 12061 || code == 12002)) {
        SmartDialog.showToast(errMsg);
        return;
      }
    }

    assert(replyItem != null || (oid != null && replyType != null));

    final (bool inputDisable, String? hint) = replyHint;
    if (inputDisable) {
      return;
    }

    final key = oid ?? replyItem!.oid + replyItem.id;
    Get.key.currentState!
        .push(
          PublishRoute(
            pageBuilder: (buildContext, animation, secondaryAnimation) {
              return ReplyPage(
                hint: hint,
                oid: oid ?? replyItem!.oid,
                root: oid != null ? 0 : replyItem!.id,
                parent: oid != null ? 0 : replyItem!.id,
                replyType: replyItem?.kind ?? replyType!,
                replyItem: replyItem,
                mentionItem: mentionItem,
                items: savedReplies[key],

                /// hd api deprecated
                // canUploadPic: canUploadPic,
                onSave: (reply) {
                  if (reply.isEmpty) {
                    savedReplies.remove(key);
                  } else {
                    savedReplies[key] = reply.toList();
                  }
                },
              );
            },
            settings: RouteSettings(arguments: Get.arguments),
          ),
        )
        .then(
          (replyInfo) {
            if (replyInfo is ReplyEntity) {
              savedReplies.remove(key);
              if (loadingState.value case Success(:final response)) {
                if (response == null) {
                  loadingState.value = Success([replyInfo]);
                } else {
                  if (oid != null) {
                    response.insert(hasUpTop ? 1 : 0, replyInfo);
                  } else {
                    replyItem!
                      ..count += 1
                      ..replies.add(replyInfo);
                  }
                  loadingState.refresh();
                }
              } else {
                loadingState.value = Success([replyInfo]);
              }
              count.value += 1;

              // check reply
              if (enableCommAntifraud) {
                onCheckReply(replyInfo, isManual: false);
              }
            }
          },
        );
  }

  void onRemove(int index, ReplyEntity item, int? subIndex) {
    if (subIndex == null) {
      loadingState.value.data!.removeAt(index);
    } else {
      item
        ..count -= 1
        ..replies.removeAt(subIndex);
    }
    count.value -= 1;
    loadingState.refresh();
  }

  void onCheckReply(ReplyEntity replyInfo, {bool isManual = true}) {
    ReplyUtils.onCheckReply(
      replyInfo: replyInfo,
      biliSendCommAntifraud: _biliSendCommAntifraud,
      sourceId: sourceId,
      isManual: isManual,
    );
  }

  Future<void> onToggleTop(
    ReplyEntity item,
    int index,
    oid,
    int type,
  ) async {
    bool isUpTop = item.control.isUpTop;
    final res = await ReplyHttp.replyTop(
      oid: oid,
      type: type,
      rpid: item.id,
      isUpTop: isUpTop,
    );
    if (res.isSuccess) {
      item.control.isUpTop = !isUpTop;
      if (!isUpTop && index != 0) {
        final list = loadingState.value.data!;
        list
          ..first.control.isUpTop = false
          ..insert(0, list.removeAt(index));
      }
      loadingState.refresh();
      SmartDialog.showToast('${isUpTop ? '取消' : ''}置顶成功');
    } else {
      res.toast();
    }
  }

  @override
  void onClose() {
    savedReplies.clear();
    super.onClose();
  }
}
