import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/live/live_contribution_rank_type.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_live.dart';

class ContributionRankController
    extends
        CommonListController<
          List<LiveContributionRankItem>,
          LiveContributionRankItem
        > {
  final int ruid;
  final int roomId;
  final LiveContributionRankType type;

  ContributionRankController({
    required this.ruid,
    required this.roomId,
    required this.type,
  });

  @override
  void onInit() {
    super.onInit();
    queryData();
  }

  /// 响应本身就是榜单列表：基线那层只含一个 `item` 字段的 `…RankData`
  /// 壳在 Rust 侧已经摊掉了。
  @override
  List<LiveContributionRankItem>? getDataList(
    List<LiveContributionRankItem> response,
  ) => response;

  /// `type.name` 与 `type.sw1tch` **必须成对**传给服务端：只带其一会回
  /// 一个空榜而不是报错，表现为「这个榜没人」。
  @override
  Future<LoadingState<List<LiveContributionRankItem>>> customGetData() =>
      RustLive.contributionRank(
        ruid: ruid,
        roomId: roomId,
        page: page,
        kind: type.name,
        switchName: type.sw1tch,
      );
}
