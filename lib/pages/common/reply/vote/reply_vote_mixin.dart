import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';

mixin ReplyVoteMixin on CommonListController<ReplyMainPage, ReplyEntity> {
  ReplyVoteCard? voteCard;

  @override
  bool customHandleResponse(bool isRefresh, Success<ReplyMainPage> response) {
    if (isRefresh) {
      voteCard = response.response.voteCard ?? voteCard;
    }
    return super.customHandleResponse(isRefresh, response);
  }

  @override
  Future<void> onRefresh() {
    voteCard = null;
    return super.onRefresh();
  }
}
