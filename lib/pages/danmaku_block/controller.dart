import 'dart:convert';

import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/dm_block_type.dart';
import 'package:kilikili/services/rust/rust_misc.dart';
import 'package:archive/archive.dart' show getCrc32;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class DanmakuBlockController extends GetxController
    with GetSingleTickerProviderStateMixin {
  late final List<RxList<FilterRule>> rules = List.generate(
    DmBlockType.values.length,
    (_) => <FilterRule>[].obs,
  );

  late TabController tabController;

  @override
  void onInit() {
    super.onInit();
    queryDanmakuFilter();
    tabController = TabController(length: 3, vsync: this);
  }

  @override
  void onClose() {
    tabController.dispose();
    super.onClose();
  }

  Future<void> queryDanmakuFilter() async {
    SmartDialog.showLoading(msg: '正在同步弹幕屏蔽规则……');
    final result = await RustMisc.danmakuFilter();
    SmartDialog.dismiss();
    if (result case Success(:final response)) {
      // 按 type 分组在 Rust 侧完成（协议知识，ADR-011 决策二），这里只是
      // 把三组接到三个 Tab 上，顺序与 DmBlockType 一致。
      rules[0].addAll(response.keywords);
      rules[1].addAll(response.patterns);
      rules[2].addAll(response.users);
      if (response.toast case final toast?) {
        SmartDialog.showToast(toast);
      }
    } else {
      result.toast();
    }
  }

  Future<void> danmakuFilterDel(int tabIndex, int itemIndex, int id) async {
    SmartDialog.showLoading(msg: '正在删除弹幕屏蔽规则……');
    final res = await RustMisc.danmakuFilterDel(ids: id);
    SmartDialog.dismiss();
    if (res.isSuccess) {
      rules[tabIndex].removeAt(itemIndex);
      SmartDialog.showToast('删除成功');
    } else {
      res.toast();
    }
  }

  Future<void> danmakuFilterAdd({
    required String filter,
    required int type,
  }) async {
    if (type == 2) {
      filter = getCrc32(ascii.encode(filter), 0).toRadixString(16);
    }
    SmartDialog.showLoading(msg: '正在添加弹幕屏蔽规则……');
    final res = await RustMisc.danmakuFilterAdd(filter: filter, type: type);
    SmartDialog.dismiss();
    if (res case Success(:final response)) {
      rules[type].add(response);
      SmartDialog.showToast('添加成功');
    } else {
      res.toast();
    }
  }
}
