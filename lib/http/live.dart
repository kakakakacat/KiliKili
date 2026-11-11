import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/account_type.dart';
import 'package:kilikili/models_new/live/live_danmaku/danmaku_msg.dart';
import 'package:kilikili/services/rust/rust_live.dart' show LiveShieldUser;
import 'package:kilikili/models_new/live/live_dm_info/data.dart';
import 'package:kilikili/models_new/live/live_superchat/data.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/app_sign.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:dio/dio.dart';

/// 直播接口中**尚未迁到 Rust** 的那些。
///
/// 只读部分已迁走十四条，见 `lib/services/rust/rust_live.dart`：分区三条、
/// 首页推荐、分区二级列表、关注的直播、H5 房间信息、取流、表情包、
/// 直播搜索、屏蔽设置、贡献榜、勋章墙、勋章名。这里剩下的是三类：
///
/// - **SuperChat（`superChatMsg`）**——它虽然是只读，却归 S11b（#62）：
///   `SuperChatItem` 的第二个生产者是弹幕长连接的 `SUPER_CHAT_MESSAGE`
///   （见 `live_room/controller.dart`），只迁 REST 这条，模型一行都删不
///   掉。它还有 `expired`/`deleted` 两个可变的客户端状态，而 FRB 结构
///   不可变；解析时还读 `GlobalData().showMedal` 这个全局设置。
/// - 弹幕长连接相关（`liveRoomDmPrefetch`/`liveRoomGetDanmakuToken`）
///   ——S11b（#62），阻塞在 ADR-012（#49）；
/// - 全部写操作（发弹幕、收藏分区、禁言、增删屏蔽词与屏蔽用户、点赞
///   上报、弹幕举报、SC 上报、反馈）——归 ADR-013（#50）。
///
/// **`liveShieldUser` 名字像查询，实际是写**：`type: 1` 添加屏蔽用户、
/// `type: 0` 移除，返回值只是新增那条的回显。它因此留在这里，但已经改成
/// 直接构造 FRB 的 `LiveShieldUser`（ADR-011 补充七）。
abstract final class LiveHttp {
  static Account get recommend => Accounts.get(AccountType.recommend);

  static Future<LoadingState<void>> sendLiveMsg({
    required Object roomId,
    required Object msg,
    Object? dmType,
    Object? emoticonOptions,
    int replyMid = 0,
    String replayDmid = '',
  }) async {
    String csrf = Accounts.main.csrf;
    final res = await Request().post(
      Api.sendLiveMsg,
      queryParameters: await WbiSign.makSign({'web_location': 444.8}),
      data: FormData.fromMap({
        'bubble': 0,
        'msg': msg,
        'color': 16777215,
        'mode': 1,
        'dm_type': ?dmType,
        if (emoticonOptions != null)
          'emoticonOptions': emoticonOptions
        else ...{
          'room_type': 0,
          'jumpfrom': 0,
          'reply_mid': replyMid,
          'reply_attr': 0,
          'replay_dmid': replayDmid,
          'statistics': '{"appId":100,"platform":5}',
          'reply_type': 0,
          'reply_uname': '',
        },
        'fontsize': 25,
        'rnd': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        'roomid': roomId,
        'csrf': csrf,
        'csrf_token': csrf,
      }),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<List<DanmakuMsg>?>> liveRoomDmPrefetch({
    required Object roomId,
  }) async {
    final res = await Request().get(
      Api.liveRoomDmPrefetch,
      queryParameters: {'roomid': roomId},
      options: Options(
        headers: {
          'referer': 'https://live.bilibili.com/$roomId',
          'user-agent': BrowserUa.pc,
        },
      ),
    );
    if (res.data['code'] == 0) {
      try {
        return Success(
          (res.data['data']?['room'] as List?)
              ?.map((e) => DanmakuMsg.fromPrefetch(e))
              .toList(),
        );
      } catch (e) {
        return Error(e.toString());
      }
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<LiveDmInfoData>> liveRoomGetDanmakuToken({
    required Object roomId,
  }) async {
    final res = await Request().get(
      Api.liveRoomDmToken,
      queryParameters: await WbiSign.makSign({
        'id': roomId,
        'web_location': 444.8,
      }),
    );
    if (res.data['code'] == 0) {
      try {
        return Success(LiveDmInfoData.fromJson(res.data['data']));
      } catch (e) {
        return Error(e.toString());
      }
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> setLiveFavTag({
    required String ids,
  }) async {
    final data = {
      'tags': ids,
      'access_key': Accounts.main.accessKey,
      'actionKey': 'appkey',
      'build': 8430300,
      'channel': 'master',
      'version': '8.43.0',
      'c_locale': 'zh_CN',
      'device': 'android',
      'disable_rcmd': 0,
      'mobi_app': 'android',
      'platform': 'android',
      's_locale': 'zh_CN',
      'statistics': Constants.statisticsApp,
    };
    AppSign.appSign(data);
    final res = await Request().post(
      Api.setLiveFavTag,
      data: data,
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );

    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> liveSetSilent({
    required String type,
    required int level,
  }) async {
    final csrf = Accounts.main.csrf;
    final res = await Request().post(
      Api.liveSetSilent,
      data: {
        'type': type,
        'level': level,
        'csrf': csrf,
        'csrf_token': csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> addShieldKeyword({
    required String keyword,
  }) async {
    final csrf = Accounts.main.csrf;
    final res = await Request().post(
      Api.addShieldKeyword,
      data: {
        'keyword': keyword,
        'csrf': csrf,
        'csrf_token': csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> delShieldKeyword({
    required String keyword,
  }) async {
    final csrf = Accounts.main.csrf;
    final res = await Request().post(
      Api.delShieldKeyword,
      data: {
        'keyword': keyword,
        'csrf': csrf,
        'csrf_token': csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<LiveShieldUser>> liveShieldUser({
    required Object uid,
    required Object roomid,
    required int type,
  }) async {
    final csrf = Accounts.main.csrf;
    final res = await Request().post(
      Api.liveShieldUser,
      data: {
        'uid': uid,
        'roomid': roomid,
        'type': type,
        'csrf': csrf,
        'csrf_token': csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      // 读那一侧（`getInfoByUser`）已迁到 Rust，这里直接构造 FRB 的
      // 同一个类而不是另留一个等价的 Dart 模型——ADR-011 补充七：读写
      // 共用一个类型，就没有两边各自漂移的机会。本函数随写操作统一迁移
      // （ADR-013 / #50）。
      final data = res.data['data'] as Map<String, dynamic>;
      return Success(
        LiveShieldUser(uid: data['uid'] as int, uname: data['uname'] as String),
      );
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> liveLikeReport({
    required int clickTime,
    required Object roomId,
    required Object uid,
    Object? anchorId,
  }) async {
    final res = await Request().post(
      Api.liveLikeReport,
      data: await WbiSign.makSign({
        'click_time': clickTime,
        'room_id': roomId,
        'uid': uid,
        'anchor_id': ?anchorId,
        'web_location': 444.8,
        'csrf': Accounts.heartbeat.csrf,
      }),
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  @pragma('vm:notify-debugger-on-exception')
  static Future<LoadingState<SuperChatData>> superChatMsg(
    int roomId,
  ) async {
    final res = await Request().get(
      Api.superChatMsg,
      queryParameters: {
        'room_id': roomId,
      },
    );
    if (res.data['code'] == 0) {
      try {
        return Success(SuperChatData.fromJson(res.data['data'], roomId));
      } catch (e, s) {
        return Error('$e\n\n$s');
      }
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> liveDmReport({
    required int roomId,
    required Object mid,
    required String msg,
    required String reason,
    required int reasonId,
    required int dmType,
    required Object idStr,
    required Object ts,
    required Object sign,
  }) async {
    final csrf = Accounts.main.csrf;
    final data = {
      'id': 0,
      'roomid': roomId,
      'tuid': mid,
      'msg': msg,
      'reason': reason,
      'ts': ts,
      'sign': sign,
      'reason_id': reasonId,
      'token': '',
      'dm_type': dmType,
      'id_str': idStr,
      'csrf_token': csrf,
      'csrf': csrf,
      'visit_id': '',
    };
    final res = await Request().post(
      Api.liveDmReport,
      data: data,
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> superChatReport({
    required int id,
    required Object roomId,
    required Object uid,
    required String msg,
    required String reason,
    required int ts,
    required String token,
  }) async {
    final csrf = Accounts.main.csrf;
    final res = await Request().post(
      Api.superChatReport,
      data: {
        'id': id,
        'roomid': roomId,
        'uid': uid,
        'msg': msg,
        'reason': reason,
        'ts': ts,
        'sign': '',
        'reason_id': reason,
        'token': token,
        'id_str': id.toString(),
        'csrf_token': csrf,
        'csrf': csrf,
        'visit_id': '',
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> liveFeedback(
    Object roomId,
    Object id,
    String type, {
    int page = 1,
  }) async {
    final params = {
      'access_key': ?recommend.accessKey,
      'actionKey': 'appkey',
      'build': 8430300,
      'channel': 'master',
      'c_locale': 'zh_CN',
      'device': 'android',
      'disable_rcmd': 0,
      'mobi_app': 'android',
      'platform': 'android',
      's_locale': 'zh_CN',
      'statistics': Constants.statisticsApp,
      'version': '8.43.0',
      'id': id,
      'id_type': type,
      'room_id': roomId,
      'type': 'dislike',
      'page': page,
    };
    AppSign.appSign(params);
    final res = await Request().get(
      Api.liveFeedback,
      queryParameters: params,
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }
}
