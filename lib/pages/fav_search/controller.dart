import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/fav_order_type.dart';
import 'package:kilikili/models/common/video/source_type.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/pages/common/search/common_search_controller.dart';
import 'package:kilikili/pages/fav_detail/controller.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:get/get.dart';

class FavSearchController
    extends CommonSearchController<FavDetail, FavMedia>
    with IdMultiSelectMixin<FavDetail, FavMedia, int>, BaseFavController {
  @override
  final RxBool enableMultiSelect = false.obs;

  @override
  final RxInt rxCount = 0.obs;

  @override
  int idOf(FavMedia item) => item.id ?? -identityHashCode(item);

  late int type;
  @override
  late int mediaId;
  @override
  late bool isOwner;
  late dynamic count;
  late dynamic title;

  @override
  void onInit() {
    final args = Get.arguments;
    type = args['type'];
    mediaId = args['mediaId'];
    isOwner = args['isOwner'];
    count = args['count'];
    title = args['title'];
    super.onInit();
  }

  final Rx<FavOrderType> order = FavOrderType.mtime.obs;

  @override
  Future<LoadingState<FavDetail>> customGetData() => RustFav.favDetail(
    pn: page,
    ps: 20,
    mediaId: mediaId,
    keyword: editController.text,
    type: type,
    order: order.value,
  );

  @override
  List<FavMedia>? getDataList(FavDetail response) {
    if (response.hasMore == false) {
      isEnd = true;
    }
    return response.medias;
  }

  @override
  // TODO: dimension
  void onViewFav(FavMedia item, int? index) => PageUtils.toVideoPage(
    bvid: item.bvid,
    cid: item.ugcFirstCid!,
    cover: item.cover,
    title: item.title,
    extraArguments: {
      'sourceType': SourceType.fav,
      'mediaId': mediaId,
      'oid': item.id,
      'favTitle': title,
      'count': count,
      'desc': true,
      'isContinuePlaying': true,
    },
  );
}
