import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_upload.dart';
import 'package:kilikili/rust/api/dynamic.dart' as rust;
import 'package:kilikili/services/rust/rust_dynamic.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:flutter/widgets.dart' show Text;
import 'package:get/get.dart';

class CreateVoteController extends GetxController {
  CreateVoteController(this.voteId);
  final int? voteId;

  String key = Utils.generateRandomString(6);
  final RxString title = ''.obs;
  final RxString desc = ''.obs;
  final RxInt type = 0.obs;
  final RxList<rust.VoteOptionData> options = <rust.VoteOptionData>[
    const rust.VoteOptionData(optDesc: '', imgUrl: '', cnt: 0),
    const rust.VoteOptionData(optDesc: '', imgUrl: '', cnt: 0),
  ].obs;
  final RxInt choiceCnt = 1.obs;
  final now = DateTime.now();
  late final end = now.copyWith(day: now.day + 90);
  late Rx<DateTime> endtime;
  final RxBool canCreate = false.obs;

  void updateCanCreate() {
    if (type.value == 0) {
      canCreate.value =
          title.value.isNotEmpty &&
          options.every((e) => e.optDesc?.isNotEmpty == true);
    } else {
      canCreate.value =
          title.value.isNotEmpty &&
          options.every(
            (e) =>
                e.optDesc?.isNotEmpty == true && e.imgUrl?.isNotEmpty == true,
          );
    }
  }

  @override
  void onInit() {
    super.onInit();
    endtime = DateTime(
      now.year,
      now.month,
      now.day + 1,
      now.hour,
      now.minute,
    ).obs;
    if (voteId != null) {
      queryData();
    }
  }

  Future<void> queryData() async {
    final res = await RustDynamic.voteInfo(voteId: voteId!);
    if (res.data case final response?) {
      key = Utils.generateRandomString(6);
      title.value = response.title!;
      desc.value = response.desc ?? '';
      type.value = response.options.first.imgUrl?.isNotEmpty == true ? 1 : 0;
      options.value = response.options;
      choiceCnt.value = response.choiceCnt!;
      endtime.value = DateTime.fromMillisecondsSinceEpoch(
        response.endTime! * 1000,
      );
      canCreate.value = true;
    } else {
      showConfirmDialog(
        context: Get.context!,
        title: Text(res.errorDetail ?? '投票信息读取失败'),
        onConfirm: Get.back,
      );
    }
  }

  void onDel(int i) {
    options.removeAt(i);
    updateCanCreate();
    if (choiceCnt.value > options.length) {
      choiceCnt.value = options.length;
    }
  }

  void addOption() {
    options.add(const rust.VoteOptionData(optDesc: '', imgUrl: '', cnt: 0));
    updateCanCreate();
  }

  Future<void> onCreate() async {
    final voteInfo = rust.VoteInfoData(
      choiceCnt: choiceCnt.value,
      defaultShare: null,
      desc: desc.value,
      endTime: null,
      status: null,
      uid: Accounts.main.mid,
      voteId: voteId,
      joinNum: 0,
      title: title.value,
      ctime: null,
      myVotes: null,
      options: options.toList(),
      optionsCnt: options.length,
      voterLevel: null,
      face: null,
      name: null,
      voteType: type.value,
      votePublisher: Accounts.main.mid,
      duration: endtime.value.difference(now).inSeconds,
      onlyFansLevel: 0,
    );
    final res = await RustDynamic.saveVote(
      info: voteInfo,
      update: voteId != null,
    );
    if (res.data case final response?) {
      Get.back(
        result: rust.VoteInfoData(
          choiceCnt: voteInfo.choiceCnt,
          defaultShare: voteInfo.defaultShare,
          desc: voteInfo.desc,
          endTime: voteInfo.endTime,
          status: voteInfo.status,
          uid: voteInfo.uid,
          voteId: response,
          joinNum: voteInfo.joinNum,
          title: voteInfo.title,
          ctime: voteInfo.ctime,
          myVotes: voteInfo.myVotes,
          options: voteInfo.options,
          optionsCnt: voteInfo.optionsCnt,
          voterLevel: voteInfo.voterLevel,
          face: voteInfo.face,
          name: voteInfo.name,
          voteType: voteInfo.voteType,
          votePublisher: voteInfo.votePublisher,
          duration: voteInfo.duration,
          onlyFansLevel: voteInfo.onlyFansLevel,
        ),
      );
    } else {
      Error(res.errorDetail ?? '投票保存失败', code: res.errorCode).toast();
    }
  }

  void updateOption(int index, {String? desc, String? imageUrl}) {
    final current = options[index];
    options[index] = rust.VoteOptionData(
      optIdx: current.optIdx,
      optDesc: desc ?? current.optDesc,
      cnt: current.cnt,
      imgUrl: imageUrl ?? current.imgUrl,
    );
    updateCanCreate();
  }

  Future<void> onUpload(int index, String path) async {
    final res = await RustUpload.awaitBfs(
      await RustUpload.beginBfs(path: path, category: 'daily', biz: 'vote'),
    );
    if (res case Success(:final response)) {
      updateOption(index, imageUrl: response.url);
    } else if (!RustUpload.isCancelled(res)) {
      res.toast();
    }
  }
}
