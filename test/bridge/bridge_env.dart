import 'dart:io';

import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hive_ce/hive.dart';

/// `test/bridge/` 是仓库里**唯一**真正加载原生库、跨 FFI 执行的测试。
///
/// ## 为什么要有这组测试
///
/// 其余 Dart 测试都跑在没有原生库的进程里：`RustLib.init()` 找不到
/// 动态库，`RustCore.isReady` 恒为 false，于是每个 `RustFlags` 开关都
/// 返回 false，所有 Rust 路径直接短路。也就是说在这组测试之前，CI 验的
/// 一直是**开关关闭**下的 Dart 原路径——桥接层（FRB 的类型映射、会话
/// 生命周期、失败降级）从未被执行过一次。
///
/// Rust 侧的单测与 golden 证明的是「引擎算得对」，这里证明的是另一件事：
/// **那个结果确实能跨过 FFI 原样回到 Dart**，且开关打开后调用方拿到的
/// 就是它。两者都绿，迁移才算有回归保护。
///
/// ## 为什么用环境变量而不是 tag 或文件名约定
///
/// 这组测试需要先 `cargo build -p bridge_flutter --release`，产物路径由
/// FRB 生成的 `kDefaultExternalLibraryLoaderConfig` 决定
/// （`rust/crates/bridge_flutter/target/release/`）。开发机上通常没有这份
/// 产物，默认 `flutter test` 必须照常全绿，所以默认**跳过**。
///
/// 但「跳过」本身是危险的：这组测试存在的全部意义就是别让桥接层继续
/// 没人跑，而最容易发生的退化恰恰是 CI 里它悄悄变成 0 个测试。因此
/// [KILIKILI_RUST_BRIDGE_TESTS] 一旦为 1，原生库没加载起来就**直接失败**，
/// 不再跳过。CI 的 bridge job 设这个变量，它是「这次真的跑了」的唯一凭据。
const String bridgeEnvVar = 'KILIKILI_RUST_BRIDGE_TESTS';

/// CI 的 bridge job 会把它设成 1。
final bool bridgeTestsRequested = Platform.environment[bridgeEnvVar] == '1';

const String _skipReason =
    '需要 $bridgeEnvVar=1 且已编出原生库：'
    'cargo build -p bridge_flutter --release';

/// 传给 `test(..., skip: bridgeSkip)`。
final Object bridgeSkip = bridgeTestsRequested ? false : _skipReason;

Directory? _hiveDir;

/// 加载原生库并把 `RustFlags` 读的那个 Hive box 准备好。
///
/// 不走 `GStorage.init()`：那会牵出 `path_provider`、账号、六个 box 与
/// 一堆 adapter，而开关只用到 `setting` 一个。这里直接给它一个临时目录
/// 上的空 box——开关的默认值因此是「全关」，与真机首次启动一致，每个
/// 测试要开哪个自己开。
Future<void> initRustBridge() async {
  TestWidgetsFlutterBinding.ensureInitialized();

  final dir = Directory.systemTemp.createTempSync('kilikili-bridge-');
  _hiveDir = dir;
  Hive.init(dir.path);
  GStorage.setting = await Hive.openBox('setting');

  await RustCore.init();
  if (!RustCore.isReady) {
    fail(
      '原生库没加载起来，桥接测试无法进行：${RustCore.error}\n'
      '先跑 cargo build -p bridge_flutter --release，产物须落在 '
      'rust/crates/bridge_flutter/target/release/',
    );
  }
}

/// 与 [initRustBridge] 配对，在 `tearDownAll` 里调用。
Future<void> closeRustBridge() async {
  await Hive.close();
  try {
    _hiveDir?.deleteSync(recursive: true);
  } catch (_) {
    // 临时目录删不掉不是测试的结论，CI runner 用完即弃。
  }
}
