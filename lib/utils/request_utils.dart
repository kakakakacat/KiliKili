import 'dart:convert' show jsonEncode;
import 'dart:math';

import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/dialog/simple_dialog_option.dart';
import 'package:kilikili/common/widgets/selection_text.dart';
import 'package:kilikili/grpc/bilibili/im/type.pbenum.dart';
import 'package:kilikili/grpc/bilibili/main/community/reply/v1.pb.dart'
    show ReplyInfo;
import 'package:kilikili/grpc/im.dart';
import 'package:kilikili/http/dynamics.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/validate.dart';
import 'package:kilikili/models/dynamics/result.dart';
import 'package:kilikili/models/login/model.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/pages/dynamics_tab/controller.dart';
import 'package:kilikili/pages/fav_detail/controller.dart'
    show BaseFavController;
import 'package:kilikili/pages/group_panel/view.dart';
import 'package:kilikili/pages/login/geetest/geetest_webview_dialog.dart';
import 'package:kilikili/services/rust/rust_dynamic.dart';
import 'package:kilikili/services/rust/rust_relation.dart';
import 'package:kilikili/services/rust/rust_video_write.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/extension/context_ext.dart';
import 'package:kilikili/utils/extension/size_ext.dart';
import 'package:kilikili/utils/extension/string_ext.dart';
import 'package:kilikili/utils/extension/theme_ext.dart';
import 'package:kilikili/utils/feed_back.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_key.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/utils/theme_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:kilikili/services/rust/rust_user.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter/services.dart' show LengthLimitingTextInputFormatter;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

abstract final class RequestUtils {
  static Future<void> syncHistoryStatus() async {
    final account = Accounts.history;
    if (!account.isLogin) {
      return;
    }
    final res = await RustHistory.historyStatus(account: account);
    if (res case Success(:final response)) {
      GStorage.localCache.put(LocalCacheKey.historyPause, response);
    }
  }

  // 1：小视频（已弃用）
  // 2：相簿
  // 3：纯文字
  // 4：直播（此类型不常用，见分享其他内容消息）
  // 5：视频
  // 6：专栏
  // 7：番剧（id 为 season_id）
  // 8：音乐
  // 9：国产动画（id 为 AV 号）
  // 10：图片
  // 11：动态
  // 16：番剧（id 为 epid）
  // 17：番剧
  // https://github.com/SocialSisterYi/bilibili-API-collect/tree/master/docs/message/private_msg_content.md
  static Future<bool> pmShare({
    required int receiverId,
    required Map content,
    String? message,
  }) async {
    final ownerMid = Accounts.main.mid;
    final contentRes = await ImGrpc.sendMsg(
      senderUid: ownerMid,
      receiverId: receiverId,
      content: jsonEncode(content),
      msgType: content['source'] is String
          ? MsgType.EN_MSG_TYPE_COMMON_SHARE_CARD
          : MsgType.EN_MSG_TYPE_SHARE_V2,
    );

    if (contentRes.isSuccess) {
      if (message?.isNotEmpty == true) {
        final msgRes = await ImGrpc.sendMsg(
          senderUid: ownerMid,
          receiverId: receiverId,
          content: jsonEncode({"content": message}),
          msgType: MsgType.EN_MSG_TYPE_TEXT,
        );
        return msgRes.isSuccess;
      } else {
        return true;
      }
    } else {
      return false;
    }
  }

  static Future<void> createFavTag(
    BuildContext context,
    ValueChanged<({int tagid, String tagName})> onSuccess,
  ) async {
    String tagName = '';
    final onCreate = await showConfirmDialog(
      context: context,
      title: const Text('新建分组'),
      content: TextFormField(
        autofocus: true,
        initialValue: tagName,
        onChanged: (value) => tagName = value,
        inputFormatters: [
          LengthLimitingTextInputFormatter(16),
        ],
        decoration: const InputDecoration(border: OutlineInputBorder()),
      ),
    );
    if (onCreate) {
      final res = await RustRelation.createFollowTag(tagName);
      if (res case Success(:final response)) {
        onSuccess((tagid: response, tagName: tagName));
        SmartDialog.showToast('创建成功');
      } else {
        res.toast();
      }
    }
  }

  static Future<void> actionRelationMod({
    required BuildContext context,
    required dynamic mid,
    required bool isFollow,
    required ValueChanged<int>? afterMod,
    Relation? followStatus,
  }) async {
    if (mid == null) {
      return;
    }
    feedBack();
    if (!isFollow) {
      final res = await RustVideoWrite.relationMod(
        mid: mid,
        act: 1,
        reSrc: 11,
      );
      if (res.isSuccess) {
        SmartDialog.showToast('关注成功');
        afterMod?.call(2);
      } else {
        res.toast();
      }
    } else {
      if (followStatus?.tag == null) {
        final res = await RustRelation.userRelation(mid);
        if (res case Success(:final response)) {
          followStatus = response;
        } else {
          res.toast();
          return;
        }
      }

      if (context.mounted) {
        bool isSpecialFollowed = followStatus!.special == 1;
        // Rust 侧的 `Relation` 是不可变的（`tag` 还是 `Int64List`）。分组
        // 面板选完要把新分组带给下一次打开，所以这里取一份界面侧的可变副本。
        List<int>? tags = followStatus.tag?.toList();
        String text = isSpecialFollowed ? '移除特别关注' : '加入特别关注';
        showDialog(
          context: context,
          builder: (context) => SimpleDialog(
            clipBehavior: Clip.hardEdge,
            contentPadding: const EdgeInsets.symmetric(vertical: 12),
            children: [
              DialogOption(
                onPressed: () async {
                  Get.back();
                  final res = await RustRelation.specialAction(
                    fid: mid,
                    isAdd: !isSpecialFollowed,
                  );
                  if (res.isSuccess) {
                    SmartDialog.showToast('$text成功');
                    afterMod?.call(isSpecialFollowed ? 2 : -10);
                  } else {
                    res.toast();
                  }
                },
                child: Text(text, style: const TextStyle(fontSize: 14)),
              ),
              DialogOption(
                onPressed: () async {
                  Get.back();
                  final result = await showModalBottomSheet<Set<int>>(
                    context: context,
                    useSafeArea: true,
                    isScrollControlled: true,
                    constraints: BoxConstraints(
                      maxWidth: min(640, context.mediaQueryShortestSide),
                    ),
                    builder: (BuildContext context) {
                      final maxChildSize =
                          PlatformUtils.isMobile &&
                              !context.mediaQuerySize.isPortrait
                          ? 1.0
                          : 0.7;
                      return DraggableScrollableSheet(
                        minChildSize: 0,
                        maxChildSize: 1,
                        snap: true,
                        expand: false,
                        snapSizes: [maxChildSize],
                        initialChildSize: maxChildSize,
                        builder: (context, scrollController) {
                          return GroupPanel(
                            mid: mid,
                            tags: tags,
                            scrollController: scrollController,
                          );
                        },
                      );
                    },
                  );
                  if (result != null) {
                    tags = result.toList();
                    afterMod?.call(result.contains(-10) ? -10 : 2);
                  }
                },
                child: const Text('设置分组', style: TextStyle(fontSize: 14)),
              ),
              DialogOption(
                onPressed: () async {
                  Get.back();
                  final res = await RustVideoWrite.relationMod(
                    mid: mid,
                    act: 2,
                    reSrc: 11,
                  );
                  if (res.isSuccess) {
                    SmartDialog.showToast('取消关注成功');
                    afterMod?.call(0);
                  } else {
                    res.toast();
                  }
                },
                child: const Text('取消关注', style: TextStyle(fontSize: 14)),
              ),
            ],
          ),
        );
      }
    }
  }

  static ReplyInfo replyCast(Map res) {
    Map? emote = res['content']['emote'];
    emote?.forEach((key, value) {
      value['size'] = value['meta']['size'];
    });
    return ReplyInfo.create()..mergeFromProto3Json(
      res
        ..['content'].remove('members')
        ..['id'] = res['rpid']
        ..['member']['name'] = res['member']['uname']
        ..['member']['face'] = res['member']['avatar']
        ..['member']['level'] = res['member']['level_info']['current_level']
        ..['member']['vipStatus'] = res['member']['vip']['vipStatus']
        ..['member']['vipType'] = res['member']['vip']['vipType']
        ..['member']['officialVerifyType'] =
            res['member']['official_verify']['type']
        ..['content']['emotes'] = emote,
      ignoreUnknownFields: true,
    );
  }

  // static Future<dynamic> getWwebid(mid) async {
  //   try {
  //     final response = await Request().get(
  //       '${HttpString.spaceBaseUrl}/$mid/dynamic',
  //       options: Options(
  //         extra: {'account': AnonymousAccount()},
  //       ),
  //     );
  //     dom.Document document = html_parser.parse(response.data);
  //     dom.Element? scriptElement =
  //         document.querySelector('script#__RENDER_DATA__');
  //     return jsonDecode(
  //         Uri.decodeComponent(scriptElement?.text ?? ''))['access_id'];
  //   } catch (e) {
  //     if (kDebugMode) debugPrint('failed to get wwebid: $e');
  //     return null;
  //   }
  // }

  static Future<void> insertCreatedDyn(dynamic id) async {
    if (id != null) {
      try {
        await Future.pause(const Duration(milliseconds: 450));
        final res = await DynamicsHttp.dynamicDetail(id: id);
        if (res case final Success<DynamicItemModel> e) {
          final ctr = Get.find<DynamicsTabController>(tag: 'all');
          if (ctr.loadingState.value case Success(:final response?)) {
            response.insert(0, e.response);
            ctr.loadingState.refresh();
            return;
          }
          ctr.loadingState.value = Success([e.response]);
        }
      } catch (e) {
        if (kDebugMode) debugPrint('create dyn $e');
      }
    }
  }

  static Future<void> checkCreatedDyn({
    dynamic id,
    String? dynText,
    bool isManual = false,
  }) async {
    if (isManual || Pref.enableCreateDynAntifraud) {
      try {
        if (id != null) {
          if (!isManual) {
            await Future.pause(const Duration(seconds: 5));
          }
          final res = await DynamicsHttp.dynamicDetail(
            id: id,
            clearCookie: true,
          );
          final isSuccess = res.isSuccess;
          showDialog(
            context: Get.context!,
            barrierDismissible: isManual,
            builder: (context) {
              final colorScheme = ColorScheme.of(context);
              final color = isSuccess ? colorScheme.primary : colorScheme.error;
              final actions = [
                if (!isSuccess)
                  TextButton(
                    onPressed: () {
                      Get.back();
                      Utils.copyText('https://www.bilibili.com/opus/$id');
                      Get.toNamed(
                        '/webview',
                        parameters: {
                          'url':
                              'https://www.bilibili.com/h5/comment/appeal?${ThemeUtils.themeUrl(colorScheme.isDark)}',
                        },
                      );
                    },
                    child: const Text('申诉'),
                  ),
                if (!isManual)
                  TextButton(
                    onPressed: Get.back,
                    child: Text(
                      '关闭',
                      style: TextStyle(color: colorScheme.outline),
                    ),
                  ),
              ];
              return AlertDialog(
                title: Text.rich(
                  TextSpan(
                    children: [
                      WidgetSpan(
                        alignment: .middle,
                        child: isSuccess
                            ? Icon(
                                size: 22,
                                color: color,
                                Icons.check_circle_outline_rounded,
                              )
                            : Icon(
                                size: 22,
                                color: color,
                                Icons.highlight_off_outlined,
                              ),
                      ),
                      TextSpan(
                        text: ' 动态检查结果',
                        style: TextStyle(color: color),
                      ),
                    ],
                  ),
                ),
                content: SelectionText(
                  '${isSuccess ? '无账号状态下找到了你的动态，动态正常！' : '你的动态被shadow ban（仅自己可见）！'}${dynText != null ? ' \n\n动态内容: $dynText' : ''}',
                ),
                actions: actions.isEmpty ? null : actions,
              );
            },
          );
        }
      } catch (e) {
        if (kDebugMode) debugPrint('check dyn error: $e');
      }
    }
  }

  // 动态点赞
  static Future<void> onLikeDynamic(
    DynamicItemModel item,
    bool uiStatus,
    VoidCallback onSuccess,
  ) async {
    feedBack();

    final like = item.modules.moduleStat?.like;
    final status = like?.status ?? false;

    if (status ^ uiStatus) {
      SmartDialog.showToast(status ? '点赞成功' : '取消赞');
      onSuccess();
      return;
    }

    final res = await RustDynamic.thumb(
      dynamicId: item.idStr!,
      up: status ? 2 : 1, // 1 已点赞 2 不喜欢 0 未操作
    );
    if (res.errorKind == null) {
      SmartDialog.showToast(status ? '取消赞' : '点赞成功');
      like
        ?..count = (like.count ?? 0) + (status ? -1 : 1)
        ..status = !status;
      onSuccess();
    } else {
      SmartDialog.showToast(res.errorDetail ?? '动态点赞失败');
    }
  }

  /// 把选中的稍后再看复制/移动进某个收藏夹（S7 #57）。
  ///
  /// 与下面那个泛型版本**几乎一样**，分开是因为稍后再看的列表项已经是
  /// Rust 的不可变结构、选中态按 id 记，而泛型那版依赖
  /// `MultiSelectData.checked` 与对象相等。收藏夹那片迁到 Rust 之后两者
  /// 应当合并，届时删掉泛型那版——**别在这期间给它加第三个分支**。
  static void onCopyOrMoveLater({
    required BuildContext context,
    required bool isCopy,
    required IdMultiSelectMixin<LaterPage, LaterItem, int> ctr,
    required dynamic mid,
  }) {
    RustFav.allFavFolders(mid).then((res) {
      if (!context.mounted) return;
      if (res case Success(:final response)) {
        final list = response.list;
        if (list.isEmpty) return;
        int? checkedId;
        showDialog(
          context: context,
          builder: (context) {
            return AlertDialog(
              title: Text('${isCopy ? '复制' : '移动'}到'),
              contentPadding: const EdgeInsets.only(top: 5),
              content: SingleChildScrollView(
                child: RadioGroup(
                  onChanged: (value) {
                    checkedId = value;
                    (context as Element).markNeedsBuild();
                  },
                  groupValue: checkedId,
                  child: Column(
                    children: list.map((item) {
                      return RadioListTile<int>(
                        dense: true,
                        title: Text(item.title),
                        value: item.id,
                      );
                    }).toList(),
                  ),
                ),
              ),
              actions: [
                TextButton(
                  onPressed: Get.back,
                  child: Text(
                    '取消',
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.outline,
                    ),
                  ),
                ),
                TextButton(
                  onPressed: () {
                    if (checkedId == null) return;
                    final removed = ctr.allChecked.map(ctr.idOf).toSet();
                    SmartDialog.showLoading();
                    RustFav.copyOrMoveFav(
                      isCopy: isCopy,
                      isFav: false,
                      srcMediaId: null,
                      tarMediaId: checkedId!,
                      resources: removed.join(','),
                      mid: isCopy ? mid : null,
                    ).then((res) {
                      if (res.isSuccess) {
                        ctr.handleSelect(checked: false);
                        if (!isCopy) {
                          ctr.afterDeleteIds(removed);
                        }
                        SmartDialog.dismiss();
                        SmartDialog.showToast('${isCopy ? '复制' : '移动'}成功');
                        Get.back();
                      } else {
                        SmartDialog.dismiss();
                        res.toast();
                      }
                    });
                  },
                  child: const Text('确认'),
                ),
              ],
            );
          },
        );
      } else {
        res.toast();
      }
    });
  }

  /// 收藏夹内的复制 / 移动。
  ///
  /// 只服务收藏夹那两个页面（详情与夹内搜索），所以直接收
  /// [BaseFavController] 而不是一个泛型的多选控制器：泛型版本当初是为了
  /// 兼容 `MultiSelectData`，而列表项迁到 Rust 之后那条路已经走不通
  /// （不可就地改，选中态按 id 记）。
  static void onCopyOrMove({
    required BuildContext context,
    required bool isCopy,
    required BaseFavController ctr,
    required dynamic mediaId,
    required dynamic mid,
  }) {
    RustFav.allFavFolders(mid).then((res) {
      if (!context.mounted) return;
      if (res case Success(:final response)) {
        final list = response.list;
        if (list.isEmpty) return;
        int? checkedId;
        showDialog(
          context: context,
          builder: (context) {
            return AlertDialog(
              title: Text('${isCopy ? '复制' : '移动'}到'),
              contentPadding: const EdgeInsets.only(top: 5),
              content: SingleChildScrollView(
                child: RadioGroup(
                  onChanged: (value) {
                    checkedId = value;
                    (context as Element).markNeedsBuild();
                  },
                  groupValue: checkedId,
                  child: Column(
                    children: list.map((item) {
                      return RadioListTile<int>(
                        dense: true,
                        title: Text(item.title),
                        value: item.id,
                      );
                    }).toList(),
                  ),
                ),
              ),
              actions: [
                TextButton(
                  onPressed: Get.back,
                  child: Text(
                    '取消',
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.outline,
                    ),
                  ),
                ),
                TextButton(
                  onPressed: () {
                    if (checkedId != null) {
                      // 倒序照搬基线（`allChecked.toList().reversed`）。
                      // 它为什么反着发没有记载，多半是因为服务端按收到的
                      // 顺序逐条插入目标夹子；没有验证过，所以**不要顺手
                      // 改成正序**——改错了不报错，只是目标夹子里的顺序反了。
                      final removeList = ctr.allChecked.toList().reversed
                          .toList();
                      SmartDialog.showLoading();
                      RustFav.copyOrMoveFav(
                        isCopy: isCopy,
                        isFav: true,
                        srcMediaId: mediaId,
                        tarMediaId: checkedId!,
                        resources: removeList
                            .map((e) => '${e.id}:${e.kind}')
                            .join(','),
                        mid: isCopy ? mid : null,
                      ).then((res) {
                        if (res.isSuccess) {
                          ctr.handleSelect(checked: false);
                          if (!isCopy) {
                            ctr.afterDeleteIds(
                              removeList.map(ctr.idOf).toSet(),
                            );
                            ctr.updateCount?.call(removeList.length);
                          }
                          SmartDialog.dismiss();
                          SmartDialog.showToast('${isCopy ? '复制' : '移动'}成功');
                          Get.back();
                        } else {
                          SmartDialog.dismiss();
                          res.toast();
                        }
                      });
                    }
                  },
                  child: const Text('确认'),
                ),
              ],
            );
          },
        );
      } else {
        res.toast();
      }
    });
  }

  static Future<void> validate(
    String vVoucher,
    ValueChanged<String> onSuccess,
  ) async {
    final res = await ValidateHttp.gaiaVgateRegister(vVoucher);
    if (!res.isSuccess) {
      res.toast();
      return;
    }

    final resData = res.data;
    if (resData == null) {
      SmartDialog.showToast("null data");
      return;
    }

    CaptchaDataModel captchaData = CaptchaDataModel();

    final geetest = resData['geetest'];
    String? gt = geetest?['gt'];
    String? challenge = geetest?['challenge'];
    captchaData.token = resData['token'];

    bool isGeeArgumentValid() {
      return gt?.isNotEmpty == true &&
          challenge?.isNotEmpty == true &&
          captchaData.token?.isNotEmpty == true;
    }

    if (!isGeeArgumentValid()) {
      SmartDialog.showToast("参数为空");
      return;
    }

    Future<void> gaiaVgateValidate() async {
      final res = await ValidateHttp.gaiaVgateValidate(
        challenge: captchaData.geetest?.challenge,
        seccode: captchaData.seccode,
        token: captchaData.token,
        validate: captchaData.validate,
      );
      if (res case Success(:final response?)) {
        if (response['is_valid'] == 1) {
          final griskId = response['grisk_id'];
          if (griskId is String) {
            onSuccess(griskId);
          }
        } else {
          SmartDialog.showToast('invalid');
        }
      } else {
        res.toast();
      }
    }

    final json = await GeetestWebviewDialog.geetest(gt!, challenge!);
    if (json != null) {
      captchaData
        ..validate = json['geetest_validate']
        ..seccode = json['geetest_seccode']
        ..geetest = GeetestData(
          challenge: json['geetest_challenge'],
          gt: gt,
        );
      gaiaVgateValidate();
    }
  }

  static Future<void> showUserRealName(String mid) async {
    // 基线这里收的是字符串 mid（路由参数直给）。Rust 侧的签名是 i64——
    // 协议上它就是个数字，让类型在边界处收紧，解析不出来就不发请求。
    final id = int.tryParse(mid);
    if (id == null) return;
    final res = await RustUser.getUserRealName(id);
    if (res case Success(:final response)) {
      final show = !response.name.isNullOrEmpty;
      showDialog(
        context: Get.context!,
        builder: (context) => AlertDialog(
          title: SelectionText(
            show ? response.name! : response.rejectPage?.title ?? '',
          ),
          content: show ? null : Text(response.rejectPage?.text ?? ''),
          actions: [
            TextButton(
              onPressed: Get.back,
              child: const Text('关闭'),
            ),
          ],
        ),
      );
    } else {
      res.toast();
    }
  }
}
