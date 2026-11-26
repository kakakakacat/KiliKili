import 'dart:math' show max;

import 'package:kilikili/common/skeleton/video_card_v.dart';
import 'package:kilikili/common/sliver_single_child_delegate.dart';
import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/flutter/refresh_indicator.dart';
import 'package:kilikili/common/widgets/loading_widget/http_error.dart';
import 'package:kilikili/common/widgets/scaffold/simple_scaffold.dart';
import 'package:kilikili/common/widgets/video_card/video_card_v.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/rcmd/controller.dart';
import 'package:kilikili/utils/grid.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:flutter/rendering.dart'
    show
        GrowthDirection,
        ScrollDirection,
        SliverConstraints,
        SliverGridRegularTileLayout;
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class RcmdPage extends StatefulWidget {
  const RcmdPage({super.key});

  @override
  State<RcmdPage> createState() => _RcmdPageState();
}

class _RcmdPageState extends State<RcmdPage>
    with AutomaticKeepAliveClientMixin {
  final controller = Get.put(RcmdController());

  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final colorScheme = ColorScheme.of(context);
    final child = Container(
      clipBehavior: .hardEdge,
      margin: const .symmetric(horizontal: Style.safeSpace),
      decoration: const BoxDecoration(borderRadius: Style.mdRadius),
      child: refreshIndicator(
        key: controller.refreshKey,
        onRefresh: controller.onRefresh,
        // 用外层的盒约束算一屏容量（#263）。刻意不用 SliverLayoutBuilder：
        // 滚动时 scrollOffset 变化会让它每帧重建整个列表；这里只有视口
        // 尺寸变化（转屏、折叠屏、分屏、dpi）时才重算。
        child: LayoutBuilder(
          builder: (context, box) {
            final capacity = _screenCapacity(box);
            _reportCapacity(capacity);
            return CustomScrollView(
              controller: controller.scrollController,
              physics: const AlwaysScrollableScrollPhysics(),
              slivers: [
                SliverPadding(
                  padding: const .only(top: Style.cardSpace, bottom: 100),
                  sliver: Obx(
                    () => _buildBody(
                      colorScheme,
                      controller.loadingState.value,
                      capacity,
                    ),
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
    if (PlatformUtils.isDesktop && Pref.showRefreshFAB) {
      return ScaffoldLayout(
        body: child,
        fab: Padding(
          padding: const .only(
            right: kFloatingActionButtonMargin,
            bottom: 80 + kFloatingActionButtonMargin,
          ),
          child: FloatingActionButton(
            tooltip: '刷新',
            onPressed: controller.toTopAndRefresh,
            backgroundColor: colorScheme.secondaryContainer,
            foregroundColor: colorScheme.onSecondaryContainer,
            child: const Icon(Icons.refresh),
          ),
        ),
      );
    }
    return child;
  }

  late final gridDelegate = SliverGridDelegateWithExtentAndRatio(
    mainAxisSpacing: Style.cardSpace,
    crossAxisSpacing: Style.cardSpace,
    maxCrossAxisExtent: Pref.recommendCardWidth,
    childAspectRatio: Style.coverCardRatio,
  );

  /// 一屏可容纳的卡片数（#263），直接用网格自己的布局算：列数与行高和
  /// 实际显示完全一致。视口尺寸、dpi（逻辑尺寸随之变化）、列宽设置任一
  /// 变化都会改变这里的结果；值不变时控制器不做任何事。行数向上取整：
  /// 最后一行露出一部分也算在屏上，宁可多拉不留空位。
  int _screenCapacity(BoxConstraints box) {
    if (!box.hasBoundedWidth || !box.hasBoundedHeight) return 10;
    final layout = gridDelegate.getLayout(
      SliverConstraints(
        axisDirection: AxisDirection.down,
        growthDirection: GrowthDirection.forward,
        userScrollDirection: ScrollDirection.idle,
        scrollOffset: 0,
        precedingScrollExtent: 0,
        overlap: 0,
        remainingPaintExtent: box.maxHeight,
        crossAxisExtent: box.maxWidth,
        crossAxisDirection: AxisDirection.right,
        viewportMainAxisExtent: box.maxHeight,
        remainingCacheExtent: box.maxHeight,
        cacheOrigin: 0,
      ),
    );
    if (layout is! SliverGridRegularTileLayout || layout.mainAxisStride <= 0) {
      return 10;
    }
    final rows = (box.maxHeight / layout.mainAxisStride).ceil();
    return max(1, layout.crossAxisCount * rows);
  }

  int? _reportedCapacity;

  /// 布局阶段不能触发请求与状态更新，挪到帧后上报。
  void _reportCapacity(int capacity) {
    if (capacity == _reportedCapacity) return;
    _reportedCapacity = capacity;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) controller.updateScreenCapacity(capacity);
    });
  }

  Widget _buildBody(
    ColorScheme colorScheme,
    LoadingState<List<dynamic>?> loadingState,
    int capacity,
  ) {
    return switch (loadingState) {
      Loading() => _buildSkeleton(capacity),
      Success(:final response) =>
        response != null && response.isNotEmpty
            ? SliverGrid.builder(
                gridDelegate: gridDelegate,
                itemBuilder: (context, index) {
                  if (index == response.length - 1) {
                    controller.onLoadMore();
                  }
                  // 「保留上次内容」时新旧内容直接相接，不再插「上次看到
                  // 这里，点击刷新」提示卡（#268，所有者决定，连同设置开关
                  // 一起删除；与上游 PiliPlus 分叉）。
                  return VideoCardV(
                    videoItem: response[index],
                    onRemove: () => controller.loadingState
                      ..value.data!.removeAt(index)
                      ..refresh(),
                  );
                },
                itemCount: response.length,
              )
            : HttpError(onReload: controller.onReload),
      Error(:final errMsg) => HttpError(
        errMsg: errMsg,
        onReload: controller.onReload,
      ),
    };
  }

  /// 骨架数与一屏容量一致，避免骨架 10 个、实际铺满时的跳变（#263）。
  Widget _buildSkeleton(int count) => SliverGrid(
    gridDelegate: gridDelegate,
    delegate: SliverSingleChildDelegate(
      count: count,
      child: const VideoCardVSkeleton(),
    ),
  );
}
