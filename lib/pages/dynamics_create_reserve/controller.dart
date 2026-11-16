import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/dynamic.dart' as rust;
import 'package:kilikili/services/rust/rust_dynamic.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:get/get.dart';

class CreateReserveController extends GetxController {
  CreateReserveController(this.sid);
  final int? sid;
  final RxInt subType = 0.obs;
  String key = Utils.generateRandomString(6);
  final RxString title = ''.obs;
  final now = DateTime.now();
  late final Rx<DateTime> date;
  late final end = now.copyWith(day: now.day + 90);
  final RxBool canCreate = false.obs;

  @override
  void onInit() {
    super.onInit();
    date = DateTime(now.year, now.month, now.day + 1, 20, 0).obs;
    if (sid != null) {
      queryData();
    }
  }

  void updateCanCreate() {
    canCreate.value = title.value.trim().isNotEmpty;
  }

  Future<void> queryData() async {
    final res = await RustDynamic.reserveInfo(sid: sid!);
    if (res.data case final response?) {
      key = Utils.generateRandomString(6);
      title.value = response.title;
      date.value = DateTime.fromMillisecondsSinceEpoch(
        response.livePlanStartTime * 1000,
      );
      canCreate.value = true;
    } else {
      Error(res.errorDetail ?? '预约信息请求失败', code: res.errorCode).toast();
    }
  }

  Future<void> onCreate() async {
    final livePlanStartTime = date.value.millisecondsSinceEpoch ~/ 1000;
    final res = sid == null
        ? await RustDynamic.createReserve(
            title: title.value,
            subType: subType.value,
            start: livePlanStartTime,
          )
        : await RustDynamic.updateReserve(
            sid: sid!,
            subType: subType.value,
            title: title.value,
            start: livePlanStartTime,
          );
    if (res.data case final response?) {
      Get.back(
        result: rust.ReserveCard(
          id: response,
          title: title.value,
          livePlanStartTime: livePlanStartTime,
        ),
      );
    } else {
      Error(res.errorDetail ?? '预约保存失败', code: res.errorCode).toast();
    }
  }
}
