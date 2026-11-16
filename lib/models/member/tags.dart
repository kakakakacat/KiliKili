/// 关注分组在**界面侧**的可变状态。
///
/// ## 为什么它没随 S6 #56 一起删
///
/// 协议侧的分组已经整条迁进 Rust（`RustRelation.followUpTags` 返回
/// `FollowTag`）。留下这个类是因为界面要**就地改**它：关注页的 tab 条改完
/// 名字不重新拉一次清单（`FollowController.onUpdateTag`），分组选择面板
/// 勾一个分组就把人数加一（`lib/pages/group_panel/view.dart`）。FRB 生成的
/// `FollowTag` 字段是 final 的，照 ADR-011 也不该为了迁就 GetX 的可变模型
/// 去改导出面——那是宿主的习惯泄进 Rust。
///
/// 按 AGENTS.md「例外要写下来」：**随 up 主置顶稿件（`getTopVideo`）迁完
/// 的那一片删**。那条接口是本类当前的另一个生产者，也是它还带着 `fromJson`
/// 的唯一理由；两边都走掉之后，这里应当收成一个只有构造函数的纯界面状态
/// 类（或直接并进关注页的控制器）。
class MemberTagItemModel {
  MemberTagItemModel({
    this.count,
    this.name,
    this.tagid,
    this.tip,
  });

  int? count;
  String? name;
  int? tagid;
  String? tip;

  MemberTagItemModel.fromJson(Map<String, dynamic> json) {
    count = json['count'];
    name = json['name'];
    tagid = json['tagid'];
    tip = json['tip'];
  }

  MemberTagItemModel.fromCreate(
    ({int tagid, String tagName}) res, {
    this.count = 0,
  }) {
    tagid = res.tagid;
    name = res.tagName;
  }
}
