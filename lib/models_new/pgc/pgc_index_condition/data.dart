import 'package:kilikili/rust/api/pgc.dart'
    show PgcConditionFilter, PgcConditionOrder;

/// 索引页筛选条件的容器。解析已在 Rust，见 S10 第一批（issue #60）。
class PgcIndexConditionData {
  List<PgcConditionFilter>? filter;
  List<PgcConditionOrder>? order;

  PgcIndexConditionData({this.filter, this.order});
}
