import 'package:kilikili/http/fav.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/sub/sub/list.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_user.dart' show RustUser, SubList;
import 'package:kilikili/services/rust/sub_compat.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class SubController extends CommonListController<SubList, SubItemModel> {
  late final account = Accounts.main;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<void> queryData([bool isRefresh = true]) {
    if (!account.isLogin) {
      loadingState.value = const Error('账号未登录');
      return Future.syncValue(null);
    }
    return super.queryData(isRefresh);
  }

  // 取消订阅
  void cancelSub(SubItemModel subFolderItem) {
    showDialog(
      context: Get.context!,
      builder: (context) => AlertDialog(
        title: const Text('提示'),
        content: const Text('确定取消订阅吗？'),
        actions: [
          TextButton(
            onPressed: Get.back,
            child: Text(
              '取消',
              style: TextStyle(color: Theme.of(context).colorScheme.outline),
            ),
          ),
          TextButton(
            onPressed: () async {
              final res = await FavHttp.cancelSub(
                id: subFolderItem.id!,
                type: subFolderItem.type!,
              );
              if (res.isSuccess) {
                loadingState
                  ..value.data!.remove(subFolderItem)
                  ..refresh();
                SmartDialog.showToast('取消订阅成功');
              } else {
                res.toast();
              }
              Get.back();
            },
            child: const Text('确定'),
          ),
        ],
      ),
    );
  }

  @override
  List<SubItemModel>? getDataList(SubList response) {
    if (response.hasMore == false) {
      isEnd = true;
    }
    // 还原成 `SubItemModel`：合集详情页那条接口还在 Dart，两边喂同一批
    // 界面，见 `lib/services/rust/sub_compat.dart`。
    return response.list.map((e) => e.toItemModel()).toList();
  }

  @override
  Future<LoadingState<SubList>> customGetData() => RustUser.userSubFolder(
    pn: page,
    ps: 20,
    mid: account.mid,
  );
}
