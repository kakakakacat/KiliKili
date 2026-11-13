import 'package:kilikili/common/widgets/loading_widget/http_error.dart';
import 'package:kilikili/common/widgets/video_card/video_card_h.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/model_hot_video_item.dart';
import 'package:kilikili/pages/video/related/controller.dart';
import 'package:kilikili/utils/extension/get_ext.dart';
import 'package:kilikili/utils/grid.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class RelatedVideoPanel extends StatefulWidget {
  const RelatedVideoPanel({super.key, required this.heroTag});
  final String heroTag;
  @override
  State<RelatedVideoPanel> createState() => _RelatedVideoPanelState();
}

class _RelatedVideoPanelState extends State<RelatedVideoPanel> with GridMixin {
  late final RelatedController _relatedController;

  @override
  void initState() {
    super.initState();
    _relatedController = Get.putOrFind(
      RelatedController.new,
      tag: widget.heroTag,
    );
  }

  @override
  Widget build(BuildContext context) {
    return SliverPadding(
      padding: const EdgeInsets.only(top: 7, bottom: 100),
      sliver: Obx(() => _buildBody(_relatedController.loadingState.value)),
    );
  }

  Widget _buildBody(LoadingState<List<HotVideoItemModel>?> loadingState) {
    return switch (loadingState) {
      Loading() => coverGridSkeleton,
      Success(:final response) =>
        response != null && response.isNotEmpty
            ? SliverGrid.builder(
                gridDelegate: coverGridDelegate,
                itemBuilder: (context, index) {
                  return VideoCardH(
                    videoItem: response[index],
                    onRemove: () => _relatedController.loadingState
                      ..value.data!.removeAt(index)
                      ..refresh(),
                  );
                },
                itemCount: response.length,
              )
            : const SliverToBoxAdapter(),
      Error(:final errMsg) => HttpError(
        errMsg: errMsg,
        onReload: _relatedController.onReload,
      ),
    };
  }
}
