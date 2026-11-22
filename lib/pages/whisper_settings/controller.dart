import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_data_controller.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:get/get.dart';

/// 私信设置页。
///
/// 读回来的是一份 [ImSettingsSnapshot]：给界面看的投影，外加**整条响应的
/// 原始字节**。改一项时把那段字节原样递回 Rust，由它重新解码、只改一处、
/// 再把整张表发回去。
///
/// 为什么不按投影重建请求：`SetImSettings` 收的是整张设置表，按窄实体
/// 拼回去会发出一个缺字段的赝品——编译得过、长得像、服务端多半也收，但
/// 凡是没投影的字段（子设置树、文案、url）都会在这一次写回里被悄悄抹掉。
class WhisperSettingsController
    extends CommonDataController<ImSettingsSnapshot, ImSettingsSnapshot> {
  WhisperSettingsController({
    required this.imSettingType,
  });

  final int imSettingType;

  final RxString title = ''.obs;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<ImSettingsSnapshot> response,
  ) {
    title.value = response.response.pageTitle;
    loadingState.value = Success(response.response);
    return true;
  }

  @override
  Future<LoadingState<ImSettingsSnapshot>> customGetData() =>
      RustIm.settings(imSettingType);

  /// 改一项并写回。成功后重新拉一次，让投影（尤其是跳转项右侧那行
  /// trailing 文字）与服务端对齐。
  ///
  /// 基线是就地改 protobuf 对象、再把子页的改动 merge 回父页。这里换成
  /// 重新拉一次：多一次请求，但省掉「把一棵树 merge 回另一棵树」那段
  /// 容易出错的逻辑，而且 trailing 文字的算法只有 Rust 一份。
  Future<bool> onSet({
    required int key,
    bool? switchOn,
    int? selectedValue,
  }) async {
    final snapshot = loadingState.value.dataOrNull;
    if (snapshot == null) return false;
    final res = await RustIm.setSetting(
      raw: snapshot.raw,
      key: key,
      switchOn: switchOn,
      selectedValue: selectedValue,
    );
    if (!res.isSuccess) {
      res.toast();
      return false;
    }
    await queryData();
    return true;
  }
}
