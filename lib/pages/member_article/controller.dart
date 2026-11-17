import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/http/loading_state.dart';

import 'package:kilikili/pages/common/common_list_controller.dart';

class MemberArticleCtr
    extends CommonListController<SpaceArticlePage, SpaceArticleItem> {
  MemberArticleCtr({
    required this.mid,
  });

  final int mid;

  int? count;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<SpaceArticleItem>? getDataList(SpaceArticlePage response) {
    count = response.count;
    return response.items;
  }

  @override
  void checkIsEnd(int length) {
    if (spaceArticleIsEnd(loadedCount: length, totalCount: count)) {
      isEnd = true;
    }
  }

  @override
  Future<LoadingState<SpaceArticlePage>> customGetData() =>
      RustSpace.article(mid: mid, page: page);
}
