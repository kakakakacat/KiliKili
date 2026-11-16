// 生成 BV 号 ↔ av 号 golden fixture：从 Dart 基线实现产出期望值，供 Rust
// 侧对照（ADR-008 决策三）。
//
// 本文件**逐字复制** lib/utils/id_utils.dart 的 bv2av / av2bv 与它们用到的
// 三个常量、字母表与 swap 扩展。复制而非 import，是为了让 fixture 反映基线
// 此刻的真实行为：上游若改了字母表或掩码，这里会与 Rust 产生差异，从而
// 在 CI 上暴露出来。
//
// 这一面为什么要有 golden：BV 号是服务端定义的一种 av 号编码，算错一位
// 不会报错，只会让请求指向**另一个稿件**——点赞点到别人头上
// （见 bili-protocol/src/bvid.rs）。
//
// 用法: dart tool/fixture-gen/bvid_fixture.dart > rust/crates/bili-protocol/tests/fixtures/bvid.json
import 'dart:convert' show JsonEncoder;

// ---- 以下为 lib/utils/id_utils.dart 的逐字复制 ----

const XOR_CODE = 23442827791579;
const MASK_CODE = 2251799813685247;
const MAX_AID = 1 << 51;
const BASE = 58;

const data = 'FcwAPNKTMug3GV5Lj7EJnHpWsx4tb8haYeviqBz6rkCy12mUSDQX9RdoZf';
final invData = {for (final (i, c) in data.codeUnits.indexed) c: i};

extension<T> on List<T> {
  void swap(int i, int j) {
    final tmp = this[i];
    this[i] = this[j];
    this[j] = tmp;
  }
}

String av2bv(int aid) {
  final bytes = ['B', 'V', '1', '0', '0', '0', '0', '0', '0', '0', '0', '0'];
  int bvIndex = bytes.length - 1;
  int tmp = (MAX_AID | aid) ^ XOR_CODE;
  while (tmp > 0) {
    bytes[bvIndex--] = data[tmp % BASE];
    tmp ~/= BASE;
  }

  bytes
    ..swap(3, 9)
    ..swap(4, 7);

  return bytes.join();
}

int bv2av(String bvid) {
  final bvidArr = bvid.codeUnits.sublist(3)
    ..swap(0, 6)
    ..swap(1, 4);

  final tmp = bvidArr.fold(0, (pre, char) => pre * BASE + invData[char]!);
  return (tmp & MASK_CODE) ^ XOR_CODE;
}

// ---- 复制到此为止 ----

void main() {
  // 取值覆盖三类：历史最小号（av2，公开可查）、仓库既有 fixture 用过的
  // 稿件号、以及接近 51 位上限的边界值——进位链最长的那一端最容易暴露
  // 掩码或进制写错。
  const aids = <int>[
    2,
    1,
    57,
    58,
    366088782,
    80433022,
    170001,
    1000000000,
    2251799813685246,
  ];

  final cases = [
    for (final aid in aids) {'aid': aid, 'bvid': av2bv(aid)},
  ];

  // 反向也走一遍，确保 fixture 自身是自洽的：生成器自己算错了方向的话，
  // Rust 侧照着它改只会一起错。
  for (final c in cases) {
    final back = bv2av(c['bvid'] as String);
    if (back != c['aid']) {
      throw StateError('生成器自相矛盾: ${c['bvid']} -> $back != ${c['aid']}');
    }
  }

  print(
    const JsonEncoder.withIndent('  ').convert({
      'protocol_version': 1,
      'alphabet': data,
      'cases': cases,
    }),
  );
}
