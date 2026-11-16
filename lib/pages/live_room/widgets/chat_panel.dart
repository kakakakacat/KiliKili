import 'package:kilikili/utils/global_data.dart';
import 'package:kilikili/pages/danmaku/danmaku_model.dart';
import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/flutter/live_list_view.dart';
import 'package:kilikili/common/widgets/flutter/popup_menu.dart';
import 'package:kilikili/common/widgets/gesture/tap_gesture_recognizer.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/common/widgets/scroll_physics.dart'
    show platformClampingPhysics;
import 'package:kilikili/http/live.dart';
import 'package:kilikili/rust/api/live_danmaku.dart';
import 'package:kilikili/pages/live_room/superchat/super_chat_state.dart';
import 'package:kilikili/pages/live_room/controller.dart';
import 'package:kilikili/pages/live_room/superchat/superchat_card.dart';
import 'package:kilikili/pages/member/widget/medal_widget.dart';
import 'package:kilikili/pages/video/widgets/header_control.dart';
import 'package:kilikili/utils/extension/theme_ext.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class LiveRoomChatPanel extends StatelessWidget {
  const LiveRoomChatPanel({
    super.key,
    required this.liveRoomController,
    required this.isPP,
  });

  final LiveRoomController liveRoomController;
  final bool isPP;

  bool get disableAutoScroll => liveRoomController.disableAutoScroll.value;

  @override
  Widget build(BuildContext context) {
    late final bg = isPP
        ? Colors.black.withValues(alpha: 0.4)
        : const Color(0x15FFFFFF);
    late final nameColor = isPP
        ? Colors.white.withValues(alpha: 0.9)
        : Colors.white.withValues(alpha: 0.6);
    late final devicePixelRatio = MediaQuery.devicePixelRatioOf(context);
    late final colorScheme = ColorScheme.of(context);
    late final primary = colorScheme.isDark
        ? colorScheme.primary
        : colorScheme.inversePrimary;
    return Stack(
      children: [
        Obx(
          () => LiveListView.separated(
            key: const PageStorageKey(LiveRoomChatPanel),
            // multiply by 2 to account for separators
            initialIndex: liveRoomController.trimDmIndex * 2,
            padding: const .symmetric(horizontal: 12),
            controller: liveRoomController.scrollController,
            separatorBuilder: (_, _) => const SizedBox(height: 8),
            itemCount: liveRoomController.builtLength =
                liveRoomController.messages.length,
            physics: platformClampingPhysics,
            itemBuilder: (_, index) {
              liveRoomController.chatSimpleIndex = index;
              final item = liveRoomController.messages[index];
              if (item is LiveDanmakuMessage) {
                WidgetSpan? medal;
                if ((GlobalData().showMedal ? item.medal : null)
                    case final medalInfo?) {
                  try {
                    medal = WidgetSpan(
                      child: Padding(
                        padding: const .only(right: 4),
                        child: MedalWidget.fromMedalInfo(
                          medal: medalInfo,
                          padding: MedalWidget.mediumPadding,
                        ),
                      ),
                    );
                  } catch (e, s) {
                    if (kDebugMode) {
                      Utils.reportError(e, s);
                    }
                  }
                }
                return Align(
                  alignment: Alignment.centerLeft,
                  child: Builder(
                    builder: (itemContext) {
                      return Container(
                        padding: const .symmetric(horizontal: 10, vertical: 4),
                        decoration: BoxDecoration(
                          color: bg,
                          borderRadius: const .all(.circular(14)),
                        ),
                        child: Text.rich(
                          TextSpan(
                            children: [
                              if (item.mid == liveRoomController.ruid)
                                const WidgetSpan(
                                  child: Padding(
                                    padding: .only(right: 4),
                                    child: PBadge(
                                      text: '主播',
                                      isStack: false,
                                      type: .line_primary,
                                    ),
                                  ),
                                ),
                              ?medal,
                              TextSpan(
                                text: '${item.name}: ',
                                style: TextStyle(
                                  color: nameColor,
                                  fontSize: 14,
                                ),
                                recognizer: item.mid == 0
                                    ? null
                                    : (NoDeadlineTapGestureRecognizer()
                                        ..onTapUp = (e) => _showMsgMenu(
                                          context,
                                          itemContext,
                                          e,
                                          item,
                                        )),
                              ),
                              if (item.reply case final reply?)
                                TextSpan(
                                  text: '@${reply.name} ',
                                  style: TextStyle(
                                    color: primary,
                                    fontSize: 14,
                                  ),
                                  recognizer: NoDeadlineTapGestureRecognizer()
                                    ..onTap = () =>
                                        Get.toNamed('/member?mid=${reply.mid}'),
                                ),
                              _buildMsg(devicePixelRatio, item),
                            ],
                          ),
                        ),
                      );
                    },
                  ),
                );
              }
              if (item is LiveSuperChatState) {
                return SuperChatCard(
                  item: item,
                  persistentSC: true,
                  onReport: () => liveRoomController.reportSC(item),
                );
              }
              return null;
            },
          ),
        ),
        if (kDebugMode && liveRoomController.showSuperChat) ...[
          Positioned(
            top: 50,
            right: 0,
            child: TextButton(
              onPressed: () {
                final item = LiveSuperChatState.random;
                liveRoomController
                  ..superChatMsg.insert(0, item)
                  ..addDm(item);
              },
              child: const Text('add superchat'),
            ),
          ),
          Positioned(
            right: 0,
            top: 90,
            child: TextButton(
              onPressed: () {
                if (liveRoomController.superChatMsg.isNotEmpty) {
                  liveRoomController.superChatMsg.removeLast();
                }
              },
              child: const Text('remove superchat'),
            ),
          ),
        ],
        if (liveRoomController.showSuperChat)
          Positioned(
            top: 12,
            right: 12,
            child: Obx(() {
              final isEmpty = liveRoomController.superChatMsg.isEmpty;
              return AnimatedOpacity(
                opacity: isEmpty ? 0 : 1,
                duration: const Duration(milliseconds: 120),
                child: GestureDetector(
                  onTap: isEmpty
                      ? null
                      : () => liveRoomController.pageController?.animateToPage(
                          1,
                          duration: const Duration(milliseconds: 200),
                          curve: Curves.easeInOut,
                        ),
                  child: Container(
                    decoration: BoxDecoration(
                      borderRadius: const BorderRadius.all(Radius.circular(8)),
                      color: const Color(0x2FFFFFFF),
                      border: Border.all(color: Colors.white24, width: 0.7),
                    ),
                    padding: const EdgeInsets.fromLTRB(10, 4, 4, 4),
                    child: Text.rich(
                      style: const TextStyle(color: Colors.white, height: 1),
                      strutStyle: const StrutStyle(height: 1, leading: 0),
                      TextSpan(
                        children: [
                          TextSpan(
                            text:
                                'SC(${liveRoomController.superChatMsg.length})',
                          ),
                          const WidgetSpan(
                            alignment: PlaceholderAlignment.middle,
                            child: Icon(
                              size: 18,
                              Icons.keyboard_arrow_right,
                              color: Colors.white,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              );
            }),
          ),
        Obx(
          () => liveRoomController.disableAutoScroll.value
              ? Positioned(
                  right: 12,
                  bottom: 0,
                  child: ElevatedButton.icon(
                    style: const ButtonStyle(visualDensity: .comfortable),
                    icon: const Icon(Icons.arrow_downward_rounded, size: 20),
                    label: const Text('回到底部'),
                    onPressed: liveRoomController.handleJumpToBottom,
                  ),
                )
              : const SizedBox.shrink(),
        ),
      ],
    );
  }

  InlineSpan _buildMsg(double devicePixelRatio, LiveDanmakuMessage obj) {
    final uemote = obj.uemote;
    if (uemote != null) {
      // "room_{{room_id}}_{{int}}" , "upower_[{{emote}}]" , "official_{{int}}"
      final double width, height;
      if (uemote.isOfficial) {
        width = uemote.width / devicePixelRatio;
        height = uemote.height / devicePixelRatio;
      } else {
        width = height = 162.0 / devicePixelRatio;
      }
      return WidgetSpan(
        child: NetworkImgLayer(
          src: uemote.url,
          type: .emote,
          width: width,
          height: height,
        ),
      );
    }
    return TextSpan(
      children: [
        for (final segment in obj.segments)
          if (segment.emoteIndex case final index?)
            WidgetSpan(
              child: NetworkImgLayer(
                src: obj.emotes[index].emote.url,
                type: .emote,
                width: obj.emotes[index].emote.width,
                height: obj.emotes[index].emote.height,
              ),
            )
          else
            TextSpan(
              text: segment.text,
              style: const TextStyle(color: Colors.white, fontSize: 14),
            ),
      ],
    );
  }

  void _showMsgMenu(
    BuildContext context,
    BuildContext itemContext,
    TapUpDetails details,
    LiveDanmakuMessage item,
  ) {
    final dx = details.globalPosition.dx;
    final renderBox = itemContext.findRenderObject() as RenderBox;
    final dy =
        details.globalPosition.dy -
        details.localPosition.dy +
        renderBox.size.height -
        4; // padding
    final autoScroll =
        liveRoomController.autoScroll &&
        !liveRoomController.disableAutoScroll.value;
    if (autoScroll) {
      liveRoomController.autoScroll = false;
    }
    showMenu(
      context: context,
      position: RelativeRect.fromLTRB(dx, dy, dx, 0),
      items: <PopupMenuEntry<Never>>[
        CustomPopupMenuItem(
          height: 38,
          child: Text(item.name, style: const TextStyle(fontSize: 13)),
        ),
        const CustomPopupMenuDivider(height: 1),
        PopupMenuItem(
          height: 38,
          onTap: () => Utils.copyText(
            Utils.jsonEncoder.convert({
              'id': item.id,
              'mid': item.mid,
              'name': item.name,
              'text': item.text,
              'dm_type': item.dmType,
              'ts': item.ts,
              'ct': item.ct,
              'color': item.color,
              'mode': item.mode,
              'emotes': [
                for (final entry in item.emotes)
                  {
                    'key': entry.key,
                    'url': entry.emote.url,
                    'is_official': entry.emote.isOfficial,
                    'emoticon_unique': entry.emote.emoticonUnique,
                    'width': entry.emote.width,
                    'height': entry.emote.height,
                  },
              ],
              if (item.reply case final reply?)
                'reply': {
                  'mid': reply.mid,
                  'name': reply.name,
                },
              if (item.medal case final medal?)
                'medal': {
                  'name': medal.name,
                  'level': medal.level,
                  'id': medal.id,
                  'ruid': medal.ruid,
                  'color_start': medal.colorStart,
                  'color_text': medal.colorText,
                },
            }),
          ),
          child: const Text('复制弹幕信息', style: TextStyle(fontSize: 13)),
        ),
        PopupMenuItem(
          height: 38,
          onTap: () => Get.toNamed('/member?mid=${item.mid}'),
          child: const Text('去TA的个人空间', style: TextStyle(fontSize: 13)),
        ),
        if (liveRoomController.isLogin) ...[
          PopupMenuItem(
            height: 38,
            onTap: () => liveRoomController.onAtUser(item),
            child: const Text('@TA', style: TextStyle(fontSize: 13)),
          ),
          PopupMenuItem(
            height: 38,
            onTap: () async {
              final res = await LiveHttp.liveShieldUser(
                uid: item.mid,
                roomid: liveRoomController.roomId,
                type: 1,
              );
              if (res.isSuccess) {
                SmartDialog.showToast('屏蔽成功');
              } else {
                res.toast();
              }
            },
            child: const Text('屏蔽发送者', style: TextStyle(fontSize: 13)),
          ),
          PopupMenuItem(
            height: 38,
            onTap: () => HeaderControl.reportLiveDanmaku(
              context,
              roomId: liveRoomController.roomId,
              msg: item.text,
              extra: LiveDanmaku(
                id: item.id,
                mid: item.mid,
                dmType: item.dmType,
                ts: item.ts,
                ct: item.ct,
              ),
            ),
            child: const Text('举报选中弹幕', style: TextStyle(fontSize: 13)),
          ),
        ],
      ],
    ).whenComplete(() {
      if (autoScroll && context.mounted) {
        liveRoomController
          ..autoScroll = true
          ..scrollToBottom();
      }
    });
  }
}
