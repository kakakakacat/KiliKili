import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/member/contribute_type.dart';
import 'package:kilikili/models_new/space/space_archive/item.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/member/controller.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;
import 'package:get/get.dart';

class MemberBangumiCtr
    extends CommonListController<rust.SpaceArchivePage, SpaceArchiveItem> {
  MemberBangumiCtr({
    required this.mid,
    required this.heroTag,
  });

  final int mid;
  final String? heroTag;
  int? count;
  late final _ctr = Get.find<MemberController>(tag: heroTag);

  @override
  void onInit() {
    super.onInit();
    // 空间主页已经带回了第一页追番，直接接着用，不再多发一次请求。
    final response = _ctr.loadingState.value.data;
    if (response != null) {
      page = 2;
      final res = response.season!;
      final items = res.items.map(SpaceArchiveItem.fromRust).toList();
      loadingState.value = Success(items);
      count = res.count!.toInt();
      isEnd = items.length >= count!;
    } else {
      queryData();
    }
  }

  @override
  List<SpaceArchiveItem>? getDataList(rust.SpaceArchivePage response) {
    return response.items.map(SpaceArchiveItem.fromRust).toList();
  }

  @override
  void checkIsEnd(int length) {
    if (count != null && length >= count!) {
      isEnd = true;
    }
  }

  @override
  Future<LoadingState<rust.SpaceArchivePage>> customGetData() =>
      rust.RustSpace.spaceArchive(
        kind: ContributeType.bangumi.kind,
        mid: mid,
        page: page,
      );
}
