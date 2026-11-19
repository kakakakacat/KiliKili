import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/reply/reply_search_type.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/video/reply_search_item/controller.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';

class ReplySearchChildController
    extends CommonListController<ReplySearchList, ReplySearchItem> {
  ReplySearchChildController(this.controller, this.searchType);

  final ReplySearchController controller;
  final ReplySearchType searchType;

  @override
  List<ReplySearchItem>? getDataList(ReplySearchList response) {
    if (!response.hasNext) {
      isEnd = true;
    }
    return response.items;
  }

  @override
  Future<LoadingState<ReplySearchList>> customGetData() {
    return RustReplyGrpc.searchItem(
      page: page,
      itemKind: searchType == ReplySearchType.video
          ? ReplySearchItemKind.video
          : ReplySearchItemKind.article,
      oid: controller.oid,
      type: controller.type,
      keyword: controller.editingController.text,
    );
  }
}
