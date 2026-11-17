import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/member/archive_order_type_web.dart';
import 'package:kilikili/pages/member_video_web/base/controller.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;
import 'package:kilikili/services/rust/space_web_compat.dart';
import 'package:get/get.dart';

class MemberVideoWebCtr
    extends
        BaseVideoWebCtr<
          rust.SpaceArchiveSearchPage,
          SpaceWebVideoModel,
          ArchiveOrderTypeWeb
        > {
  int? _totalCount;
  @override
  final Rx<ArchiveOrderTypeWeb> order = Rx(.pubdate);

  int tid = 0;
  String? specialType;
  List<rust.SpaceArchiveTag>? tags;

  @override
  List<SpaceWebVideoModel>? getDataList(rust.SpaceArchiveSearchPage response) {
    return response.items.map(SpaceWebVideoModel.fromRust).toList();
  }

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<rust.SpaceArchiveSearchPage> response,
  ) {
    if (isRefresh) {
      final data = response.response;
      if (data.count?.toInt() case final count?) {
        if (tid == 0 && specialType == null) {
          _totalCount = count;
        }
        this.count = count;
        totalPage = (count / ps).ceil();
      }
      if (data.tags.isNotEmpty) {
        // 「全部类型」是本地拼的一条，服务端不下发：它代表「不带筛选
        // 参数」这个状态，总数只有未筛选那次请求才知道。
        tags = [
          rust.SpaceArchiveTag(tid: 0, name: '全部类型', count: _totalCount),
          ...data.tags,
        ];
      }
    }
    return false;
  }

  @override
  Future<LoadingState<rust.SpaceArchiveSearchPage>> customGetData() =>
      rust.RustSpace.archiveSearch(
        mid: mid,
        pageSize: ps,
        page: page,
        order: order.value.kind,
        tid: tid,
        specialType: specialType,
      );
}
