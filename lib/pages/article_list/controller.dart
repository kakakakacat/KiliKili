import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/rust/api/dynamic.dart' as rust;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_dynamic.dart';
import 'package:get/get.dart';

class ArticleListController
    extends
        CommonListController<rust.ArticleListData, rust.ArticleListItemData> {
  final id = Get.parameters['id']!;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  final list = Rxn<rust.ArticleListInfoData>();

  /// 作者三字段。基线这里套的是共享的 `Owner`，Rust 侧已摊平——`Owner`
  /// 还有别的生产者，把它一起迁会让本批无限扩散（#150）。
  int? authorMid;
  String? authorName;
  String? authorFace;

  @override
  List<rust.ArticleListItemData>? getDataList(rust.ArticleListData response) {
    list.value = response.info;
    authorMid = response.authorMid?.toInt();
    authorName = response.authorName;
    authorFace = response.authorFace;
    return response.articles;
  }

  @override
  Future<LoadingState<rust.ArticleListData>> customGetData() =>
      RustDynamic.articleList(id: id);
}
