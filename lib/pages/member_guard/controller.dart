import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:get/get_core/src/get_main.dart';
import 'package:get/get_navigation/get_navigation.dart';

class MemberGuardController
    extends CommonListController<MemberGuardPage, GuardItem> {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  final int ruid = Get.arguments['ruid'];

  late List<GuardItem> tops;

  @override
  List<GuardItem>? getDataList(MemberGuardPage response) {
    return response.items;
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<MemberGuardPage> response) {
    if (response.response.hasMore != 1) {
      isEnd = true;
    }
    if (isRefresh) {
      final list = response.response.items;
      tops = list.take(3).toList();
      if (list.length > 3) {
        list.removeRange(0, 3);
      } else {
        list.clear();
      }
    }
    return false;
  }

  @override
  Future<LoadingState<MemberGuardPage>> customGetData() =>
      RustSpace.memberGuard(ruid: ruid, page: page);
}
