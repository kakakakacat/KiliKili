import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_msg.dart';
import 'package:get/get.dart';

class LikeDetailController
    extends CommonListController<LikeDetail, LikeDetailItem> {
  late final String cardId;
  late final String? uri;
  late final int counts;

  /// 上一页最后一个人的 mid。按字符串翻页：服务端对它既可能回整数也可能
  /// 回字符串（同类问题见 #166）。
  String lastMid = '0';

  @override
  void onInit() {
    super.onInit();
    final args = Get.arguments;
    cardId = '${args['id']}';
    uri = args['uri'];
    counts = args['counts'];
    queryData();
  }

  LikeDetailCard? card;

  @override
  List<LikeDetailItem>? getDataList(LikeDetail response) {
    card = response.card;
    final items = response.items;
    if (items.lastOrNull?.user?.mid case final mid?) {
      lastMid = '$mid';
    }
    return items;
  }

  @override
  void checkIsEnd(int length) {
    if (length >= counts) {
      isEnd = true;
    }
  }

  @override
  Future<void> onRefresh() {
    lastMid = '0';
    return super.onRefresh();
  }

  @override
  Future<LoadingState<LikeDetail>> customGetData() =>
      RustMsg.msgLikeDetail(cardId: cardId, pn: page, lastMid: lastMid);
}
