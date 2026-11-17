import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:get/get.dart';

class UpowerRankController
    extends CommonListController<UpowerRank, UpowerRankItem> {
  UpowerRankController({
    this.privilegeType,
    required this.upMid,
  });

  final int upMid;
  final int? privilegeType;

  late final Rx<List<UpowerLevel>?> tabs = Rx<List<UpowerLevel>?>(null);

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<UpowerRankItem>? getDataList(UpowerRank response) {
    isEnd = true;
    // 只有一个档位时不显示 tab 条；`levels` 只在不指定档位时才下发。
    if (privilegeType == null && response.levels.length > 1) {
      tabs.value = response.levels;
    }
    return response.items;
  }

  @override
  Future<LoadingState<UpowerRank>> customGetData() => RustSpace.upowerRank(
    upMid: upMid,
    page: page,
    privilegeType: privilegeType,
  );
}
