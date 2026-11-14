// ignore_for_file: constant_identifier_names, non_constant_identifier_names

import 'dart:convert' show ascii, base64;

import 'package:kilikili/utils/utils.dart';
import 'package:collection/collection.dart';
import 'package:uuid/v4.dart';

abstract final class IdUtils {
  static const XOR_CODE = 23442827791579;
  static const MASK_CODE = 2251799813685247;
  static const MAX_AID = 1 << 51;
  static const BASE = 58;

  static const data =
      'FcwAPNKTMug3GV5Lj7EJnHpWsx4tb8haYeviqBz6rkCy12mUSDQX9RdoZf';
  static final invData = {for (final (i, c) in data.codeUnits.indexed) c: i};

  static final bvRegex = RegExp(r'bv1[0-9a-zA-Z]{9}', caseSensitive: false);
  static final bvRegexExact = RegExp(
    r'^bv1[0-9a-zA-Z]{9}$',
    caseSensitive: false,
  );
  static final avRegex = RegExp(r'av(\d+)', caseSensitive: false);
  static final avRegexExact = RegExp(r'^av(\d+)$', caseSensitive: false);
  static final digitOnlyRegExp = RegExp(r'^\d+$');

  /// av转bv
  ///
  /// ## Rust 侧还有一份，这是有意的（S4 / issue #54）
  ///
  /// `bili_protocol::bvid` 复刻了这里的 `av2bv` / `bv2av`。两份并存的理由
  /// 在 ADR-011 补充十八：视频页的写操作参数是 `aid` 而调用点只有 `bvid`，
  /// 而这段编码**算错一位不会报错，只会让写操作落到另一个稿件上**——
  /// 点赞点到别人头上。那条路径上不能靠宿主转。
  ///
  /// **为什么这一份还留着**：`IdUtils` 被几十处尚未迁移的 Dart 路径用着，
  /// 按切片边界它不属于任何一片。
  ///
  /// **随谁删**：最后一个 Dart 调用点（预计是 Phase 2 清 `lib/http` 残留
  /// 那一轮），不是某个单独的切片。并存期间由
  /// `rust/crates/bili-protocol/tests/bvid_golden.rs` 对照同一组样本钉住，
  /// 期望值由 `tool/fixture-gen/bvid_fixture.dart` 从本文件产出——**改了
  /// 下面这两个函数或字母表，要重新生成那份 fixture**，否则 CI 会红。
  static String av2bv(int aid) {
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

  /// bv转av
  static int bv2av(String bvid) {
    final bvidArr = bvid.codeUnits.sublist(3)
      ..swap(0, 6)
      ..swap(1, 4);

    final tmp = bvidArr.fold(0, (pre, char) => pre * BASE + invData[char]!);
    return (tmp & MASK_CODE) ^ XOR_CODE;
  }

  // 匹配
  static AvBvRes matchAvorBv({String? input}) {
    if (input == null || input.isEmpty) {
      return const (av: null, bv: null);
    }
    String? bvid = bvRegex.firstMatch(input)?.group(0);

    late String? aid = avRegex.firstMatch(input)?.group(1);

    if (bvid != null) {
      return (av: null, bv: bvid);
    } else if (aid != null) {
      return (av: int.parse(aid), bv: null);
    }
    return const (av: null, bv: null);
  }

  static String genBuvid3() {
    return '${const UuidV4().generate().toUpperCase()}${Utils.random.nextInt(100000).toString().padLeft(5, "0")}infoc';
  }

  static String genAuroraEid(int uid) {
    if (uid == 0) {
      return '';
    }

    final midByte = ascii.encode(uid.toString());

    const key = 'ad1va46a7lza';
    for (int i = 0; i < midByte.length; i++) {
      midByte[i] ^= key.codeUnitAt(i % key.length);
    }

    String base64Encoded = base64.encode(midByte).replaceAll('=', '');

    return base64Encoded;
  }

  // https://github.com/SocialSisterYi/bilibili-API-collect/blob/master/grpc_api/readme.md#x-bili-trace-id-生成算法
  static String genTraceId() {
    final randomTraceId = StringBuffer(Utils.generateRandomString(24));

    final ts = (DateTime.now().millisecondsSinceEpoch ~/ 1000) >> 8;

    randomTraceId
      ..write((ts & 0xFFFFFF).toRadixString(16).padLeft(6, '0'))
      ..write(Utils.generateRandomString(2));

    return '${randomTraceId.toString()}:${randomTraceId.toString().substring(16, 32)}:0:0';
  }
}

typedef AvBvRes = ({int? av, String? bv});

extension AvBvExt on AvBvRes {
  bool get isNotEmpty => this != const (av: null, bv: null);
}
