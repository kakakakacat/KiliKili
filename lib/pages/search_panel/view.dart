import 'package:kilikili/common/widgets/flutter/refresh_indicator.dart';
import 'package:kilikili/common/widgets/loading_widget/http_error.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/search/search_type.dart';
import 'package:kilikili/models/search/result.dart';
import 'package:kilikili/pages/search_panel/controller.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

abstract class CommonSearchPanel extends StatefulWidget {
  const CommonSearchPanel({
    super.key,
    required this.keyword,
    required this.searchType,
    required this.tag,
  });

  final String keyword;
  final SearchType searchType;
  final String tag;
}

abstract class CommonSearchPanelState<
  S extends CommonSearchPanel,
  R extends SearchNumData<T>,
  T
>
    extends State<S>
    with AutomaticKeepAliveClientMixin {
  SearchPanelController<R, T> get controller;

  late ColorScheme colorScheme;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    colorScheme = ColorScheme.of(context);
  }

  @override
  bool get wantKeepAlive => true;

  @override
  void dispose() {
    controller.cancelListener();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return refreshIndicator(
      onRefresh: controller.onRefresh,
      child: CustomScrollView(
        controller: controller.scrollController,
        physics: const AlwaysScrollableScrollPhysics(),
        slivers: [
          ?buildHeader(),
          SliverPadding(
            padding: EdgeInsets.only(
              bottom: MediaQuery.viewPaddingOf(context).bottom + 100,
            ),
            sliver: SliverMainAxisGroup(
              slivers: [
                Obx(() => _buildBody(controller.loadingState.value)),
                Obx(_buildFooter),
              ],
            ),
          ),
        ],
      ),
    );
  }

  /// 加载更多的结局要说出来（#257）：失败时给原因与重试，到底时说没有了。
  /// 两者都不是时什么也不画——刻意不显示「加载中...」，加载更多是在最后一条
  /// 出现时悄悄触发的，常驻一行「加载中」反而像卡住。
  Widget _buildFooter() {
    final error = controller.loadMoreError.value;
    final reachedEnd = controller.reachedEnd.value;
    if (controller.loadingState.value case Success(:final response)) {
      if (response == null || response.isEmpty) {
        return const SliverToBoxAdapter();
      }
    } else {
      return const SliverToBoxAdapter();
    }
    final style = TextStyle(fontSize: 13, color: colorScheme.outline);
    final Widget child;
    if (error != null) {
      // 重试做成按钮而不是可点的文字：一行灰字看不出能点。样式与整页
      // 错误态 `HttpError` 的「点击重试」一致。
      child = Padding(
        padding: const .symmetric(horizontal: 16, vertical: 16),
        child: Column(
          mainAxisSize: .min,
          spacing: 10,
          children: [
            Text('加载失败：$error', textAlign: .center, style: style),
            FilledButton.tonal(
              onPressed: controller.retryLoadMore,
              style: FilledButton.styleFrom(
                tapTargetSize: .padded,
                backgroundColor: colorScheme.primary.withAlpha(20),
              ),
              child: Text(
                '点击重试',
                style: TextStyle(color: colorScheme.primary),
              ),
            ),
          ],
        ),
      );
    } else if (reachedEnd) {
      child = Padding(
        padding: const .symmetric(vertical: 16),
        child: Text('没有更多了', textAlign: .center, style: style),
      );
    } else {
      return const SliverToBoxAdapter();
    }
    return SliverToBoxAdapter(
      child: SizedBox(width: double.infinity, child: child),
    );
  }

  Widget get buildLoading;

  Widget _buildBody(LoadingState<List<T>?> loadingState) {
    return switch (loadingState) {
      Loading() => buildLoading,
      Success(:final response) =>
        response != null && response.isNotEmpty
            ? buildList(response)
            : HttpError(onReload: controller.onReload),
      Error(:final errMsg) => HttpError(
        errMsg: errMsg,
        onReload: controller.onReload,
      ),
    };
  }

  Widget? buildHeader() => null;

  Widget buildList(List<T> list);
}
