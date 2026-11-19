import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/dynamic/dyn_mention/item.dart';
import 'package:kilikili/pages/common/reply/fold/reply_fold_mixin.dart';
import 'package:kilikili/pages/common/reply/reply_controller.dart';
import 'package:kilikili/pages/common/reply/vote/reply_vote_mixin.dart';
import 'package:kilikili/pages/video/controller.dart';
import 'package:kilikili/pages/video/reply_reply/view.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/reply_scheme_helper.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter/widgets.dart' show BoxConstraints, WidgetsBinding;
import 'package:get/get.dart';

class VideoReplyController extends ReplyController<ReplyMainPage>
    with
        ReplyVoteMixin,
        GetSingleTickerProviderStateMixin,
        ReplyLocateMixin,
        MainReplyLocateMixin,
        BaseReplyFoldMixin<ReplyMainPage>,
        ReplyFoldMixin {
  VideoReplyController({
    required this.heroTag,
  });
  late final isPugv = videoCtr.videoType == .pugv;

  @override
  int get oid => isPugv ? videoCtr.epId! : videoCtr.aid;

  @override
  int get replyType => videoCtr.videoType.replyType;

  final String heroTag;
  late final videoCtr = Get.find<VideoDetailController>(tag: heroTag);

  MentionItem? get mentionItem {
    if (videoCtr.isUgc) {
      try {
        return videoCtr.ugcIntroCtr.videoDetail.value.owner?.mentionItem;
      } catch (_) {
        if (kDebugMode) rethrow;
      }
    }
    return null;
  }

  @override
  dynamic get sourceId => IdUtils.av2bv(videoCtr.aid);

  @override
  void onAnimRootReply(ReplyEntity rootReply) {
    if (args[secondRpIdKey] case final int secondRpId) {
      clearTargetRootRpId();
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => replyReply(rootReply, secondRpId),
      );
    } else {
      super.onAnimRootReply(rootReply);
    }
  }

  void replyReply(ReplyEntity replyItem, int? id) {
    videoCtr.childKey.currentState?.showBottomSheet(
      constraints: const BoxConstraints(),
      (context) => VideoReplyReplyPanel(
        id: id,
        oid: replyItem.oid,
        rpid: replyItem.id,
        firstFloor: replyItem.control.isNote ? null : replyItem,
        replyType: videoCtr.videoType.replyType,
        isVideoDetail: true,
        isNested: videoCtr.scrollKey.currentContext != null,
        upMid: upMid,
      ),
    );
  }

  @override
  Future<LoadingState<ReplyMainPage>> customGetData() =>
      RustReplyGrpc.mainList(
        oid: oid,
        rpid: rootRpId,
        type: replyType,
        mode: mode,
        offset: nextOffset,
        isRefresh: nextOffset == null,
        intro: intro,
      );
}
