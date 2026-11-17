import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/member.dart';
import 'package:kilikili/models/common/member/search_type.dart';
import 'package:kilikili/models/dynamics/result.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/member_search/controller.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;

class MemberSearchChildController extends CommonListController {
  MemberSearchChildController(this.controller, this.searchType);

  final MemberSearchController controller;
  final MemberSearchType searchType;

  // archive
  static const _ps = 20;
  late final _mid = int.parse(controller.mid);

  // dynamic
  String? offset;

  @override
  void checkIsEnd(int length) {
    final count = controller.counts[searchType.index];
    if (count != -1 && length >= count) {
      isEnd = true;
    }
  }

  @override
  List? getDataList(response) {
    switch (searchType) {
      case MemberSearchType.archive:
        final rust.SpaceSearchArchivePage data = response;
        controller.counts[searchType.index] = data.total?.toInt() ?? 0;
        return data.items;
      case MemberSearchType.dynamic:
        DynamicsDataModel data = response;
        offset = data.offset;
        if (data.hasMore == false) {
          isEnd = true;
        }
        controller.counts[searchType.index] = data.total ?? 0;
        return data.items;
    }
  }

  @override
  Future<void> onRefresh() {
    offset = null;
    return super.onRefresh();
  }

  @override
  Future<LoadingState> customGetData() {
    return switch (searchType) {
      MemberSearchType.archive => rust.RustSpace.searchArchiveGrpc(
        mid: _mid,
        page: page,
        pageSize: _ps,
        keyword: controller.editingController.text,
      ),
      MemberSearchType.dynamic => MemberHttp.dynSearch(
        mid: controller.mid,
        pn: page,
        offset: offset ?? '',
        keyword: controller.editingController.text,
      ),
    };
  }
}
