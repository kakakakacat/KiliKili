import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/history/data.dart';
import 'package:kilikili/models_new/later/data.dart';
import 'package:kilikili/models_new/media_list/data.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:dio/dio.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

abstract final class UserHttp {

  // 稍后再看
  static Future<LoadingState<LaterData>> seeYouLater({
    required int page,
    int viewed = 0,
    String keyword = '',
    bool asc = false,
    int ps = 20,
  }) async {
    final res = await Request().get(
      Api.seeYouLater,
      queryParameters: await WbiSign.makSign({
        'pn': page,
        'ps': ps,
        'viewed': viewed,
        'key': keyword,
        'asc': asc,
        'need_split': true,
        'web_location': 333.881,
      }),
    );
    if (res.data['code'] == 0) {
      return Success(LaterData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  // 观看历史
  static Future<LoadingState<HistoryData>> historyList({
    required String type,
    int? max,
    int? viewAt,
    Account? account,
    int ps = 20,
  }) async {
    final res = await Request().get(
      Api.historyList,
      queryParameters: {
        'type': type,
        'ps': ps,
        'max': max ?? 0,
        'view_at': viewAt ?? 0,
      },
      options: Options(extra: {'account': account ?? Accounts.history}),
    );
    if (res.data['code'] == 0) {
      return Success(HistoryData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  // 暂停观看历史
  static Future<LoadingState<void>> pauseHistory(
    bool switchStatus, {
    Account? account,
  }) async {
    // 暂停switchStatus传true 否则false
    account ??= Accounts.history;
    final res = await Request().post(
      Api.pauseHistory,
      data: {
        'switch': switchStatus,
        'jsonp': 'jsonp',
        'csrf': account.csrf,
      },
      options: Options(
        extra: {'account': account},
        contentType: Headers.formUrlEncodedContentType,
      ),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  // 观看历史暂停状态
  static Future<LoadingState<bool>> historyStatus({Account? account}) async {
    final res = await Request().get(
      Api.historyStatus,
      options: Options(extra: {'account': account ?? Accounts.history}),
    );
    if (res.data['code'] == 0) {
      return Success(res.data['data']);
    } else {
      return Error(res.data['message']);
    }
  }

  // 清空历史记录
  static Future<LoadingState<void>> clearHistory({Account? account}) async {
    account ??= Accounts.history;
    final res = await Request().post(
      Api.clearHistory,
      data: {
        'jsonp': 'jsonp',
        'csrf': account.csrf,
      },
      options: Options(
        extra: {'account': account},
        contentType: Headers.formUrlEncodedContentType,
      ),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  // 稍后再看
  static Future<LoadingState<void>> toViewLater({
    String? bvid,
    Object? aid,
  }) async {
    assert(aid != null || bvid != null);
    final res = await Request().post(
      Api.toViewLater,
      data: {
        'aid': ?aid,
        'bvid': ?bvid,
        'csrf': Accounts.main.csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      SmartDialog.showToast('yeah！稍后再看');
      return const Success(null);
    } else {
      SmartDialog.showToast(res.data['message'].toString());
      return const Error(null);
    }
  }

  // 移除已观看
  static Future<LoadingState<void>> toViewDel({required Object aids}) async {
    final Map<String, dynamic> params = {
      'csrf': Accounts.main.csrf,
      'resources': aids,
    };
    final res = await Request().post(
      Api.toViewDel,
      data: params,
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      SmartDialog.showToast('yeah！成功移除');
      return const Success(null);
    } else {
      SmartDialog.showToast(res.data['message'].toString());
      return const Error(null);
    }
  }

  // 获取用户凭证 失效
  // static Future thirdLogin() async {
  //   final res = await Request().get(
  //     'https://passport.bilibili.com/login/app/third',
  //     queryParameters: {
  //       'appkey': Constants.appKey,
  //       'api': Constants.thirdApi,
  //       'sign': Constants.thirdSign,
  //     },
  //   );
  //   try {
  //     if (res.data['code'] == 0 && res.data['data']['has_login'] == 1) {
  //       Request().get(res.data['data']['confirm_uri']);
  //     }
  //   } catch (err) {
  //     SmartDialog.showNotify(msg: '获取用户凭证: $err', notifyType: NotifyType.error);
  //   }
  // }

  // 清空稍后再看 // clean_type: null->all, 1->invalid, 2->viewed
  static Future<LoadingState<void>> toViewClear([int? cleanType]) async {
    final res = await Request().post(
      Api.toViewClear,
      data: {
        'clean_type': ?cleanType,
        'csrf': Accounts.main.csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  // 删除历史记录
  static Future<LoadingState<void>> delHistory(
    String kid, {
    Account? account,
  }) async {
    account ??= Accounts.history;
    final res = await Request().post(
      Api.delHistory,
      data: {
        'kid': kid,
        'jsonp': 'jsonp',
        'csrf': account.csrf,
      },
      options: Options(
        extra: {'account': account},
        contentType: Headers.formUrlEncodedContentType,
      ),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  // 搜索历史记录
  static Future<LoadingState<HistoryData>> searchHistory({
    required int pn,
    required String keyword,
    Account? account,
  }) async {
    final res = await Request().get(
      Api.searchHistory,
      queryParameters: {
        'pn': pn,
        'keyword': keyword,
        'business': 'all',
      },
      options: Options(extra: {'account': account ?? Accounts.history}),
    );
    if (res.data['code'] == 0) {
      return Success(HistoryData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  // 稍后再看列表
  static Future<LoadingState<MediaListData>> getMediaList({
    required Object type,
    required Object bizId,
    required int ps,
    dynamic oid,
    int? otype,
    bool withCurrent = false,
    bool desc = true,
    dynamic sortField = 1,
    bool direction = false,
  }) async {
    final res = await Request().get(
      Api.mediaList,
      queryParameters: {
        'mobi_app': 'web',
        'type': type,
        'biz_id': bizId,
        'oid': ?oid,
        'otype': ?otype, // ugc:2 // pgc: 24
        'ps': ps,
        'direction': direction,
        'desc': desc,
        'sort_field': sortField,
        'tid': 0,
        'with_current': withCurrent,
      },
    );
    if (res.data['code'] == 0) {
      return Success(MediaListData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }
}
