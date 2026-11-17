import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/follow_type/controller.dart';
import 'package:kilikili/services/rust/rust_misc.dart' show FollowList;
import 'package:kilikili/services/rust/rust_relation.dart';

class FollowSameController extends FollowTypeController {
  @override
  Future<LoadingState<FollowList>> customGetData() =>
      RustRelation.sameFollowing(mid: mid, pn: page);
}
