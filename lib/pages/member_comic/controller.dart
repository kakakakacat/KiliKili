import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/member/contribute_type.dart';
import 'package:kilikili/models_new/space/space_archive/item.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;

class MemberComicController
    extends CommonListController<rust.SpaceArchivePage, SpaceArchiveItem> {
  MemberComicController(this.mid);

  final int mid;

  int? count;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  void checkIsEnd(int length) {
    if (count != null && length >= count!) {
      isEnd = true;
    }
  }

  @override
  List<SpaceArchiveItem>? getDataList(rust.SpaceArchivePage response) {
    count = response.count?.toInt();
    return response.items.map(SpaceArchiveItem.fromRust).toList();
  }

  @override
  Future<LoadingState<rust.SpaceArchivePage>> customGetData() =>
      rust.RustSpace.spaceArchive(
        kind: ContributeType.comic.kind,
        mid: mid,
      );
}
