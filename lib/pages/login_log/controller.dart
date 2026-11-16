import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/log_table/controller.dart';
import 'package:kilikili/services/rust/rust_user.dart';

class LoginLogController extends LogController<List<LoginLogItem>, LoginLogItem> {
  @override
  List<LoginLogItem>? getDataList(List<LoginLogItem> response) => response;

  @override
  Future<LoadingState<List<LoginLogItem>>> customGetData() => RustUser.loginLog();

  @override
  List<(int, String)> getFlexAndText(LoginLogItem item) {
    return [(3, item.timeAt), (2, item.ip), (3, item.geo)];
  }

  @override
  final LoginLogItem header = const LoginLogItem(
    timeAt: '时间',
    ip: '变化',
    geo: '地理位置',
  );

  @override
  final String title = '登录记录';
}
