import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/log_table/controller.dart';
import 'package:kilikili/services/rust/rust_user.dart';

class CoinLogController extends LogController<List<CoinLogItem>, CoinLogItem> {
  @override
  List<CoinLogItem>? getDataList(List<CoinLogItem> response) => response;

  @override
  Future<LoadingState<List<CoinLogItem>>> customGetData() => RustUser.coinLog();

  @override
  List<(int, String)> getFlexAndText(CoinLogItem item) {
    return [(3, item.time), (1, item.delta), (4, item.reason)];
  }

  @override
  final CoinLogItem header = const CoinLogItem(
    time: '时间',
    delta: '变化',
    reason: '原因',
  );

  @override
  final String title = '硬币记录';
}
