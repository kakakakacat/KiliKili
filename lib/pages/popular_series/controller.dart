import 'package:kilikili/common/widgets/scroll_physics.dart' show ReloadMixin;
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/model_hot_video_item.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_video_feed.dart';
import 'package:kilikili/utils/extension/iterable_ext.dart';
import 'package:get/get.dart';

class PopularSeriesController
    extends CommonListController<PopularSeriesOne, HotVideoItemModel>
    with ReloadMixin {
  late int number;

  final config = Rxn<PopularSeriesConfig>();
  String? reminder;
  List<PopularSeriesListItem>? seriesList;

  @override
  void onInit() {
    super.onInit();
    _getSeriesList();
  }

  Future<void> _getSeriesList() async {
    final res = await RustVideoFeed.seriesList();
    if (res case Success(:final response)) {
      if (response != null && response.isNotEmpty) {
        number = response.first.number!.toInt();
        seriesList = response;
        queryData();
      } else {
        loadingState.value = const Success(null);
      }
    } else {
      loadingState.value = res as Error;
    }
  }

  @override
  List<HotVideoItemModel>? getDataList(PopularSeriesOne response) {
    config.value = response.config;
    reminder = response.reminder;
    return response.list?.map(HotVideoItemModel.fromRust).toList();
  }

  @override
  Future<LoadingState<PopularSeriesOne>> customGetData() =>
      RustVideoFeed.seriesOne(number: number);

  @override
  Future<void> onReload() {
    if (seriesList.isNullOrEmpty) {
      return _getSeriesList();
    }
    reload = true;
    return super.onReload();
  }
}
