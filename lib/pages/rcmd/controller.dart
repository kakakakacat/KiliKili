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

  @override
  bool get isEnd => false;

  // ---- 首屏铺满（#263）----
  //
  // 默认的 app 推荐接口没有 `ps` 参数，每次约 10 条，大屏一屏放不下就会
  // 留空位。这里在刷新后按「一屏容量」自动续拉，直到铺满或达到上限。
  //
  // 为什么留在宿主：推荐流还没迁到 Rust（`lib/http/video.dart`）。「不够
  // 就续拉、最多几次」是阈值与重试策略，按 AGENTS.md §2 应该在 Rust；
  // 推荐切片迁移时把它做成 Rust 接口的参数（如 `min_count`），并删掉这一段。
  // 「一屏容量」依赖布局，那部分留在宿主（见 view 的 `_reportCapacity`）。

  /// 一屏可容纳的卡片数，由页面按网格布局算出；视口尺寸、dpi、列宽设置
  /// 变化时页面会重新上报，值不变则沿用。
  int? screenCapacity;

  /// 每次刷新最多续拉的次数，防止服务端持续返回很少或大半被过滤时刷请求。
  static const int _maxTopUps = 3;
  int _topUps = 0;
  bool _filling = false;

  /// 本次刷新拿到的新卡数（含续拉）。开着「保留上次数据」时，刷新把新卡
  /// 放在最前、旧卡接在后面，列表总数早就超过一屏；铺满与否必须只看
  /// 新卡，否则刷新后只有顶上约 10 张是新的，其余全是旧卡（#282）。
  int _freshCount = 0;

  int get _loadedCount => switch (loadingState.value) {
    Success(:final List? response) => response?.length ?? 0,
    _ => 0,
  };

  /// 页面上报新的一屏容量。变大时（如折叠屏展开、转屏）立即补齐。
  void updateScreenCapacity(int capacity) {
    if (capacity == screenCapacity) return;
    screenCapacity = capacity;
    _fillFirstScreen();
  }

  Future<void> _fillFirstScreen() async {
    final capacity = screenCapacity;
    if (capacity == null || _filling || !_hasData) return;
    _filling = true;
    try {
      while (!isClosed && _topUps < _maxTopUps && _freshCount < capacity) {
        // 刷新还在进行时等它结束，不与之并发（基类见 isLoading 直接返回）。
        if (isLoading) break;
        _topUps++;
        final before = _loadedCount;
        await queryData(false);
        // 续拉失败或一条没多（基类对加载更多的失败不报错）就停，不重试。
        if (_loadedCount <= before) break;
      }
    } finally {
      _filling = false;
    }
  }

  @override
  Future<void> queryData([bool isRefresh = true]) async {
    if (isRefresh) _topUps = 0;
    await super.queryData(isRefresh);
    if (isRefresh) await _fillFirstScreen();
  }

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
    if (page == 0) {
      _freshCount = dataList.length;
    } else if (_filling) {
      // 续拉的新卡插在本次新卡之后、旧卡之前；基类随后追加的是已清空的
      // dataList，等于只做一次刷新通知。
      if (loadingState.value case Success(:final response?)) {
        response.insertAll(_freshCount, dataList);
        _freshCount += dataList.length;
        dataList.clear();
      }
      return;
    }
    if (enableSaveLastData && page == 0) {
      if (loadingState.value case Success(:final response)) {
        if (response != null && response.isNotEmpty) {
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
