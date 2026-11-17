import 'package:kilikili/services/rust/rust_space.dart' as rust;

/// 空间页上的一条预约，**界面侧的视图模型**（S6 #56）。
///
/// 预约/取消预约会就地改「已预约人数」与按钮文案，而 Rust 给的结构是
/// 不可变的（FRB 生成的字段都是 final）。这两个值本来也不是那条接口的
/// 事实——它们会被另一条写接口改掉，按 AGENTS.md §2 属于界面状态。
/// 其余字段只读，直接转发 [raw]。
class MemberReserveItem {
  MemberReserveItem(this.raw)
    : total = raw.total.toInt(),
      isFollow = raw.isFollow;

  final rust.SpaceReservation raw;

  /// 已预约人数。预约成功后 +1、取消后 -1。
  int total;

  /// 当前账号预约了没有。
  bool isFollow;

  int? get sid => raw.sid?.toInt();
  String? get name => raw.name;
  String? get descText => raw.descText;
  String? get dynamicId => raw.dynamicId;
  rust.SpaceReservationLottery? get lottery => raw.lottery;
}
