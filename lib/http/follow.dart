import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/error_msg.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/follow/data.dart';

/// 关注列表的只读接口。
///
/// `sortFollowTag`（分组排序）已迁到 `lib/services/rust/rust_misc.dart`
/// （#66 / ADR-013）。这里剩下的 `followings` 与 `FanHttp.fans` 共用
/// `FollowData` 模型，按 ADR-011 决策六要两个生产者一起迁，归 #66 的下一批。
abstract final class FollowHttp {
  static Future<LoadingState<FollowData>> followings({
    int? vmid,
    int? pn,
    int ps = 20,
    String orderType = '', // ''=>最近关注，'attention'=>最常访问
  }) async {
    final res = await Request().get(
      Api.followings,
      queryParameters: {
        'vmid': vmid,
        'pn': pn,
        'ps': ps,
        'order': 'desc',
        'order_type': orderType,
      },
    );
    if (res.data['code'] == 0) {
      return Success(FollowData.fromJson(res.data['data']));
    } else {
      return Error(errorMsg[res.data['code']] ?? res.data['message']);
    }
  }
}
