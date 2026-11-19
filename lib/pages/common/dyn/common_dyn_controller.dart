import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/dynamic/dyn_mention/item.dart';
import 'package:kilikili/pages/common/reply/fold/reply_fold_mixin.dart';
import 'package:kilikili/pages/common/reply/reply_controller.dart';
import 'package:kilikili/pages/common/reply/vote/reply_vote_mixin.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:kilikili/utils/reply_scheme_helper.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:get/get.dart';

mixin DynReplyLocateMixin on MainReplyLocateMixin {
  int? targetSecondRpId; // jump

  @override
  void onAnimRootReply(ReplyEntity rootReply) {
    if (args[secondRpIdKey] case final int secondRpId) {
      args.remove(rootRpIdKey);
      targetRootRpId = rootRpId;
      targetSecondRpId = secondRpId;
    } else {
      super.onAnimRootReply(rootReply);
    }
  }
}

abstract class CommonDynController extends ReplyController<ReplyMainPage>
    with ReplyVoteMixin, BaseReplyFoldMixin<ReplyMainPage>, ReplyFoldMixin {
  CommonDynController({super.count});

  MentionItem? get mentionItem => null;

  late final RxBool showTitle = false.obs;

  late final horizontalPreview = Pref.horizontalPreview;
  late final List<double> ratio = Pref.dynamicDetailRatio;

  late final showDynActionBar = Pref.showDynActionBar;

  @override
  Future<LoadingState<ReplyMainPage>> customGetData() =>
      RustReplyGrpc.mainList(
        type: replyType,
        oid: oid,
        rpid: rootRpId,
        mode: mode,
        offset: nextOffset,
        isRefresh: nextOffset == null,
        intro: intro,
      );
}
