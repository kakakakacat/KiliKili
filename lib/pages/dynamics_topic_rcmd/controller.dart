import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/dynamic.dart' as rust;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_dynamic.dart';

class DynTopicRcmdController
    extends CommonListController<List<rust.TopicItemData>, rust.TopicItemData> {
  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<LoadingState<List<rust.TopicItemData>>> customGetData() =>
      RustDynamic.topicRcmd();
}
