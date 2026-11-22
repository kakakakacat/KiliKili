import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_whisper_controller.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:get/get.dart';

class WhisperSecController extends CommonWhisperController<ImSessionPage> {
  WhisperSecController({
    required this.sessionPageType,
  });

  List<ImSessionOffset>? offsets;

  /// 由 `ImSessionTarget.foldPageType` 给，Rust 已经映射过。
  @override
  final int sessionPageType;

  Rx<List<ImThreeDotItem>?> threeDotItems = Rx<List<ImThreeDotItem>?>(null);

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<void> onRefresh() {
    offsets = null;
    return super.onRefresh();
  }

  @override
  List<ImSession>? getDataList(ImSessionPage response) {
    if (!response.hasMore) {
      isEnd = true;
    }
    offsets = response.offsets;
    return response.sessions;
  }

  @override
  bool customHandleResponse(bool isRefresh, Success<ImSessionPage> response) {
    if (isRefresh) {
      threeDotItems.value = response.response.threeDotItems;
    }
    return false;
  }

  @override
  Future<LoadingState<ImSessionPage>> customGetData() =>
      RustIm.sessionSecondary(pageType: sessionPageType, offsets: offsets);
}
