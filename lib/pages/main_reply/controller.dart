import 'package:kilikili/grpc/bilibili/main/community/reply/v1.pb.dart'
    show MainListReply;
import 'package:kilikili/grpc/reply.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/reply/fold/reply_fold_mixin.dart';
import 'package:kilikili/pages/common/reply/reply_controller.dart';
import 'package:get/get.dart';

class MainReplyController extends ReplyController<MainListReply>
    with BaseReplyFoldMixin<MainListReply>, ReplyFoldMixin {
  @override
  late final int oid;
  @override
  late final int replyType;

  @override
  int get sourceId => oid;

  @override
  void onInit() {
    super.onInit();
    final args = Get.arguments;
    oid = args['oid'];
    replyType = args['replyType'];

    queryData();
  }

  @override
  Future<LoadingState<MainListReply>> customGetData() => ReplyGrpc.mainList(
    type: replyType,
    oid: oid,
    mode: mode,
    offset: nextOffset,
  );
}
