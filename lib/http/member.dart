import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/browser_ua.dart';
import 'package:kilikili/http/error_msg.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/dynamics/result.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:dio/dio.dart';

abstract final class MemberHttp {
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
}
