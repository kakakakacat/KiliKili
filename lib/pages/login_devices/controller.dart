import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_login.dart';

/// 已登录设备列表（S14 / issue #65）。
///
/// Rust 直接返回设备列表本身，不再包一层 `LoginDevicesData`——那个模型
/// 只有一个字段，存在的理由是基线要给 `fromJson` 一个落点（ADR-011：
/// Dart 侧直接消费 FRB 类型，不再还原成 Dart 模型）。
class LoginDevicesController
    extends CommonListController<List<LoginDeviceData>, LoginDeviceData> {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<LoginDeviceData>? getDataList(List<LoginDeviceData> response) =>
      response;

  @override
  Future<LoadingState<List<LoginDeviceData>>> customGetData() =>
      RustLogin.loginDevices();
}
