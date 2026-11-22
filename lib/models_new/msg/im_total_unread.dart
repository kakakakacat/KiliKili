import 'package:fixnum/fixnum.dart' show Int64;

/// gRPC `getTotalUnread` 回的那张未读数表。
///
/// 原名 `MsgFeedUnread`，S12 迁移 REST 那一半时改掉：Rust 侧导出的
/// `MsgFeedUnread`（`/x/msgfeed/unread` 的响应）与它同名不同义，一个文件
/// 同时 import 两边就会撞，而撞出来的错很难一眼看出是「两个未读数」。
///
/// 两者的差别不只是传输方式：这张表的值是 `Int64`（protobuf map），而
/// REST 那条是 JSON 整数。私信列表页用这一张，首页红点用那一条。
///
/// gRPC 的那一半（`lib/grpc/im.dart`）仍在 Dart，归 #63 的后续。
class ImTotalUnread {
  int at = 0;
  int like = 0;
  int reply = 0;
  int sysMsg = 0;

  ImTotalUnread.fromJson(Map<String, Int64> json) {
    at = json['at']?.toInt() ?? 0;
    like = json['like']?.toInt() ?? 0;
    reply = json['reply']?.toInt() ?? 0;
    sysMsg = json['sys_msg']?.toInt() ?? 0;
  }
}
