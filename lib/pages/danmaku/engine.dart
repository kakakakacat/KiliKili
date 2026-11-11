/// 基线的弹幕处理：标记自己 → 合并 → 过滤 → 按 100ms 分桶。
///
/// 这段逻辑原本写在 `PlDanmakuController.handleDanmaku` 里，逐字搬到这里，
/// **行为未作任何改动**。搬出来有两个原因：
///
/// 1. Rust 引擎（`rust/crates/bili-danmaku`）要与它对齐，而对齐靠的是同一
///    份 fixture 两边各跑一遍（`tool/fixture-gen/danmaku_engine_fixture.dart`）。
///    生成器是纯 dart 脚本，不能 import flutter，也就无法构造
///    `PlPlayerController`——逻辑留在控制器里就没法让**基线自己**产出
///    golden，只能靠手写期望值，那证明不了任何事。
/// 2. 它本来就不依赖控制器状态：需要的四个输入（登录态、自己的 midHash、
///    合并开关、屏蔽规则）都是会话内不变的量。
///
/// 本文件**不得** import flutter。
library;

import 'dart:collection';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/models/user/danmaku_rule.dart';

/// 把一批弹幕并入 [into]（键为 `progress ~/ 100`）。
///
/// 会**就地修改**传入的弹幕：写入 `isSelf`（登录态下）与 `count`（合并
/// 开启时）。这两个字段是上游手工加进 protobuf 生成代码的客户端字段，
/// 服务端从不下发，由此处产生。
///
/// 顺序不能错：合并在过滤之前，因此被过滤掉的内容也会占住合并表——首条
/// 命中规则被丢弃后，后续同内容的弹幕撞上合并表直接跳过。
void bucketDanmaku(
  List<DanmakuElem> elems, {
  required Map<int, List<DanmakuElem>> into,
  required bool isLogin,
  required String midHash,
  required bool mergeDanmaku,
  required RuleFilter filters,
}) {
  if (elems.isEmpty) return;
  final uniques = HashMap<String, DanmakuElem>();

  final shouldFilter = filters.count != 0;
  for (final element in elems) {
    if (isLogin) {
      element.isSelf = element.midHash == midHash;
    }

    if (!element.isSelf) {
      if (mergeDanmaku) {
        final elem = uniques[element.content];
        if (elem == null) {
          uniques[element.content] = element..count = 1;
        } else {
          elem.count++;
          continue;
        }
      }

      if (shouldFilter && filters.remove(element)) {
        continue;
      }
    }

    final int pos = element.progress ~/ 100; //每0.1秒存储一次
    (into[pos] ??= []).add(element);
  }
}
