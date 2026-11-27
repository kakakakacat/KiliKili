import 'dart:async' show Completer;

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

  // ---- 一次刷新至少一屏（#263、#282）----
  //
  // 默认的 app 推荐接口没有 `ps` 参数，每次约 10 条，大屏一屏放不下。
  // 刷新时连续请求，把结果先收进缓冲队列，攒够「一屏容量」（或达到请求
  // 上限）后一次性换上列表：用户看到的是整屏同时刷新，不会先出现 10 张、
  // 再陆续插入。下拉刷新的转圈持续到换上为止。
  //
  // 为什么留在宿主：推荐流还没迁到 Rust（`lib/http/video.dart`）。「攒够
  // 多少、最多请求几次」是阈值与重试策略，按 AGENTS.md §2 应该在 Rust；
  // 推荐切片迁移时把它做成 Rust 接口的参数（如 `min_count`），并删掉这一段。
  // 「一屏容量」依赖布局，那部分留在宿主（见 view 的 `_reportCapacity`）。

  /// 一屏可容纳的卡片数，由页面按网格布局算出；视口尺寸、dpi、列宽设置
  /// 变化时页面会重新上报，值不变则沿用。
  int? screenCapacity;
  final _capacityReady = Completer<int>();

  /// 一次刷新最多请求的次数，防止服务端持续返回很少或大半被过滤时刷请求。
  static const int _maxRequests = 4;

  /// 首次加载时页面还没上报容量，最多等这么久，超时就按单次请求处理。
  static const Duration _capacityWait = Duration(milliseconds: 500);

  int get _loadedCount => switch (loadingState.value) {
    Success(:final List? response) => response?.length ?? 0,
    _ => 0,
  };

  /// 页面上报新的一屏容量。变大时（如折叠屏展开、转屏）列表不够一屏就
  /// 接着往后加载，补满为止（这时不是刷新，追加在末尾即可）。
  void updateScreenCapacity(int capacity) {
    if (capacity == screenCapacity) return;
    screenCapacity = capacity;
    if (!_capacityReady.isCompleted) {
      _capacityReady.complete(capacity);
      return;
    }
    _topUpAfterResize(capacity);
  }

  Future<void> _topUpAfterResize(int capacity) async {
    for (var i = 0; i < _maxRequests; i++) {
      if (isClosed || isLoading || !_hasData || _loadedCount >= capacity) {
        return;
      }
      final before = _loadedCount;
      await queryData(false);
      if (_loadedCount <= before) return;
    }
  }

  @override
  Future<void> queryData([bool isRefresh = true]) async {
    if (!isRefresh) return super.queryData(false);
    if (isLoading) return;
    isLoading = true;
    try {
      final capacity =
          screenCapacity ??
          await _capacityReady.future.timeout(
            _capacityWait,
            onTimeout: () => 0,
          );
      // 缓冲队列：攒够一屏前不动列表。
      final fresh = [];
      LoadingState? failure;
      for (var i = 0; i < _maxRequests; i++) {
        final res = await customGetData();
        if (isClosed) return;
        if (res case Success(:final response)) {
          final list = getDataList(response);
          if (list == null || list.isEmpty) break;
          fresh.addAll(list);
          page++;
          if (fresh.length >= capacity) break;
        } else {
          failure = res;
          break;
        }
      }
      if (fresh.isEmpty) {
        // 一条也没拿到：与基类刷新同样处理（保留上次数据时吞掉错误）。
        final error = failure is Error ? failure : null;
        if (!handleError(error?.errMsg)) {
          loadingState.value = error ?? Success(fresh);
        }
        return;
      }
      // 中途失败也用已攒到的部分，不因后几次请求失败丢掉整次刷新。
      _appendLastData(fresh);
      loadingState.value = Success(fresh);
    } finally {
      isLoading = false;
    }
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

  /// 「保留上次数据」：新卡在前，旧卡接在后面。
  void _appendLastData(List fresh) {
    if (!enableSaveLastData) return;
    if (loadingState.value case Success(:final response)) {
      if (response != null && response.isNotEmpty) {
        if (response.length > 200) {
          fresh.addAll(response.take(50));
        } else {
          fresh.addAll(response);
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
