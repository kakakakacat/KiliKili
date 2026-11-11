import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_video_page.dart';
import 'package:get/get.dart';

class NoteListPageCtr extends CommonListController<NoteList, NoteItem> {
  NoteListPageCtr({required this.oid});
  final int oid;

  RxInt count = (-1).obs;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<NoteItem>? getDataList(NoteList response) {
    count.value = response.total ?? -1;
    return response.list;
  }

  @override
  void checkIsEnd(int length) {
    final count = this.count.value;
    if (count != -1 && length >= count) {
      isEnd = true;
    }
  }

  @override
  Future<LoadingState<NoteList>> customGetData() =>
      RustVideoPage.noteList(oid: oid, page: page);
}
