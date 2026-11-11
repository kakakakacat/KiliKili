import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/dyn/common_dyn_controller.dart';
import 'package:kilikili/services/rust/rust_misc.dart';
import 'package:get/get.dart';

class MusicDetailController extends CommonDynController {
  @override
  late final int oid;
  @override
  late final int replyType;

  @override
  dynamic get sourceId => oid.toString();

  final infoState = LoadingState<MusicDetail>.loading().obs;

  /// 「想听」的当前状态。**不写回详情**：详情是 Rust 侧的 typed 结构，
  /// 不可变（基线把新值写回模型当缓存，迁移后那条路走不通，也不该走——
  /// 服务端返回的快照不是本地状态的存放处）。
  final RxBool wishListen = false.obs;
  final RxInt wishCount = 0.obs;

  late final String musicId;

  String get shareUrl =>
      'https://music.bilibili.com/h5/music-detail?music_id=$musicId';

  @override
  void onInit() {
    super.onInit();
    musicId = Get.parameters['musicId']!;
    getMusicDetail();
  }

  Future<void> getMusicDetail() async {
    final res = await RustMisc.bgmDetail(musicId);
    if (res case Success(:final response)) {
      final comment = response.musicComment!;
      oid = comment.oid!;
      replyType = comment.pageType ?? 47;
      count.value = comment.nums ?? -1;
      wishListen.value = response.wishListen ?? false;
      wishCount.value = response.wishCount ?? 0;
      queryData();
    }
    infoState.value = res;
  }
}
