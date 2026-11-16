import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/search/common_search_controller.dart';
import 'package:kilikili/services/rust/rust_misc.dart' show FollowList, FollowUser;
import 'package:kilikili/services/rust/rust_relation.dart';

class FollowSearchController
    extends CommonSearchController<FollowList, FollowUser> {
  FollowSearchController(this.mid);
  final int mid;

  @override
  Future<LoadingState<FollowList>> customGetData() => RustRelation.followSearch(
    mid: mid,
    ps: 20,
    pn: page,
    name: editController.value.text,
  );

  @override
  List<FollowUser>? getDataList(FollowList response) {
    return response.list;
  }
}
