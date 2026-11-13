import 'package:kilikili/common/skeleton/video_card_v.dart';
import 'package:kilikili/common/sliver_single_child_delegate.dart';
import 'package:kilikili/models/search/result.dart';
import 'package:kilikili/pages/search_panel/controller.dart';
import 'package:kilikili/pages/search_panel/pgc/widgets/item.dart';
import 'package:kilikili/pages/search_panel/view.dart';
import 'package:kilikili/utils/grid.dart';
import 'package:kilikili/services/rust/rust_search.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart'
    hide SliverGridDelegateWithMaxCrossAxisExtent;

class SearchPgcPanel extends CommonSearchPanel {
  const SearchPgcPanel({
    super.key,
    required super.keyword,
    required super.tag,
    required super.searchType,
  });

  @override
  State<SearchPgcPanel> createState() => _SearchPgcPanelState();
}

class _SearchPgcPanelState
    extends
        CommonSearchPanelState<
          SearchPgcPanel,
          SearchPgcData,
          SearchPgcItem
        > {
  @override
  late final SearchPanelController<SearchPgcData, SearchPgcItem>
  controller;

  @override
  void initState() {
    super.initState();
    controller = Get.put(
      SearchPanelController<SearchPgcData, SearchPgcItem>(
        keyword: widget.keyword,
        searchType: widget.searchType,
        tag: widget.tag,
      ),
      tag: widget.searchType.name + widget.tag,
    );
  }

  late final gridDelegate = Grid.coverCardDelegate(aspectRatio: 3 / 4);

  @override
  Widget buildList(List<SearchPgcItem> list) {
    return SliverGrid.builder(
      gridDelegate: gridDelegate,
      itemBuilder: (BuildContext context, int index) {
        if (index == list.length - 1) {
          controller.onLoadMore();
        }
        return SearchPgcCard(item: list[index]);
      },
      itemCount: list.length,
    );
  }

  @override
  Widget get buildLoading => SliverGrid(
    gridDelegate: gridDelegate,
    delegate: const SliverSingleChildDelegate(
      count: 10,
      child: VideoCardVSkeleton(),
    ),
  );
}
