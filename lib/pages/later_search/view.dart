import 'package:kilikili/models/common/video/source_type.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/pages/common/search/common_search_page.dart';
import 'package:kilikili/pages/later/widgets/video_card_h_later.dart';
import 'package:kilikili/pages/later_search/controller.dart';
import 'package:kilikili/utils/grid.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/request_utils.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class LaterSearchPage extends StatefulWidget {
  const LaterSearchPage({super.key});

  @override
  State<LaterSearchPage> createState() => _LaterSearchPageState();
}

class _LaterSearchPageState
    extends CommonSearchPageState<LaterSearchPage, LaterPage, LaterItem>
    with GridMixin {
  late final enablePlayAll = Pref.enablePlayAll;

  @override
  final LaterSearchController controller = Get.put(
    LaterSearchController(),
    tag: Utils.generateRandomString(8),
  );

  @override
  List<Widget>? get multiSelectActions {
    final btnStyle = TextButton.styleFrom(visualDensity: .compact);
    final textStyle = TextStyle(
      color: ColorScheme.of(context).onSurfaceVariant,
    );
    return [
      TextButton(
        style: btnStyle,
        onPressed: () => RequestUtils.onCopyOrMoveLater(
          context: context,
          isCopy: true,
          ctr: controller,
          mid: controller.mid,
        ),
        child: Text('复制', style: textStyle),
      ),
      TextButton(
        style: btnStyle,
        onPressed: () => RequestUtils.onCopyOrMoveLater(
          context: context,
          isCopy: false,
          ctr: controller,
          mid: controller.mid,
        ),
        child: Text('移动', style: textStyle),
      ),
    ];
  }

  @override
  Widget buildList(List<LaterItem> list) {
    return SliverGrid.builder(
      gridDelegate: coverGridDelegate,
      itemBuilder: (context, index) {
        if (index == list.length - 1) {
          controller.onLoadMore();
        }
        final item = list[index];
        return VideoCardHLater(
          index: index,
          videoItem: item,
          ctr: controller,
          onViewLater: (cid) {
            PageUtils.toVideoPage(
              bvid: item.bvid,
              cid: cid,
              cover: item.pic,
              title: item.title,
              // Rust 给的宽高已按 rotate 摆正。
              dimension: item.width == null || item.height == null
                  ? null
                  : Dimension(width: item.width, height: item.height),
              extraArguments: enablePlayAll
                  ? {
                      'oid': item.aid,
                      'sourceType': SourceType.watchLater,
                      'count': controller.count,
                      'favTitle': '稍后再看',
                      'mediaId': controller.mid,
                      'desc': false,
                      'isContinuePlaying': index != 0,
                    }
                  : const {'viewLater': true},
            );
          },
        );
      },
      itemCount: list.length,
    );
  }
}
