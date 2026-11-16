import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/rust/api/dynamic.dart';
import 'package:kilikili/services/rust/rust_dynamic.dart';
import 'package:get/get.dart';

class DynLikeController
    extends CommonListController<LikeListPage, DynamicAuthor> {
  DynLikeController(this.id, {int count = -1}) : count = RxInt(count);
  final String id;

  int? _uidOffset;
  final RxInt count;

  @override
  List<DynamicAuthor>? getDataList(LikeListPage response) {
    count.value = response.totalCount.toInt();
    final list = response.items;
    _uidOffset = list.lastOrNull?.mid.toInt();
    if (!response.hasMore) {
      isEnd = true;
    }
    return list;
  }

  @override
  Future<LoadingState<LikeListPage>> customGetData() async {
    final result = await RustDynamic.likeList(
      dynamicId: id,
      uidOffset: _uidOffset,
      page: page,
    );
    if (result.data case final data?) return Success(data);
    return Error(result.errorDetail ?? '动态点赞列表获取失败');
  }

  @override
  Future<void> onRefresh() {
    _uidOffset = null;
    return super.onRefresh();
  }
}
