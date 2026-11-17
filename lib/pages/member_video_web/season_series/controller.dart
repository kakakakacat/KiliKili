import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/member/archive_sort_type_app.dart';
import 'package:kilikili/models/common/member/web_ss_type.dart';
import 'package:kilikili/pages/member_video_web/base/controller.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;
import 'package:kilikili/services/rust/space_web_compat.dart';
import 'package:get/get.dart';

class MemberSSWebCtr
    extends
        BaseVideoWebCtr<
          rust.SpaceSeasonWebPage,
          SpaceWebVideoModel,
          ArchiveSortTypeApp
        > {
  @override
  final Rx<ArchiveSortTypeApp> order = Rx(.desc);
  late final WebSsType _type;
  late final int _id;

  @override
  void onInit() {
    final args = Get.arguments;
    _type = args['type'];
    _id = args['id'];
    super.onInit();
  }

  @override
  List<SpaceWebVideoModel>? getDataList(rust.SpaceSeasonWebPage response) {
    return response.items.map(SpaceWebVideoModel.fromRust).toList();
  }

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<rust.SpaceSeasonWebPage> response,
  ) {
    if (isRefresh) {
      if (response.response.total?.toInt() case final total?) {
        count = total;
        totalPage = (total / ps).ceil();
      }
    }
    return false;
  }

  @override
  Future<LoadingState<rust.SpaceSeasonWebPage>> customGetData() =>
      rust.RustSpace.seasonWeb(
        kind: _type.kind,
        mid: mid,
        id: _id,
        page: page,
        pageSize: ps,
        ascending: order.value == ArchiveSortTypeApp.asc,
      );
}
