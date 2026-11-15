import 'package:kilikili/utils/parse_int.dart';

// 文章详情 JSON 仍内嵌投票摘要；随文章详情接口迁入 Rust 时删除。
class SimpleVoteInfo {
  int? choiceCnt;
  int? defaultShare;
  String? desc;
  int? endTime;
  int? status;
  int? uid;
  int? voteId;
  late int joinNum;

  SimpleVoteInfo({
    this.choiceCnt,
    this.defaultShare,
    this.desc,
    this.endTime,
    this.status,
    this.uid,
    this.voteId,
    this.joinNum = 0,
  });

  SimpleVoteInfo.fromJson(Map<String, dynamic> json) {
    choiceCnt = safeToInt(json['choice_cnt']);
    defaultShare = safeToInt(json['default_share']);
    desc = json['desc'];
    endTime = safeToInt(json['end_time']);
    status = safeToInt(json['status']);
    uid = safeToInt(json['uid']);
    voteId = safeToInt(json['vote_id']);
    joinNum = safeToInt(json['join_num']) ?? 0;
  }
}
