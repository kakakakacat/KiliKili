import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/video.dart';
import 'package:kilikili/models/common/rcmd_source.dart';
import 'package:kilikili/pages/common/common_controller.dart'
    show RefreshImplMixin;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:kilikili/utils/storage_pref.dart';

class RcmdController extends CommonListController with RefreshImplMixin {
  late bool enableSaveLastData = Pref.enableSaveLastData;

  /// 可在设置页切换后立即生效，无需重启
  RcmdSource rcmdSource = Pref.rcmdSource;

  int? lastRefreshAt;
  late bool savedRcmdTip = Pref.savedRcmdTip;

  @override
  bool get isEnd => false;

  @override
  void onInit() {
    super.onInit();
    page = 0;
    queryData();
  }

  bool get _hasData => switch (loadingState.value) {
    Success(:final List response) => response.isNotEmpty,
    _ => false,
  };

  @override
  Future<LoadingState> customGetData() {
    return switch (rcmdSource) {
      RcmdSource.app => VideoHttp.rcmdVideoListApp(freshIdx: page),
      // 桌面端：与官方 Windows 客户端一致，每次请求 30 条
      RcmdSource.desktop => VideoHttp.rcmdVideoList(
        freshIdx: page,
        ps: 30,
        desktop: true,
      ),
      RcmdSource.web => VideoHttp.rcmdVideoList(freshIdx: page, ps: 20),
    };
  }

  /// 「保留上次数据」只在已有数据时吞掉错误。首屏没有数据时吞掉，
  /// 页面会一直停在骨架屏，既不显示错误也没有重试按钮。
  @override
  bool handleError(String? errMsg) {
    return enableSaveLastData && _hasData;
  }

  @override
  void handleListResponse(List dataList) {
    if (enableSaveLastData && page == 0) {
      if (loadingState.value case Success(:final response)) {
        if (response != null && response.isNotEmpty) {
          if (savedRcmdTip) {
            lastRefreshAt = dataList.length;
          }
          if (response.length > 200) {
            dataList.addAll(response.take(50));
          } else {
            dataList.addAll(response);
          }
        }
      }
    }
  }

  @override
  Future<void> onRefresh() {
    page = 0;
    isEnd = false;
    return queryData();
  }

  void toTopAndRefresh() {
    scrollController.jumpToTop();
    showRefresh();
  }
}
