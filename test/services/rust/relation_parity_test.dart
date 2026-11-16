import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 关注分组与关注关系的**一次性基线对照**（S6 #56 / S7 #57，机制见
/// ADR-011 决策五）。
///
/// ## 为什么是逐行转写，而不是 import 基线模型
///
/// 两个原因，各管一半：
///
/// - `RelationData`（`lib/models_new/relation/data.dart`）与
///   `FollowData` / `FollowItemModel`（`lib/models_new/follow/`）在**本 PR
///   里已经删除**——它们的生产者全部迁进了 Rust，照 ADR-011 决策四不留旧
///   实现。被删掉的东西 import 不进来。
/// - `MemberTagItemModel` 还活着（`getTopVideo` 还在 Dart），但它只有四个
///   直取字段，转写与 import 一样长，放在一起反而好对。
///
/// 下面的 `_Baseline*` 是三个 `fromJson` 的逐行转写，取自本 PR 的父提交，
/// 写下之后不再改，也不许顺手修正任何怪异之处。
///
/// ## 这个测试的寿命
///
/// 一次性。基线实现已经不在仓库里，本文件证明的是「删掉它们的那一刻，
/// Rust 的输出与它们逐字段一致」。再往后它只会变成一份与谁都对不上的副本
/// ——**下一个碰这一族的 PR 可以连同它一起删**，长期留在仓库里的是
/// `rust/crates/bili-api/tests/fixtures/relation/*.rust.json` 那几份快照。
///
/// fixture 是手工构造的（原因见 `rust/crates/bili-api/tests/relation_golden.rs`
/// 的文件头：这一族整条要登录态）。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/relation';

  dynamic read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync());

  test('关注分组清单：Rust 快照与基线逐字段一致', () {
    final data = read('follow_tags.raw.json')['data'] as List;
    final snapshot = read('follow_tags.rust.json') as List;
    final baseline = data
        .map((e) => _BaselineTag.fromJson(e as Map<String, dynamic>))
        .toList();

    expect(snapshot.length, baseline.length);
    for (var i = 0; i < baseline.length; i++) {
      final got = snapshot[i] as Map<String, dynamic>;
      expect(got['tagid'], baseline[i].tagid, reason: '第 $i 组 tagid');
      expect(got['name'], baseline[i].name, reason: '第 $i 组名字');
      expect(got['count'], baseline[i].count, reason: '第 $i 组人数');
      expect(got['tip'], baseline[i].tip, reason: '第 $i 组说明');
    }
  });

  test('分组内关注：裸数组也要解析成和基线一样的列表', () {
    // 基线 `MemberHttp.followUpGroup` 对这条接口的写法是
    // `(res.data['data'] as List?)?.map(FollowItemModel.fromJson)`——
    // 外层没有 `{list, total}`，所以它也拿不到总数。
    final data = read('follow_group.raw.json')['data'] as List;
    final snapshot = read('follow_group.rust.json') as Map<String, dynamic>;
    final baseline = data
        .map((e) => _BaselineFollowItem.fromJson(e as Map<String, dynamic>))
        .toList();

    expect(
      snapshot['total'],
      isNull,
      reason: '基线在这条路径上只填了 list，total 无从得来',
    );

    final list = (snapshot['list'] as List).cast<Map<String, dynamic>>();
    expect(list.length, baseline.length);
    for (var i = 0; i < baseline.length; i++) {
      final b = baseline[i];
      expect(list[i]['mid'], b.mid, reason: '第 $i 人 mid');
      expect(list[i]['attribute'], b.attribute, reason: '第 $i 人 attribute');
      expect(list[i]['uname'], b.uname, reason: '第 $i 人昵称');
      expect(list[i]['face'], b.face, reason: '第 $i 人头像');
      expect(list[i]['sign'], b.sign, reason: '第 $i 人签名');

      final verify = list[i]['official_verify'] as Map<String, dynamic>?;
      expect(
        verify?['kind'],
        b.officialVerifyType,
        reason: '第 $i 人认证类型（协议里叫 type，Rust 侧避开关键字叫 kind）',
      );
      expect(verify?['desc'], b.officialVerifyDesc, reason: '第 $i 人认证说明');
    }
  });

  test('缺 mid 落到 0、空签名不归一成 null —— 两处都照基线', () {
    final snapshot = read('follow_group.rust.json') as Map<String, dynamic>;
    final list = (snapshot['list'] as List).cast<Map<String, dynamic>>();
    expect(list[1]['mid'], 0, reason: '基线 FollowItemModel 缺 mid 时填 0');
    expect(
      list[1]['sign'],
      '',
      reason: '空串若归一成 null，界面 FollowItem 会少掉那一行空白',
    );
  });

  test('关注关系：Rust 快照与基线逐字段一致', () {
    for (final name in ['relation', 'relation_no_tag']) {
      final data = read('$name.raw.json')['data'] as Map<String, dynamic>;
      final snapshot = read('$name.rust.json') as Map<String, dynamic>;
      final baseline = _BaselineRelation.fromJson(data);

      expect(snapshot['attribute'], baseline.attribute, reason: '$name attribute');
      expect(snapshot['mtime'], baseline.mtime, reason: '$name mtime');
      expect(snapshot['special'], baseline.special, reason: '$name special');
      expect(snapshot['tag'], baseline.tag, reason: '$name tag');
    }
  });

  test('tag 的 null 与非空在两份 fixture 里保持区分', () {
    // 分组面板靠这个区别决定默认勾选哪一项：没有分组是 null，在默认分组
    // 是 [0]。任何一侧把它们归一成同一个值，面板就会勾错。
    expect(
      (read('relation.rust.json') as Map<String, dynamic>)['tag'],
      [0, 900001],
    );
    expect(
      (read('relation_no_tag.rust.json') as Map<String, dynamic>)['tag'],
      isNull,
    );
  });
}

/// `lib/models/member/tags.dart` 的 `MemberTagItemModel.fromJson` 逐行转写。
class _BaselineTag {
  int? count;
  String? name;
  int? tagid;
  String? tip;

  _BaselineTag.fromJson(Map<String, dynamic> json) {
    count = json['count'];
    name = json['name'];
    tagid = json['tagid'];
    tip = json['tip'];
  }
}

/// 已删除的 `lib/models_new/follow/list.dart` 的 `FollowItemModel.fromJson`
/// 逐行转写。`officialVerify` 那一层被摊平成两个字段——基线那里套的是
/// `BaseOfficialVerify`，而它一路拖进 widget 层，在没打 Flutter SDK 补丁的
/// 机器上编译不过（同 `follow_parity_test.dart` 当初的理由）。
class _BaselineFollowItem {
  int? mid;
  int? attribute;
  String? uname;
  String? face;
  String? sign;
  int? officialVerifyType;
  String? officialVerifyDesc;

  _BaselineFollowItem.fromJson(Map<String, dynamic> json) {
    mid = json['mid'] as int? ?? 0;
    attribute = json['attribute'] as int?;
    uname = json['uname'] as String?;
    face = json['face'] as String?;
    sign = json['sign'] as String?;
    final verify = json['official_verify'] as Map<String, dynamic>?;
    officialVerifyType = verify?['type'] as int?;
    officialVerifyDesc = verify?['desc'] as String?;
  }
}

/// 已删除的 `lib/models_new/relation/data.dart` 的 `RelationData.fromJson`
/// 逐行转写。基线那里的 `tag` 用了 `fromCast()`（`iterable_ext.dart`），
/// 效果等同 `cast<int>().toList()`。
class _BaselineRelation {
  int? attribute;
  int? mtime;
  List<int>? tag;
  int? special;

  _BaselineRelation.fromJson(Map<String, dynamic> json)
    : attribute = json['attribute'] as int?,
      mtime = json['mtime'] as int?,
      tag = (json['tag'] as List<dynamic>?)?.cast<int>().toList(),
      special = json['special'] as int?;
}
