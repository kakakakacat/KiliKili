import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_space.dart';

class SeasonSeriesController
    extends CommonListController<SpaceSeasonSeriesPage, SpaceSeasonSeries> {
  SeasonSeriesController(this.mid);
  final int mid;
  int? count;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  List<SpaceSeasonSeries>? getDataList(SpaceSeasonSeriesPage response) {
    count = response.total;
    // 合集与列表分两个数组下发，拼成一条列表是界面的事（见领域层文档）。
    return [...response.seasons, ...response.series];
  }

  @override
  void checkIsEnd(int length) {
    if (count != null && length >= count!) {
      isEnd = true;
    }
  }

  @override
  Future<LoadingState<SpaceSeasonSeriesPage>> customGetData() =>
      RustSpace.seasonSeries(mid: mid, page: page);
}
