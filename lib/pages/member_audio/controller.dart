import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/grpc/bilibili/app/listener/v1.pbenum.dart'
    show PlaylistSource;
import 'package:kilikili/http/loading_state.dart';

import 'package:kilikili/pages/audio/view.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';

class MemberAudioController
    extends CommonListController<SpaceAudioPage, SpaceAudioItem> {
  MemberAudioController(this.mid);

  final int mid;
  int? totalSize;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  void checkIsEnd(int length) {
    if (spaceAudioIsEnd(loadedCount: length, totalCount: totalSize)) {
      isEnd = true;
    }
  }

  @override
  List<SpaceAudioItem>? getDataList(SpaceAudioPage response) {
    totalSize = response.totalSize;
    return response.items;
  }

  @override
  Future<LoadingState<SpaceAudioPage>> customGetData() => RustSpace.audio(
    page: page,
    mid: mid,
  );

  void toViewPlayAll() {
    final item = loadingState.value.data!.first;
    AudioPage.toAudioPage(
      itemType: 3,
      id: item.uid!,
      oid: item.id!,
      from: PlaylistSource.MEM_SPACE,
    );
  }
}
