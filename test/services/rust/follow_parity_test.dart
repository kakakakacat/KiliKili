import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 关注/粉丝列表的**一次性基线对照**（#66 收尾，机制见 ADR-011 决策五）。
///
/// ## 为什么是逐行转写，而不是直接 import 基线模型
///
/// 按 ADR-011 补充二的判据，基线模型在本 PR 之后仍活在 `lib/` 里
/// （`FollowData` / `FollowItemModel` 还有四个生产者，见下），这种情形本该
/// 像 `sponsor_block_parity_test.dart` 那样直接借用活着的模型。这里**做不到**：
///
/// `FollowItemModel` → `models/model_avatar.dart` → `models/model_owner.dart`
/// → `models/model_video.dart` / `models_new/dynamic/dyn_mention/item.dart`，
/// 一路拖进 widget 层。那些符号由 `lib/scripts/*.patch` 打进 Flutter SDK，
/// 没跑过 `tool/prepare-build.sh` 的机器上编译不过，测试只能在 CI 跑——
/// 而 parity 恰恰是写迁移代码时要反复跑的那一个。
///
/// 所以照 S9 的做法转写：下面的 `_Baseline*` 是 `lib/models_new/follow/`
/// 两个 `fromJson` 的**逐行转写**（取自本 PR 的 `main`，`0bbeb142e`），
/// 写下之后不再改，也不许顺手修正任何怪异之处。
///
/// ## 这个测试的寿命
///
/// 删除时机仍按补充二：S6 #56（`followUpGroup` / `getfollowSearch`）与
/// S7 #57（`followedUp` / `sameFollowing`）把另外四个生产者迁完、
/// `lib/models_new/follow/` 整个删掉的那个 PR，应当连同本文件与
/// `lib/services/rust/follow_compat.dart` 一起删。长期留在仓库里的是
/// `follow_list.rust.json` 快照。
///
/// fixture 是**手工构造**的（原因见 `rust/crates/bili-api/tests/misc_golden.rs`
/// 的文件头：关注/粉丝列表要登录态，脱敏之后剩下的就是一串示例用户）。
/// 三条样本分别对着基线的三个分支：有签名 / 空签名 / 缺 mid。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/misc';

  Map<String, dynamic> read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as Map<String, dynamic>;

  test('关注/粉丝列表：Rust 快照与基线逐字段一致', () {
    final data = read('follow_list.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('follow_list.rust.json');
    final baseline = _BaselineFollowData.fromJson(data);

    expect(snapshot['total'], baseline.total);

    final list = (snapshot['list'] as List).cast<Map<String, dynamic>>();
    expect(list.length, baseline.list?.length ?? 0);

    for (var i = 0; i < list.length; i++) {
      final b = baseline.list![i];
      expect(list[i]['mid'], b.mid, reason: '第 $i 人 mid');
      expect(list[i]['attribute'], b.attribute, reason: '第 $i 人 attribute');
      expect(list[i]['uname'], b.uname, reason: '第 $i 人昵称');
      expect(list[i]['face'], b.face, reason: '第 $i 人头像');
      expect(list[i]['sign'], b.sign, reason: '第 $i 人签名');

      final verify = list[i]['official_verify'] as Map<String, dynamic>?;
      expect(
        verify?['kind'],
        b.officialVerify?.type,
        reason: '第 $i 人认证类型（协议里叫 type，Rust 侧避开关键字叫 kind）',
      );
      expect(verify?['desc'], b.officialVerify?.desc, reason: '第 $i 人认证说明');
    }
  });

  test('缺 mid 落到 0、空签名不归一成 null —— 两处都照基线', () {
    final data = read('follow_list.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('follow_list.rust.json');
    final baseline = _BaselineFollowData.fromJson(data);
    final list = (snapshot['list'] as List).cast<Map<String, dynamic>>();

    expect(baseline.list![1].sign, '', reason: 'fixture 里第二条就是空签名');
    expect(
      list[1]['sign'],
      '',
      reason: '空串若归一成 null，界面 FollowItem 会少掉那一行空白',
    );

    expect(
      baseline.list![2].mid,
      0,
      reason: '基线 FollowItemModel 缺 mid 时填 0',
    );
    expect(list[2]['mid'], 0);
  });
}

// --------------------------------------------------------------- 基线转写
//
// 取自 `lib/models_new/follow/data.dart` 与 `lib/models_new/follow/list.dart`
// （`main` @ 0bbeb142e）。基线的 `FollowItemModel extends UpItem`，继承来的
// `mid` / `uname` / `face` 在这里摊平成普通字段——`UpItem` 是界面基类，拖进
// 测试没有任何对照价值。`hasUpdate` 这条接口不给，基线也没读，略去。

class _BaselineFollowData {
  _BaselineFollowData({required this.list, this.total});

  final List<_BaselineFollowItem>? list;
  final int? total;

  factory _BaselineFollowData.fromJson(Map<String, dynamic> json) =>
      _BaselineFollowData(
        list: (json['list'] as List<dynamic>?)
            ?.map(
              (e) => _BaselineFollowItem.fromJson(e as Map<String, dynamic>),
            )
            .toList(),
        total: json['total'] as int?,
      );
}

class _BaselineFollowItem {
  _BaselineFollowItem({
    required this.mid,
    this.attribute,
    this.uname,
    this.face,
    this.sign,
    this.officialVerify,
  });

  final int mid;
  final int? attribute;
  final String? uname;
  final String? face;
  final String? sign;
  final _BaselineOfficialVerify? officialVerify;

  factory _BaselineFollowItem.fromJson(Map<String, dynamic> json) =>
      _BaselineFollowItem(
        mid: json['mid'] as int? ?? 0,
        attribute: json['attribute'] as int?,
        uname: json['uname'] as String?,
        face: json['face'] as String?,
        sign: json['sign'] as String?,
        officialVerify: json['official_verify'] == null
            ? null
            : _BaselineOfficialVerify.fromJson(
                json['official_verify'] as Map<String, dynamic>,
              ),
      );
}

class _BaselineOfficialVerify {
  _BaselineOfficialVerify.fromJson(Map<String, dynamic> json)
    : type = json['type'] as int?,
      desc = json['desc'] as String?;

  final int? type;
  final String? desc;
}
