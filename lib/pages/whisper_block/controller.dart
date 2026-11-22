import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';

/// 私信关键词屏蔽。
///
/// 列表项从 protobuf 的 `KeywordBlockingItem` 换成了裸字符串——那个消息
/// 只有一个 `keyword` 字段，包一层没有任何收益，而 Rust 侧投影时顺手把
/// 空串过滤掉了。
class WhisperBlockController
    extends CommonListController<ImKeywordBlocking, String> {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  RxInt count = 0.obs;
  int? listLimit;
  int? charLimit;

  @override
  List<String>? getDataList(ImKeywordBlocking response) {
    count.value = response.keywords.length;
    listLimit = response.listLimit;
    charLimit = response.charLimit;
    // FRB 的列表要拷一份：下面 onAdd / onRemove 会往里增删。
    return [...response.keywords];
  }

  @override
  Future<LoadingState<ImKeywordBlocking>> customGetData() =>
      RustIm.keywordBlocking();

  Future<void> onAdd(String keyword) async {
    final res = await RustIm.addKeyword(keyword);
    if (res.isSuccess) {
      Get.back();
      loadingState
        ..value.data!.add(keyword)
        ..refresh();
      count.value += 1;
      SmartDialog.showToast('添加成功');
    } else {
      res.toast();
    }
  }

  Future<void> onRemove(String keyword) async {
    final res = await RustIm.deleteKeyword(keyword);
    if (res.isSuccess) {
      loadingState
        ..value.data!.remove(keyword)
        ..refresh();
      count.value -= 1;
      SmartDialog.showToast('删除成功');
    } else {
      res.toast();
    }
  }
}
