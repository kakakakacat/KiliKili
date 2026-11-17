import 'dart:convert';
import 'dart:typed_data';

import 'package:kilikili/models/model_owner.dart';
import 'package:kilikili/models/user/danmaku_rule_adapter.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account_adapter.dart';
import 'package:kilikili/utils/accounts/account_type_adapter.dart';
import 'package:kilikili/utils/accounts/cookie_jar_adapter.dart';
import 'package:kilikili/utils/path_utils.dart';
import 'package:kilikili/utils/set_int_adapter.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:hive_ce/hive.dart';
import 'package:path/path.dart' as path;

abstract final class GStorage {
  /// 登录用户信息的本地缓存，存的是**一段 JSON**（S7 #57）。
  ///
  /// 基线存的是 `UserInfoData` 对象，靠 Hive 适配器序列化。typed 直连之后
  /// 导出结构不可变，适配器无从生成，所以改存 JSON，序列化由 Rust 负责
  /// （`RustUser.cacheUserInfo`）。
  ///
  /// **盒名换了**（`userInfo` → `userInfoJson`）：老盒里是 typeId 4/5 的
  /// 二进制帧，适配器已随模型删除，再去打开它会在 `openBox` 阶段抛
  /// 「unknown typeId」。换名之后老盒不再被打开，并在这里一次性删掉。
  /// 代价是老用户升级后首屏的头像/昵称空一下，等 nav 回来——这是换格式
  /// 一次性的成本，已在 #57 里确认过。
  ///
  /// **这句话当时漏了一个前提**（issue #161）：要等得到 nav，得有人去发。
  /// 那时 `AccountService` 把登录态判在「有没有这份缓存」上，而唯一会发
  /// nav 的地方又挡在同一个判断后面，缓存一空就再也填不回来。登录态现在
  /// 取自账号本身，缓存只决定首屏先显示什么——改缓存格式之前先确认这条
  /// 还成立。
  static late final Box<String> userInfo;
  static late final Box<dynamic> historyWord;
  static late final Box<dynamic> localCache;
  static late final Box<dynamic> setting;
  static late final Box<dynamic> video;
  static late final Box<int> watchProgress;
  static late final Box<Uint8List>? reply;

  static Future<void> init() async {
    Hive.init(path.join(appSupportDirPath, 'hive'));
    regAdapter();

    // 老的 `userInfo` 盒里是 typeId 4/5 的二进制帧，适配器已随模型删除
    // （S7 #57）。不删的话它只是占着磁盘；删掉也顺带让「换格式」这件事
    // 不留第二份真相。失败忽略——盒本来就可能不存在。
    try {
      await Hive.deleteBoxFromDisk('userInfo');
    } catch (_) {}

    await Future.wait([
      // 登录用户信息
      Hive.openBox<String>(
        'userInfoJson',
        compactionStrategy: (int entries, int deletedEntries) {
          return deletedEntries > 2;
        },
      ).then((res) => userInfo = res),
      // 本地缓存
      Hive.openBox(
        'localCache',
        compactionStrategy: (int entries, int deletedEntries) {
          return deletedEntries > 4;
        },
      ).then((res) => localCache = res),
      // 设置
      Hive.openBox('setting').then((res) => setting = res),
      // 搜索历史
      Hive.openBox(
        'historyWord',
        compactionStrategy: (int entries, int deletedEntries) {
          return deletedEntries > 10;
        },
      ).then((res) => historyWord = res),
      // 视频设置
      Hive.openBox('video').then((res) => video = res),
      Accounts.init(),
      Hive.openBox<int>(
        'watchProgress',
        keyComparator: _intStrDescKeyComparator,
        compactionStrategy: (entries, deletedEntries) {
          return deletedEntries > 4;
        },
      ).then((res) => watchProgress = res),
    ]);

    if (Pref.saveReply) {
      reply = await Hive.openBox<Uint8List>(
        'reply',
        keyComparator: _intStrDescKeyComparator,
        compactionStrategy: (entries, deletedEntries) {
          return deletedEntries > 10;
        },
      );
    } else {
      reply = null;
    }
  }

  static String exportAllSettings() {
    return Utils.jsonEncoder.convert({
      setting.name: setting.toMap(),
      video.name: video.toMap(),
    });
  }

  static Future<void> importAllSettings(String data) =>
      importAllJsonSettings(jsonDecode(data));

  static Future<List<void>> importAllJsonSettings(
    Map<String, dynamic> map,
  ) {
    return Future.wait([
      setting.clear().then((_) => setting.putAll(map[setting.name])),
      video.clear().then((_) => video.putAll(map[video.name])),
    ]);
  }

  static void regAdapter() {
    Hive
      ..registerAdapter(OwnerAdapter())
      ..registerAdapter(BiliCookieJarAdapter())
      ..registerAdapter(LoginAccountAdapter())
      ..registerAdapter(AccountTypeAdapter())
      ..registerAdapter(SetIntAdapter())
      ..registerAdapter(RuleFilterAdapter());
  }

  static Future<List<void>> compact() {
    return Future.wait([
      userInfo.compact(),
      historyWord.compact(),
      localCache.compact(),
      setting.compact(),
      video.compact(),
      Accounts.account.compact(),
      watchProgress.compact(),
      ?reply?.compact(),
    ]);
  }

  static Future<List<void>> close() {
    return Future.wait([
      userInfo.close(),
      historyWord.close(),
      localCache.close(),
      setting.close(),
      video.close(),
      Accounts.account.close(),
      watchProgress.close(),
      ?reply?.close(),
    ]);
  }

  static Future<List<void>> clear() {
    return Future.wait([
      userInfo.clear(),
      historyWord.clear(),
      localCache.clear(),
      setting.clear(),
      video.clear(),
      Accounts.clear(),
      watchProgress.clear(),
      ?reply?.clear(),
    ]);
  }

  static int _intStrDescKeyComparator(dynamic k1, dynamic k2) {
    if (k1 is int) {
      if (k2 is int) {
        return k2.compareTo(k1);
      } else {
        return -1;
      }
    } else if (k2 is String) {
      final lenCompare = k2.length.compareTo((k1 as String).length);
      if (lenCompare == 0) {
        return k2.compareTo(k1);
      } else {
        return lenCompare;
      }
    } else {
      return 1;
    }
  }
}
