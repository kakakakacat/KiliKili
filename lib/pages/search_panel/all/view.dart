import 'package:kilikili/models/search/result.dart';
import 'package:kilikili/services/rust/rust_misc.dart' show MatchContest;
import 'package:kilikili/pages/search_panel/all/controller.dart';
import 'package:kilikili/pages/search_panel/all/widgets/activity.dart';
import 'package:kilikili/pages/search_panel/all/widgets/esports.dart';
import 'package:kilikili/pages/search_panel/all/widgets/user.dart';
import 'package:kilikili/pages/search_panel/pgc/widgets/item.dart';
import 'package:kilikili/pages/search_panel/video/view.dart';
import 'package:kilikili/pages/search_panel/view.dart';
import 'package:kilikili/utils/grid.dart';
import 'package:kilikili/services/rust/rust_search.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart'
    hide SliverGridDelegateWithMaxCrossAxisExtent;

class SearchAllPanel extends SearchVideoPanel {
  const SearchAllPanel({
    super.key,
    required super.keyword,
    required super.tag,
    required super.searchType,
  });

  @override
  State<SearchAllPanel> createState() => _SearchAllPanelState();
}

class _SearchAllPanelState
    extends
        CommonSearchPanelState<
          SearchAllPanel,
          SearchVideoData,
          SearchVideoItemModel
        >
    with GridMixin, SearchVideoPanelMixin<SearchAllPanel> {
  @override
  late final SearchAllController controller;

  @override
  void initState() {
    super.initState();
    controller = Get.put(
      SearchAllController(
        keyword: widget.keyword,
        searchType: widget.searchType,
        tag: widget.tag,
      ),
      tag: widget.searchType.name + widget.tag,
    );
  }

  @override
  Widget buildList(List<SearchVideoItemModel> list) {
    return SliverMainAxisGroup(
      slivers: [
        if (controller.searchEsports?.isNotEmpty ?? false) ...[
          _buildEsports(controller.searchEsports!.first),
          SliverToBoxAdapter(
            child: Divider(
              height: 14,
              color: colorScheme.outline.withValues(alpha: 0.1),
            ),
          ),
        ],
        ...?controller.searchActivity?.map((e) {
          return SliverToBoxAdapter(
            child: SearchActivityCard(item: e),
          );
        }),
        ...?controller.searchUser?.map((e) {
          return SliverToBoxAdapter(
            child: SearchAllUserItem(item: e),
          );
        }),
        if (controller.searchMedia?.isNotEmpty ?? false) ...[
          _buildPgc(controller.searchMedia!),
          SliverToBoxAdapter(
            child: Divider(
              height: 14,
              color: colorScheme.outline.withValues(alpha: 0.1),
            ),
          ),
        ],
        super.buildList(list),
      ],
    );
  }

  static Widget _buildEsports(MatchContest contest) {
    return SliverToBoxAdapter(child: SearchEsportsItem(contest: contest));
  }

  static Widget _buildPgc(List<SearchPgcItem> list) {
    final Widget child;
    if (list.length == 1) {
      child = Align(
        alignment: Alignment.centerLeft,
        child: SizedBox(width: 240, child: SearchPgcCard(item: list.first)),
      );
    } else {
      child = ListView.builder(
        padding: .zero,
        itemExtent: 240,
        itemCount: list.length,
        scrollDirection: .horizontal,
        physics: const AlwaysScrollableScrollPhysics(),
        itemBuilder: (context, index) {
          return SearchPgcCard(item: list[index]);
        },
      );
    }
    return SliverToBoxAdapter(child: SizedBox(height: 320, child: child));
  }

  late final pgcGridDelegate = Grid.coverCardDelegate(aspectRatio: 3 / 4);
}
