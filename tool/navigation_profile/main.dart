import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/main.dart' as app;
import 'package:kilikili/models/common/nav_bar_config.dart';
import 'package:kilikili/navigation_profile.dart';
import 'package:kilikili/pages/dynamics/controller.dart';
import 'package:kilikili/pages/dynamics_tab/controller.dart';
import 'package:kilikili/pages/main/controller.dart';
import 'package:kilikili/pages/mine/controller.dart';
import 'package:kilikili/services/account_service.dart';
import 'package:material_ui/material_ui.dart';

Future<void> main() async {
  if (!kProfileMode) throw StateError('只接受 profile/AOT 包，debug 不可用于性能结论');
  await app.main();
  WidgetsBinding.instance.addObserver(_ProfileLifecycle());
  SchedulerBinding.instance.addTimingsCallback(NavigationProfile.frames);
  // 等首页首帧与启动请求完成；四组都使用同一预热时间。
  await Future<void>.delayed(const Duration(seconds: 6));
  final main = Get.find<MainController>();
  final login = Get.find<AccountService>().isLogin.value;
  NavigationProfile.event('ready', {'login': login});
  // 通过 Android 启动参数指定目标，避免重编包。首轮每个进程只访问一个目标页。
  const channel = MethodChannel('navigation_profile');
  final name = await channel.invokeMethod<String>('target') ?? 'dynamics';
  final nav = name == 'mine'
      ? NavigationBarType.mine
      : NavigationBarType.dynamics;
  NavigationProfile.target = main.navigationBars.indexOf(nav);
  NavigationProfile.settled = () => main.controller is PageController
      ? (main.controller as PageController).page == NavigationProfile.target
      : (main.controller as TabController).animation!.value ==
            NavigationProfile.target;

  SchedulerBinding.instance.addPersistentFrameCallback((_) {
    // PageView 复用已合成的页面层时不会再次调用子树 paint，仍需记录到位帧。
    NavigationProfile.sampleFrame();
  });
  for (var i = 0; i < 3; i++) {
    NavigationProfile.sample = i;
    NavigationProfile.armed = true;
    NavigationProfile.event('sample', {
      'login': login,
      'first': i == 0,
      'name': name,
    });
    main.setIndex(NavigationProfile.target);
    final deadline = DateTime.now().add(const Duration(seconds: 20));
    while (NavigationProfile.armed &&
        !NavigationProfile.fullPainted &&
        DateTime.now().isBefore(deadline)) {
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }
    while (NavigationProfile.armed &&
        !_dataReady(nav, login) &&
        DateTime.now().isBefore(deadline)) {
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }
    if (!NavigationProfile.armed) return;
    if (!NavigationProfile.fullPainted || !_dataReady(nav, login)) {
      NavigationProfile.event('timeout', {'state': _state(nav, login)});
      NavigationProfile.armed = false;
      return;
    }
    NavigationProfile.event('data_terminal', {'state': _state(nav, login)});
    await Future<void>.delayed(const Duration(seconds: 1));
    NavigationProfile.armed = false;
    main.setIndex(main.navigationBars.indexOf(NavigationBarType.home));
    await Future<void>.delayed(const Duration(seconds: 1));
  }
  NavigationProfile.event('done');
}

DynamicsTabController? _dynamicTab() {
  final controller = Get.find<DynamicsController>();
  return controller.controller;
}

bool _dataReady(NavigationBarType nav, bool login) {
  if (nav == NavigationBarType.mine) {
    if (!login) return true;
    return NavigationProfile.minePending.isEmpty &&
        !Get.find<MineController>().tabCtr().any((c) => c.isLoading);
  }
  final tab = _dynamicTab();
  return tab != null &&
      tab.loadingState.value is! Loading &&
      !Get.find<DynamicsController>().isLoading;
}

String _state(NavigationBarType nav, bool login) {
  if (nav == NavigationBarType.mine)
    return login ? 'mine_lists_terminal' : 'anonymous_no_request';
  return _dynamicTab()?.loadingState.value.runtimeType.toString() ??
      'not_mounted';
}

class _ProfileLifecycle extends WidgetsBindingObserver {
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (NavigationProfile.armed && state != AppLifecycleState.resumed) {
      NavigationProfile.event('interrupted', {'lifecycle': state.name});
      NavigationProfile.armed = false;
    }
  }
}
