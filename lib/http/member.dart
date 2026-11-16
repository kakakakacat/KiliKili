import 'dart:io';

import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/error_msg.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/member/archive_order_type_app.dart';
import 'package:kilikili/models/common/member/archive_order_type_web.dart';
import 'package:kilikili/models/common/member/archive_sort_type_app.dart';
import 'package:kilikili/models/common/member/contribute_type.dart';
import 'package:kilikili/models/common/member/web_ss_type.dart';
import 'package:kilikili/models/dynamics/result.dart';
import 'package:kilikili/models/member/tags.dart';
import 'package:kilikili/models_new/member/coin_like_arc/data.dart';
import 'package:kilikili/models_new/member/search_archive/data.dart';
import 'package:kilikili/models_new/member/season_web/data.dart';
import 'package:kilikili/models_new/member_guard/data.dart';
import 'package:kilikili/models_new/space/space/data.dart';
import 'package:kilikili/models_new/space/space_archive/data.dart';
import 'package:kilikili/models_new/space/space_article/data.dart';
import 'package:kilikili/models_new/space/space_audio/data.dart';
import 'package:kilikili/models_new/space/space_cheese/data.dart';
import 'package:kilikili/models_new/space/space_opus/data.dart';
import 'package:kilikili/models_new/space/space_season_series/item.dart';
import 'package:kilikili/models_new/space/space_shop/data.dart';
import 'package:kilikili/models_new/upower_rank/data.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/app_sign.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:dio/dio.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

abstract final class MemberHttp {
  static Future<LoadingState<SpaceArticleData>> spaceArticle({
    required int mid,
    required int page,
  }) async {
    final params = {
      'build': 8430300,
      'channel': 'master',
      'version': '8.43.0',
      'c_locale': 'zh_CN',
      'mobi_app': 'android',
      'platform': 'android',
      'pn': page,
      'ps': 10,
      's_locale': 'zh_CN',
      'statistics': Constants.statisticsApp,
      'vmid': mid,
    };
    final res = await Request().get(
      Api.spaceArticle,
      queryParameters: params,
      options: Options(
        headers: {
          'bili-http-engine': 'cronet',
          'user-agent': Constants.userAgentApp,
        },
      ),
    );
    if (res.data['code'] == 0) {
      return Success(SpaceArticleData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<SpaceSsData>> seasonSeriesList({
    required int? mid,
    required int pn,
  }) async {
    final res = await Request().get(
      Api.seasonSeries,
      queryParameters: {
        'mid': mid,
        'page_num': pn,
        'page_size': 10,
      },
    );
    if (res.data['code'] == 0) {
      return Success(
        SpaceSsData.fromJson(res.data['data']?['items_lists'] ?? {}),
      );
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<SpaceArchiveData>> spaceArchive({
    required ContributeType type,
    required int? mid,
    String? aid,
    ArchiveOrderTypeApp? order,
    ArchiveSortTypeApp? sort,
    int? pn,
    int? next,
    int? seasonId,
    int? seriesId,
    bool? includeCursor,
  }) async {
    final params = {
      'aid': ?aid,
      'build': 8430300,
      'version': '8.43.0',
      'c_locale': 'zh_CN',
      'channel': 'master',
      'mobi_app': 'android',
      'platform': 'android',
      's_locale': 'zh_CN',
      'ps': 20,
      'pn': ?pn,
      'next': ?next,
      'season_id': ?seasonId,
      'series_id': ?seriesId,
      'qn': type == .video ? 80 : 32,
      'order': ?order?.name,
      'sort': ?sort?.name,
      'include_cursor': ?includeCursor,
      'statistics': Constants.statisticsApp,
      'vmid': mid,
    };
    final res = await Request().get(
      type.api,
      queryParameters: params,
      options: Options(
        headers: {
          'bili-http-engine': 'cronet',
          'user-agent': Constants.userAgentApp,
        },
      ),
    );
    if (res.data['code'] == 0) {
      return Success(SpaceArchiveData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<SpaceAudioData>> spaceAudio({
    required int page,
    required mid,
  }) async {
    final res = await Request().get(
      Api.spaceAudio,
      queryParameters: {
        'pn': page,
        'ps': 20,
        'order': 1,
        'uid': mid,
        'web_location': 333.1387,
      },
    );
    if (res.data['code'] == 0) {
      return Success(SpaceAudioData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<SpaceCheeseData>> spaceCheese({
    required int page,
    required mid,
  }) async {
    final res = await Request().get(
      Api.spaceCheese,
      queryParameters: {
        'pn': page,
        'ps': 30,
        'mid': mid,
        'web_location': 333.1387,
      },
    );
    if (res.data['code'] == 0) {
      return Success(SpaceCheeseData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  // static Future<LoadingState> spaceStory({
  //   required Object mid,
  //   required Object aid,
  //   required Object beforeSize,
  //   required Object afterSize,
  //   required Object cid,
  //   required Object contain,
  //   required Object index,
  // }) async {
  //   final params = {
  //     'aid': aid,
  //     'before_size': beforeSize,
  //     'after_size': afterSize,
  //     'cid': cid,
  //     'contain': contain,
  //     'index': index,
  //     'build': 8430300,
  //     'version': '8.43.0',
  //     'c_locale': 'zh_CN',
  //     'channel': 'master',
  //     'mobi_app': 'android',
  //     'platform': 'android',
  //     's_locale': 'zh_CN',
  //     'statistics': Constants.statisticsApp,
  //     'vmid': mid,
  //   };
  //   final res = await Request().get(
  //     Api.spaceStory,
  //     queryParameters: params,
  //     options: Options(
  //       headers: {
  //         'bili-http-engine': 'cronet',
  //         'user-agent': Constants.userAgentApp,
  //       },
  //     ),
  //   );
  //   if (res.data['code'] == 0) {
  //     return Success(res.data['data']);
  //   } else {
  //     return Error(res.data['message']);
  //   }
  // }

  static Future<LoadingState<SpaceData>> space({
    int? mid,
    dynamic fromViewAid,
  }) async {
    final params = {
      'build': 8430300,
      'version': '8.43.0',
      'c_locale': 'zh_CN',
      'channel': 'master',
      'mobi_app': 'android',
      'platform': 'android',
      's_locale': 'zh_CN',
      'from_view_aid': ?fromViewAid,
      'statistics': Constants.statisticsApp,
      'vmid': mid,
    };
    final res = await Request().get(
      Api.space,
      queryParameters: params,
      options: Options(
        headers: {
          'bili-http-engine': 'cronet',
          'user-agent': Constants.userAgentApp,
        },
      ),
    );
    if (res.data['code'] == 0) {
      return Success(SpaceData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<SearchArchiveData>> searchArchive({
    required Object mid,
    int tid = 0, // e.g. pugv: 196
    int ps = 30,
    required int pn,
    String? keyword,
    String? specialType, // e.g. 'charging'
    ArchiveOrderTypeWeb order = .pubdate,
  }) async {
    String dmImgStr = Utils.base64EncodeRandomString(16, 64);
    String dmCoverImgStr = Utils.base64EncodeRandomString(32, 128);
    final params = await WbiSign.makSign({
      'mid': mid,
      'ps': ps,
      'tid': tid,
      'pn': pn,
      'keyword': ?keyword,
      'special_type': ?specialType,
      'order': order.name,
      'platform': 'web',
      'web_location': 333.1387,
      'order_avoided': true,
      'dm_img_list': '[]',
      'dm_img_str': dmImgStr,
      'dm_cover_img_str': dmCoverImgStr,
      'dm_img_inter': '{"ds":[],"wh":[0,0,0],"of":[0,0,0]}',
    });
    final res = await Request().get(
      Api.searchArchive,
      queryParameters: params,
      options: Options(
        headers: {
          HttpHeaders.userAgentHeader: BrowserUa.pc,
          HttpHeaders.refererHeader: '${HttpString.spaceBaseUrl}/$mid',
          'origin': HttpString.spaceBaseUrl,
        },
      ),
    );
    if (res.data['code'] == 0) {
      return Success(SearchArchiveData.fromJson(res.data['data']));
    } else {
      return Error(errorMsg[res.data['code']] ?? res.data['message']);
    }
  }

  static Future<LoadingState<SeasonWebData>> seasonSeriesWeb({
    required WebSsType type,
    required Object mid,
    required Object id,
    int ps = 30,
    required int pn,
    ArchiveSortTypeApp sort = .desc,
  }) async {
    final res = await Request().get(
      type.api,
      queryParameters: switch (type) {
        .season => {
          'mid': mid,
          'season_id': id,
          'sort_reverse': sort == .asc,
          'page_size': ps,
          'page_num': pn,
          'web_location': 333.1387,
        },
        .series => {
          'mid': mid,
          'series_id': id,
          'sort': sort.name,
          'ps': ps,
          'pn': pn,
          'web_location': 333.1387,
        },
      },
      options: Options(
        headers: {
          HttpHeaders.userAgentHeader: BrowserUa.pc,
          HttpHeaders.refererHeader: '${HttpString.spaceBaseUrl}/$mid',
          'origin': HttpString.spaceBaseUrl,
        },
      ),
    );
    if (res.data['code'] == 0) {
      return Success(SeasonWebData.fromJson(res.data['data']));
    } else {
      return Error(errorMsg[res.data['code']] ?? res.data['message']);
    }
  }

  // 用户动态
  @pragma('vm:notify-debugger-on-exception')
  static Future<LoadingState<DynamicsDataModel>> memberDynamic({
    String? offset,
    required int mid,
  }) async {
    String dmImgStr = Utils.base64EncodeRandomString(16, 64);
    String dmCoverImgStr = Utils.base64EncodeRandomString(32, 128);
    final params = await WbiSign.makSign({
      'offset': offset ?? '',
      'host_mid': mid,
      'timezone_offset': '-480',
      'features': Constants.dynFeatures,
      'platform': 'web',
      'web_location': '333.1387',
      'dm_img_list': '[]',
      'dm_img_str': dmImgStr,
      'dm_cover_img_str': dmCoverImgStr,
      'dm_img_inter': '{"ds":[],"wh":[0,0,0],"of":[0,0,0]}',
      'x-bili-device-req-json':
          '{"platform":"web","device":"pc","spmid":"333.1387"}',
    });
    final res = await Request().get(
      Api.memberDynamic,
      queryParameters: params,
      options: Options(
        headers: {
          'user-agent': BrowserUa.pc,
          'origin': 'https://space.bilibili.com',
          'referer': 'https://space.bilibili.com/$mid/dynamic',
        },
      ),
    );
    if (res.data['code'] == 0) {
      try {
        DynamicsDataModel data = DynamicsDataModel.fromJson(res.data['data']);
        if (data.loadNext == true) {
          return await memberDynamic(offset: data.offset, mid: mid);
        }
        return Success(data);
      } catch (e, s) {
        return Error('$e\n\n$s');
      }
    } else {
      return Error(errorMsg[res.data['code']] ?? res.data['message']);
    }
  }

  static Future<LoadingState<DynamicsDataModel>> dynSearch({
    required int pn,
    required dynamic mid,
    required dynamic offset,
    required String keyword,
  }) async {
    final res = await Request().get(
      Api.dynSearch,
      queryParameters: {
        'host_mid': mid,
        'page': pn,
        'offset': offset,
        'keyword': keyword,
        'features': Constants.dynFeatures,
        'web_location': 333.1387,
      },
    );
    if (res.data['code'] == 0) {
      return Success(DynamicsDataModel.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<SpaceOpusData>> spaceOpus({
    required int hostMid,
    required int page,
    String offset = '',
    String type = 'all',
  }) async {
    final res = await Request().get(
      Api.spaceOpus,
      queryParameters: await WbiSign.makSign({
        'host_mid': hostMid,
        'page': page,
        'offset': offset,
        'type': type,
        'web_location': 333.1387,
      }),
    );
    if (res.data['code'] == 0) {
      return Success(SpaceOpusData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<UpowerRankData>> upowerRank({
    required Object upMid,
    required int page,
    int? privilegeType,
  }) async {
    final res = await Request().get(
      Api.upowerRank,
      queryParameters: {
        'up_mid': upMid,
        'pn': page,
        'ps': 100,
        'privilege_type': ?privilegeType,
        'mobi_app': 'web',
        'web_location': 333.1196,
        if (Accounts.main.isLogin) 'csrf': Accounts.main.csrf,
      },
    );
    if (res.data['code'] == 0) {
      return Success(UpowerRankData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<CoinLikeArcData>> coinArc({
    required int mid,
    required int page,
  }) async {
    final res = await Request().get(
      Api.coinArc,
      queryParameters: {
        'pn': page,
        'ps': 20,
        'vmid': mid,
      },
    );
    if (res.data['code'] == 0) {
      return Success(CoinLikeArcData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<CoinLikeArcData>> likeArc({
    required int mid,
    required int page,
  }) async {
    final res = await Request().get(
      Api.likeArc,
      queryParameters: {
        'pn': page,
        'ps': 20,
        'vmid': mid,
      },
    );
    if (res.data['code'] == 0) {
      return Success(CoinLikeArcData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<SpaceShopData>> spaceShop({
    required int mid,
  }) async {
    final params = {
      'access_key': ?Accounts.main.accessKey,
      'actionKey': 'appkey',
      'build': 8430300,
      'mVersion': 309,
      'mallVersion': 8430300,
      'statistics': Constants.statisticsApp,
    };
    AppSign.appSign(params);
    final res = await Request().post(
      Api.spaceShop,
      queryParameters: params,
      data: {
        "from": "cps_productTab_$mid",
        "searchAfter": 0,
        "msource": "cps_productTab_$mid",
        "pageSize": 8,
        "upMid": mid.toString(),
      },
    );
    if (res.data['code'] == 0) {
      return Success(SpaceShopData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<MemberGuardData>> memberGuard({
    required Object ruid,
    required int page,
  }) async {
    final res = await Request().get(
      Api.memberGuard,
      queryParameters: {
        'page': page,
        'page_size': 20,
        'ruid': ruid,
      },
    );
    if (res.data['code'] == 0) {
      return Success(MemberGuardData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }
}
