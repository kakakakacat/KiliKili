import 'package:kilikili/common/assets.dart';
import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/dialog/simple_dialog_option.dart';
import 'package:kilikili/common/widgets/flutter/list_tile.dart';
import 'package:kilikili/common/widgets/pendant_avatar.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/whisper_secondary/view.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:kilikili/services/rust/rust_msg.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/extension/num_ext.dart';
import 'package:kilikili/utils/extension/theme_ext.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart' hide ListTile;

/// 私信列表里的一条。
///
/// 这个 widget 在 S12 续单（#63）里瘦了一圈：头像三件套（主图 / 挂件 /
/// 认证标按图层位置取）、折叠入口的页面类型映射、`vip_info` 那段 JSON，
/// 以前都在这里现算，现在由 Rust 投影好送过来（`bili_api::grpc_im`）。
/// 按 AGENTS.md §2，那三段是「换掉宿主要原样重写一遍」的决定，不该住在
/// widget 里。
///
/// 留下的是真正属于界面的部分：布局、手势、菜单与跳转。
class WhisperSessionItem extends StatelessWidget {
  const WhisperSessionItem({
    super.key,
    required this.item,
    required this.index,
    required this.onSetTop,
    required this.onSetMute,
    required this.onRemove,
    required this.onAcked,
  });

  final ImSession item;

  /// 列表里的位置。
  ///
  /// 置顶与免打扰成功后要**换掉列表里那一条**（FRB 导出的类型不可变，
  /// 字段全是 final），所以回调要知道改哪一条。基线靠就地改 protobuf
  /// 对象，不需要它。
  final int index;
  final void Function(ImSession item, int index, bool isTop) onSetTop;
  final void Function(ImSession item, int index, bool isMuted) onSetMute;
  final ValueChanged<int> onRemove;

  /// 「标为已读」或点进会话后通知上层清掉那一条的未读。
  final ValueChanged<int> onAcked;

  bool get _hasUnread => item.unreadStyle != ImUnreadStyle.none;

  Future<void> _updateAck() async {
    final talkerUid = item.target.talkerUid;
    if (talkerUid == null) return;
    // 两步：先取这个会话的已读位点，再拿它去标记。位点由服务端定，
    // 本地算不出来。
    final seqno = await RustIm.ackSeqno(talkerUid);
    if (seqno case Success(:final response)) {
      final res = await RustMsg.ackSessionMsg(
        talkerId: talkerUid,
        ackSeqno: response,
      );
      if (res.isSuccess) {
        SmartDialog.showToast('已标为已读');
        onAcked(index);
      } else {
        res.toast();
      }
    } else {
      seqno.toast();
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isPrivate = item.target.kind == ImSessionTargetKind.private;

    return ListTile(
      safeArea: true,
      tileColor: item.isPinned
          ? theme.colorScheme.onInverseSurface.withValues(
              alpha: theme.isDark ? 0.4 : 0.8,
            )
          : null,
      onLongPress: () => showDialog(
        context: context,
        builder: (_) => SimpleDialog(
          clipBehavior: .hardEdge,
          contentPadding: const .symmetric(vertical: 12),
          children: [
            DialogOption(
              onPressed: () {
                Get.back();
                onSetTop(item, index, item.isPinned);
              },
              child: Text(item.isPinned ? '移除置顶' : '置顶'),
            ),
            if (isPrivate) ...[
              if (kDebugMode || _hasUnread)
                DialogOption(
                  onPressed: () {
                    Get.back();
                    _updateAck();
                  },
                  child: const Text('标为已读'),
                ),
              DialogOption(
                onPressed: () {
                  Get.back();
                  onSetMute(item, index, item.isMuted);
                },
                child: Text('${item.isMuted ? '关闭' : '开启'}免打扰'),
              ),
              DialogOption(
                onPressed: () {
                  Get.back();
                  showConfirmDialog(
                    context: context,
                    title: const Text('确定删除该对话？'),
                    onConfirm: () => onRemove(index),
                  );
                },
                child: const Text('删除'),
              ),
            ],
          ],
        ),
      ),
      onSecondaryTapUp: PlatformUtils.isDesktop
          ? (details) => showMenu(
              context: context,
              position: PageUtils.menuPosition(details.globalPosition),
              items: <PopupMenuEntry<Never>>[
                PopupMenuItem(
                  height: 42,
                  onTap: () => onSetTop(item, index, item.isPinned),
                  child: Text(item.isPinned ? '移除置顶' : '置顶'),
                ),
                if (isPrivate) ...[
                  if (kDebugMode || _hasUnread)
                    PopupMenuItem(
                      height: 42,
                      onTap: _updateAck,
                      child: const Text('标为已读'),
                    ),
                  PopupMenuItem(
                    height: 42,
                    onTap: () => onSetMute(item, index, item.isMuted),
                    child: Text('${item.isMuted ? '关闭' : '开启'}免打扰'),
                  ),
                  const PopupMenuDivider(height: 10),
                  PopupMenuItem(
                    height: 42,
                    onTap: () => showConfirmDialog(
                      context: context,
                      title: const Text('确定删除该对话？'),
                      onConfirm: () => onRemove(index),
                    ),
                    child: Text(
                      '删除',
                      style: TextStyle(color: theme.colorScheme.error),
                    ),
                  ),
                ],
              ],
            )
          : null,
      onTap: () {
        if (_hasUnread) {
          onAcked(index);
        }
        switch (item.target.kind) {
          case ImSessionTargetKind.private:
            Get.toNamed(
              '/whisperDetail',
              arguments: {
                'talkerId': item.target.talkerUid,
                'name': item.name,
                'face': item.avatarUrl,
                if (item.mid != null) 'mid': item.mid,
                'isLive': item.isLive,
              },
            );
          case ImSessionTargetKind.fold:
            // `foldPageType` 是 Rust 映射好的**页面**类型。映射不出来时
            // 它是 null，此时按基线 toast 出枚举名而不是猜一个页面——
            // 猜错了会打开另一个列表，而且不报错。
            final pageType = item.target.foldPageType;
            if (pageType != null) {
              Get.to(
                WhisperSecPage(name: item.name, sessionPageType: pageType),
              );
            } else {
              SmartDialog.showToast(item.target.typeName ?? '未知会话类型');
            }
          case ImSessionTargetKind.system:
            // 基线只有「系统消息」这一种有去处，其余各类都只是 toast。
            if (item.target.typeName == 'SESSION_TYPE_SYSTEM') {
              Get.toNamed('/sysMsg');
            } else {
              SmartDialog.showToast(item.target.typeName ?? '未知会话类型');
            }
          case ImSessionTargetKind.group:
          case ImSessionTargetKind.customer:
          case ImSessionTargetKind.unknown:
            // 基线这三类点了也什么都不做。
            break;
        }
      },
      leading: GestureDetector(
        onTap: item.mid != null
            ? () => Get.toNamed('/member?mid=${item.mid}')
            : null,
        child: PendantAvatar(
          item.avatarUrl,
          size: 42,
          badgeSize: 14,
          pendantImage: item.pendantUrl,
          vipStatus: item.vipStatus,
          officialType: item.officialType,
        ),
      ),
      title: Row(
        spacing: 5,
        children: [
          Expanded(
            child: Row(
              spacing: 5,
              children: [
                Flexible(
                  child: Text(
                    item.name,
                    maxLines: 1,
                    overflow: .ellipsis,
                    style: TextStyle(
                      fontSize: 15,
                      color: (item.vipStatus ?? 0) > 0 && item.vipType == 2
                          ? theme.colorScheme.vipColor
                          : null,
                    ),
                  ),
                ),
                if (item.labelText case final label?)
                  PBadge(
                    isStack: false,
                    type: .line_secondary,
                    size: .small,
                    fontSize: 10,
                    isBold: false,
                    text: label,
                  ),
                if (item.isLive)
                  Image.asset(
                    Assets.livingRect,
                    height: 15,
                    cacheHeight: 15.cacheSize(context),
                    filterQuality: .low,
                  ),
              ],
            ),
          ),
          if (item.timestampUs case final ts?)
            Text(
              // 协议给的是**微秒**，不是毫秒也不是秒。
              DateFormatUtils.dateFormat(ts ~/ 1000000),
              style: TextStyle(
                fontSize: 12,
                color: theme.colorScheme.outline,
              ),
            ),
        ],
      ),
      subtitle: Row(
        children: [
          Expanded(
            child: Text(
              item.summary ?? '',
              maxLines: 1,
              overflow: .ellipsis,
              style: theme.textTheme.labelMedium!.copyWith(
                color: theme.colorScheme.outline,
              ),
            ),
          ),
          if (item.isMuted)
            Icon(
              size: 16,
              Icons.notifications_off,
              color: theme.colorScheme.outline,
            )
          else if (_hasUnread)
            Badge(
              label: item.unreadStyle == ImUnreadStyle.number
                  ? Text(item.unreadNumber.toString())
                  : null,
            ),
        ],
      ),
    );
  }
}
