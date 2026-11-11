import 'package:kilikili/rust/api/pgc.dart' show PgcReviewItem;

/// 评论列表的容器。解析已在 Rust，见 S10 第一批（issue #60）。
///
/// `count` 的来源由 Rust 决定：长评给 `count`，短评给 `total`，与基线的
/// `json['count'] ?? json['total']` 一致。
class PgcReviewData {
  List<PgcReviewItem>? list;
  String? next;
  int? count;

  PgcReviewData({this.list, this.next, this.count});
}
