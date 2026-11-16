import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_data_controller.dart';
import 'package:kilikili/services/rust/rust_user.dart';

/// 空间隐私设置页（S7 #57 起改吃 Rust 的 typed 结构）。
///
/// ## 开关的当前值为什么不在 [PrivacySetting] 上
///
/// Rust 回来的 `PrivacySetting` 是不可变的——它描述的是「服务端此刻说这个
/// 开关是什么」。用户在页面上拨动开关属于**界面状态**，离开页面才一次性
/// 提交（见 [onMod]），所以放在控制器的 [values] 里。把可变性塞回导出结构
/// 等于让宿主的习惯泄进 Rust（ADR-011 / AGENTS.md §2）。
class SpaceSettingController
    extends CommonDataController<List<PrivacySetting>, List<PrivacySetting>> {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  bool? hasMod;

  /// 开关的当前值，key → 0/1。服务端没下发的那几项不会出现在这里。
  final Map<String, int> values = {};

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<List<PrivacySetting>> response,
  ) {
    values
      ..clear()
      ..addEntries(
        response.response
            .where((e) => e.value != null)
            .map((e) => MapEntry(e.key, e.value!)),
      );
    return false;
  }

  /// 这一项当前是「开」还是「关」。
  ///
  /// 反相的那几项（`disable_*` 一族）要取反才是界面上说的那个意思，判据
  /// 由 Rust 给（`reversed`）——读错它就是把用户的隐私设置显示反。
  bool isOn(PrivacySetting item) {
    final value = values[item.key];
    if (value == null) return false;
    return item.reversed ? value == 0 : value == 1;
  }

  /// 拨动一项。只改本地状态，提交在 [onMod]。
  void toggle(PrivacySetting item, [bool? on]) {
    hasMod ??= true;
    final next = on ?? !isOn(item);
    values[item.key] = item.reversed
        ? (next ? 0 : 1)
        : (next ? 1 : 0);
  }

  @override
  Future<LoadingState<List<PrivacySetting>>> customGetData() =>
      RustUser.spaceSetting();

  /// 离开页面时一次性提交。整批是置为语义，重发安全。
  Future<void> onMod() async {
    if (hasMod ?? false) {
      if (values.isNotEmpty) {
        final res = await RustUser.spaceSettingMod(values);
        if (!res.isSuccess) {
          res.toast();
        }
      }
    }
  }
}
