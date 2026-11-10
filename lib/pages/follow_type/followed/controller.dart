import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/user.dart';
import 'package:kilikili/models_new/follow/data.dart';
import 'package:kilikili/pages/follow_type/controller.dart';

class FollowedController extends FollowTypeController {
  @override
  Future<LoadingState<FollowData>> customGetData() =>
      UserHttp.followedUp(mid: mid, pn: page);
}
