import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/reply/fold/reply_fold_mixin.dart';
import 'package:kilikili/pages/common/reply/reply_controller.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:get/get.dart';

class MainReplyController extends ReplyController<ReplyMainPage>
    with BaseReplyFoldMixin<ReplyMainPage>, ReplyFoldMixin {
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
  Future<LoadingState<ReplyMainPage>> customGetData() =>
      RustReplyGrpc.mainList(
        type: replyType,
        oid: oid,
        mode: mode,
        offset: nextOffset,
        isRefresh: nextOffset == null,
        intro: intro,
      );
}
