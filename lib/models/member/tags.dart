/// 关注分组在**界面侧**的可变状态。
///
/// ## 为什么它还在
///
/// 协议侧的分组已经整条迁进 Rust（`RustRelation.followUpTags` 返回
/// `FollowTag`）。留下这个类是因为界面要**就地改**它：关注页的 tab 条改完
/// 名字不重新拉一次清单（`FollowController.onUpdateTag`），分组选择面板
/// 勾一个分组就把人数加一（`lib/pages/group_panel/view.dart`）。FRB 生成的
/// `FollowTag` 字段是 final 的，照 ADR-011 也不该为了迁就 GetX 的可变模型
/// 去改导出面——那是宿主的习惯泄进 Rust。
///
/// S6 #56 已经把 `fromJson` 去掉了：当时让它还带着解析的那个生产者是
/// `MemberHttp.getTopVideo`，而那条接口**整个 `lib/` 没有一个调用点**，
/// 返回类型还是错的（`/x/space/top/arc` 给的是置顶稿件，不是关注分组），
/// 于是随本切片一并删除而不是迁移。现在它是一个纯界面状态类，不再有
/// 任何取数职责。
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

  MemberTagItemModel.fromCreate(
    ({int tagid, String tagName}) res, {
    this.count = 0,
  }) {
    tagid = res.tagid;
    name = res.tagName;
  }
}
