import 'package:kilikili/common/constants.dart';
import 'package:kilikili/common/widgets/pair.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/reply.dart';
import 'package:kilikili/models/common/dynamic/dynamics_type.dart';
import 'package:kilikili/models/common/reply/reply_option_type.dart';
import 'package:kilikili/models/dynamics/result.dart';
import 'package:kilikili/models/dynamics/up.dart';
import 'package:kilikili/models_new/article/article_view/data.dart';
import 'package:kilikili/models_new/dynamic/dyn_mention/data.dart';
import 'package:kilikili/models_new/dynamic/dyn_mention/group.dart';
import 'package:kilikili/models_new/dynamic/dyn_topic_feed/topic_card_list.dart';
import 'package:kilikili/services/rust/follow_compat.dart';
import 'package:kilikili/services/rust/rust_misc.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:kilikili/utils/wbi_sign.dart';
import 'package:dio/dio.dart';

/// 动态域剩下还在 Dart 的接口。**S5 续单（#150）第一批之后还剩 13 条。**
///
/// 已迁到 Rust（见 `lib/services/rust/rust_dynamic.dart`）：专栏信息、文集、
/// 话题顶部、推荐话题、部落气泡，以及发布页的话题搜索（那条原在
/// `lib/http/search.dart`，因为是 `TopicItem` 的第二个生产者而一并迁走）。
/// 对应的 `lib/models_new/{bubble,article/article_info,article/article_list,
/// dynamic/dyn_topic_top,dynamic/dyn_topic_pub_search}` 已在同一个 PR 里
/// 删除（ADR-011 决策四）。`dynReaction` 全仓库零调用点，直接删掉。
///
/// 剩下这些留在 Dart 是**刻意的**，分两类，别顺手一起迁：
///
/// 1. **经过枢纽模型的**——`followDynamic`、`dynamicDetail`、`opusDetail`、
///    `topicFeed`、`topicFold`、`dynPic`。它们全部产出或消费
///    `lib/models/dynamics/result.dart` 里的 `DynamicItemModel`（1426 行的
///    多态卡片树）。那棵树必须整棵一次迁完，否则两边会同时持有半棵；
///    它是 #150 的下一批，也是最大的一批。
/// 2. **各有一处独立障碍的**：
///    - `articleView`：`opus.content.paragraphs` 挂着 `ArticleContentModel`
///      （492 行的富文本树），与卡片树同量级；
///    - `dynMention`：`MentionItem with MultiSelectData` 掺着 widget 层的
///      多选 mixin，要连同选择页一起改；
///    - `createDynamic` / `editDyn`：发布与编辑，参数面大且与图片上传
///      耦合，等 ADR-015（multipart 与上传取消，#193）落地后再迁；
///    - `followUp` / `dynUpList` / `followings`：`UpItem` 还有第二个生产者
///      （`RustMisc.followings` 经 `follow_compat`），按共享模型的生产者
///      分批。
///
/// 去向都在 **issue #150**，迁完之后这个文件整个删掉。
abstract final class DynamicsHttp {
  @pragma('vm:notify-debugger-on-exception')
  static Future<LoadingState<DynamicsDataModel>> followDynamic({
    int? hostMid,
    String? offset,
    Set<int>? tempBannedList,
    DynamicsTabType type = .all,
  }) async {
    Map<String, dynamic> data = {
      if (type == .up) 'host_mid': hostMid else 'type': type.name,
      'offset': ?offset,
      'features': Constants.dynFeatures,
    };
    final res = await Request().get(Api.followDynamic, queryParameters: data);
    final code = res.data['code'];
    if (code == 0) {
      try {
        DynamicsDataModel data = DynamicsDataModel.fromJson(
          res.data['data'],
          type: type,
          tempBannedList: tempBannedList,
        );
        if (data.loadNext == true) {
          return await followDynamic(
            type: type,
            offset: data.offset,
            hostMid: hostMid,
            tempBannedList: tempBannedList,
          );
        }
        return Success(data);
      } catch (e, s) {
        return Error('$e\n\n$s');
      }
    } else {
      return Error(code == 4101132 ? '没有数据' : res.data['message']);
    }
  }

  static Future<LoadingState<FollowUpModel>> followUp() async {
    final res = await Request().get(
      Api.followUp,
      queryParameters: {
        'up_list_more': 1,
        'web_location': 333.1365,
      },
    );
    if (res.data['code'] == 0) {
      return Success(FollowUpModel.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<FollowUpModel>> dynUpList(String? offset) async {
    final res = await Request().get(
      Api.dynUplist,
      queryParameters: {
        'offset': ?offset,
        'platform': 'web',
        'web_location': 333.1365,
      },
    );
    if (res.data['code'] == 0) {
      return Success(FollowUpModel.fromUpList(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  /// 动态页「全部 up 主」那一栏。
  ///
  /// 用的就是关注列表接口（`/x/relation/followings`），已随 #66 整条迁到
  /// Rust；这里不再自己发请求，否则同一条路径会留下两份 Dart 实现
  /// （ADR-011 决策四）。装进 `FollowUpModel` 只是换个容器，适配见
  /// `lib/services/rust/follow_compat.dart`。
  static Future<LoadingState<FollowUpModel>> followings({
    required int vmid,
    required int pn,
    int ps = 20,
    String orderType = '', // ''=>最近关注，'attention'=>最常访问
  }) async {
    final res = await RustMisc.followings(
      vmid: vmid,
      pn: pn,
      ps: ps,
      orderType: orderType,
    );
    return res.toFollowUpState();
  }

  // 动态点赞
  // static Future likeDynamic({
  //   required String? dynamicId,
  //   required int? up,
  // }) async {
  //   final res = await Request().post(
  //     Api.likeDynamic,
  //     queryParameters: {
  //       'dynamic_id': dynamicId,
  //       'up': up,
  //       'csrf': Accounts.main.csrf,
  //     },
  //   );
  //   if (res.data['code'] == 0) {
  //     return {
  //       'status': true,
  //       'data': res.data['data'],
  //     };
  //   } else {
  //     return {'status': false, 'msg': res.data['message']};
  //   }
  // }

  static Future<LoadingState<Map?>> createDynamic({
    dynamic mid,
    dynamic dynIdStr, // repost dyn
    dynamic rid, // repost video
    dynamic dynType,
    dynamic rawText,
    List? pics,
    int? publishTime,
    ReplyOptionType? replyOption,
    int? privatePub,
    List<Map<String, dynamic>>? extraContent,
    Pair<int, String>? topic,
    String? title,
    Map? attachCard,
  }) async {
    final res = await Request().post(
      Api.createDynamic,
      queryParameters: {
        'platform': 'web',
        'csrf': Accounts.main.csrf,
        'x-bili-device-req-json': '{"platform": "web", "device": "pc"}',
        'x-bili-web-req-json': '{"spm_id": "333.999"}',
      },
      data: {
        "dyn_req": {
          "content": {
            "contents": [
              if (rawText != null)
                {
                  "raw_text": rawText,
                  "type": 1,
                  "biz_id": "",
                },
              ...?extraContent,
            ],
            if (title != null && title.isNotEmpty) 'title': title,
          },
          if (privatePub != null || replyOption != null || publishTime != null)
            "option": {
              'private_pub': ?privatePub,
              "timer_pub_time": ?publishTime,
              if (replyOption == ReplyOptionType.close)
                "close_comment": 1
              else if (replyOption == ReplyOptionType.choose)
                "up_choose_comment": 1,
            },
          "scene": rid != null
              ? 5
              : dynIdStr != null
              ? 4
              : pics != null
              ? 2
              : 1,
          'pics': ?pics,
          "attach_card": attachCard,
          "upload_id":
              "${rid != null ? 0 : mid}_${DateTime.now().millisecondsSinceEpoch ~/ 1000}_${Utils.random.nextInt(9000) + 1000}",
          "meta": {
            "app_meta": {"from": "create.dynamic.web", "mobi_app": "web"},
          },
          if (topic != null)
            "topic": {
              "id": topic.first,
              "name": topic.second,
              "from_source": "dyn.web.list",
              "from_topic_id": 0,
            },
        },
        if (dynIdStr != null || rid != null)
          "web_repost_src": {
            "dyn_id_str": ?dynIdStr,
            if (rid != null)
              "revs_id": {
                "dyn_type": dynType,
                "rid": rid,
              },
          },
      },
    );
    if (res.data['code'] == 0) {
      return Success(res.data['data']);
    } else {
      return Error(res.data['message']);
    }
  }

  //
  @pragma('vm:notify-debugger-on-exception')
  static Future<LoadingState<DynamicItemModel>> dynamicDetail({
    dynamic id,
    dynamic rid,
    dynamic type,
    bool clearCookie = false,
  }) async {
    final res = await Request().get(
      Api.dynamicDetail,
      queryParameters: {
        'timezone_offset': -480,
        'id': ?id,
        'rid': ?rid,
        'type': ?type,
        'features': Constants.dynDetailFeatures,
        'gaia_source': 'Athena',
        'web_location': '333.1330',
        'x-bili-device-req-json':
            '{"platform":"web","device":"pc","spmid":"333.1330"}',
        if (!clearCookie && Accounts.main.isLogin) 'csrf': Accounts.main.csrf,
      },
      options: clearCookie ? ReplyHttp.options : null,
    );
    if (res.data['code'] == 0) {
      try {
        return Success(DynamicItemModel.fromJson(res.data['data']['item']));
      } catch (e, s) {
        return Error('$e\n\n$s');
      }
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<ArticleViewData>> articleView({
    required dynamic cvId,
  }) async {
    final res = await Request().get(
      Api.articleView,
      queryParameters: await WbiSign.makSign({
        'id': cvId,
        'gaia_source': 'main_web',
        'web_location': '333.976',
      }),
    );
    if (res.data['code'] == 0) {
      return Success(ArticleViewData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<DynamicItemModel>> opusDetail({
    required dynamic opusId,
  }) async {
    final res = await Request().get(
      Api.opusDetail,
      queryParameters: await WbiSign.makSign({
        'timezone_offset': '-480',
        'features': Constants.opusDetailFeatures,
        'id': opusId,
      }),
    );
    if (res.data['code'] == 0) {
      return Success(DynamicItemModel.fromOpusJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<TopicCardList?>> topicFeed({
    required Object topicId,
    String? offset,
    required int sortBy,
  }) async {
    final res = await Request().get(
      Api.topicFeed,
      queryParameters: {
        'topic_id': topicId,
        'sort_by': sortBy,
        'offset': ?offset,
        'page_size': 20,
        'source': 'Web',
        'features': Constants.dynFeatures,
      },
    );
    if (res.data['code'] == 0) {
      final list = res.data['data']?['topic_card_list'];
      if (list == null) {
        return const Success(null);
      } else {
        return Success(TopicCardList.fromJson(list));
      }
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<TopicCardList?>> topicFold({
    required Object topicId,
    required int sortBy,
  }) async {
    final res = await Request().get(
      Api.topicFold,
      queryParameters: {
        'topic_id': topicId,
        'sort_by': sortBy,
      },
    );
    if (res.data['code'] == 0) {
      final list = res.data['data']?['topic_card_list'];
      if (list == null) {
        return const Success(null);
      } else {
        return Success(TopicCardList.fromJson(list));
      }
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<List<OpusPicModel>?>> dynPic(dynamic id) async {
    final res = await Request().get(
      Api.dynPic,
      queryParameters: {
        'id': id,
        'web_location': 333.1368,
      },
    );
    if (res.data['code'] == 0) {
      return Success(
        (res.data['data'] as List?)
            ?.map((e) => OpusPicModel.fromJson(e))
            .toList(),
      );
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<List<MentionGroup>?>> dynMention({
    String? keyword,
  }) async {
    final res = await Request().get(
      Api.dynMention,
      queryParameters: {
        if (keyword != null && keyword.isNotEmpty) 'keyword': keyword,
        'web_location': 333.1365,
      },
    );
    if (res.data['code'] == 0) {
      return Success(
        DynMentionData.fromJson(res.data['data']).groups,
      );
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> editDyn({
    required Object dynId,
    Object? repostDynId,
    dynamic rawText,
    List? pics,
    ReplyOptionType? replyOption,
    int? privatePub,
    List<Map<String, dynamic>>? extraContent,
    Pair<int, String>? topic,
    String? title,
    Map? attachCard,
  }) async {
    final uploadId =
        "${Accounts.main.mid}_${DateTime.now().millisecondsSinceEpoch ~/ 1000}_${Utils.random.nextInt(9000) + 1000}";
    final res = await Request().post(
      Api.editDyn,
      queryParameters: await WbiSign.makSign({
        'platform': 'web',
        'csrf': Accounts.main.csrf,
        'x-bili-device-req-json':
            '{"platform":"web","device":"pc","spmid":"333.1368"}',
        'w_dyn_req.upload_id': uploadId,
        'w_dyn_req.meta':
            '{"app_meta":{"from":"create.dynamic.web","mobi_app":"web"}}',
      }),
      data: {
        "dyn_req": {
          "content": {
            "contents": [
              if (rawText != null)
                {
                  "raw_text": rawText,
                  "type": 1,
                  "biz_id": "",
                },
              ...?extraContent,
            ],
            if (title != null && title.isNotEmpty) 'title': title,
          },
          if (privatePub != null || replyOption != null)
            "option": {
              'private_pub': ?privatePub,
              if (replyOption == ReplyOptionType.close)
                "close_comment": 1
              else if (replyOption == ReplyOptionType.choose)
                "up_choose_comment": 1,
            },
          "scene": repostDynId != null
              ? 4
              : pics != null
              ? 2
              : 1,
          'pics': ?pics,
          "attach_card": attachCard,
          "upload_id": uploadId,
          "meta": {
            "app_meta": {"from": "create.dynamic.web", "mobi_app": "web"},
          },
          if (topic != null)
            "topic": {
              "id": topic.first,
              "name": topic.second,
              "from_source": "dyn.web.list",
              "from_topic_id": 0,
            },
        },
        "dyn_id_str": dynId.toString(),
        if (repostDynId != null)
          "web_repost_src": {"dyn_id_str": repostDynId.toString()},
      },
      options: Options(contentType: Headers.jsonContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }
}
