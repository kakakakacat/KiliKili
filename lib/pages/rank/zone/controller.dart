import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/common/common_controller.dart'
    show RefreshImplMixin;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_video_feed.dart';
import 'package:kilikili/services/rust/rust_pgc.dart';

class ZoneController extends CommonListController with RefreshImplMixin {
  ZoneController({this.rid, this.seasonType});

  int? rid;
  int? seasonType;

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  @override
  Future<LoadingState> customGetData() {
    if (rid != null) {
      return RustVideoFeed.rank(rid!);
    }
    // seasonType == 1（番剧）走 web 变体，其余走 season 变体。两者的
    // 载荷键不同（result / data），由 Rust 侧按这个布尔分支。
    return RustPgc.rank(season: seasonType != 1, seasonType: seasonType!);
  }
}
