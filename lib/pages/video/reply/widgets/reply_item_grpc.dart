import 'dart:math';

import 'package:kilikili/common/assets.dart';
import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/custom_icon.dart';
import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/dialog/report.dart';
import 'package:kilikili/common/widgets/emote_tooltip.dart';
import 'package:kilikili/common/widgets/gesture/tap_gesture_recognizer.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/common/widgets/image_grid/image_grid_view.dart';
import 'package:kilikili/common/widgets/pendant_avatar.dart';
import 'package:kilikili/common/widgets/text_ellipsis/text_ellipsis.dart';
import 'package:kilikili/common/widgets/text_more/text_more.dart';
import 'package:kilikili/common/widgets/translucent_row.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/reply.dart';
import 'package:kilikili/pages/article/view.dart';
import 'package:kilikili/pages/dynamics/widgets/vote.dart';
import 'package:kilikili/pages/member/widget/medal_widget.dart';
import 'package:kilikili/pages/save_panel/view.dart';
import 'package:kilikili/pages/video/controller.dart';
import 'package:kilikili/pages/video/reply/widgets/reply_threaded_indent_box.dart';
import 'package:kilikili/pages/video/reply/widgets/zan_grpc.dart';
import 'package:kilikili/pages/video/reply_reply/tree.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:kilikili/services/rust/rust_video_write.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/app_scheme.dart';
import 'package:kilikili/utils/bili_utils.dart';
import 'package:kilikili/utils/color_utils.dart';
import 'package:kilikili/utils/danmaku_utils.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/extension/context_ext.dart';
import 'package:kilikili/utils/extension/iterable_ext.dart';
import 'package:kilikili/utils/extension/num_ext.dart';
import 'package:kilikili/utils/extension/selectable_region_ext.dart';
import 'package:kilikili/utils/extension/theme_ext.dart';
import 'package:kilikili/utils/feed_back.dart';
import 'package:kilikili/utils/global_data.dart';
import 'package:kilikili/utils/image_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_key.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/utils/theme_utils.dart';
import 'package:kilikili/utils/url_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:cached_network_image_ce/cached_network_image.dart';
import 'package:collection/collection.dart' show IterableExtension;
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

part 'package:kilikili/common/widgets/context_menu/reply_menu_helper.dart';

const _replyTextButtonStyle = ButtonStyle(
  visualDensity: VisualDensity.compact,
  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
  padding: WidgetStatePropertyAll(EdgeInsets.zero),
);

final _colorsLight = [
  Colors.redAccent,
  Colors.orangeAccent,
  Colors.yellowAccent.shade400,
  Colors.greenAccent,
  Colors.cyanAccent,
  Colors.blueAccent,
  Colors.purpleAccent,
];

final _colorsDark = [
  Colors.red.shade900,
  Colors.orange.shade900,
  Colors.yellow.shade800,
  Colors.green.shade900,
  Colors.cyan.shade900,
  Colors.blue.shade900,
  Colors.deepPurple.shade900,
];

Widget _replyItemLayout({
  required BuildContext context,
  required Widget content,
  int depth = -1,
  bool needDivider = true,
  VoidCallback? onTap,
  VoidCallback? onLongPress,
  VoidCallback? onTapLine,
  VoidCallback? onScrollLeft,
}) {
  final colorScheme = ColorScheme.of(context);
  Widget child = Padding(
    padding: const EdgeInsets.fromLTRB(12, 14, 8, 5),
    child: content,
  );

  if (depth >= 0) {
    child = ReplyThreadedIndentBox(
      depth: depth,
      colors: colorScheme.isLight ? _colorsLight : _colorsDark,
      divColor: colorScheme.outline.withValues(alpha: 0.08),
      onTapLine: onTapLine,
      // onTapDot: (i) => debugPrint('onTapDot: $i'),
      onScrollLeft: onScrollLeft,
      child: child,
    );
  }

  if (needDivider) {
    child = Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        child,
        Divider(
          indent: 55,
          endIndent: 15,
          height: 0.3,
          color: colorScheme.outline.withValues(alpha: 0.08),
        ),
      ],
    );
  }

  return Material(
    type: MaterialType.transparency,
    child: InkWell(
      onTap: onTap,
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      child: child,
    ),
  );
}

Widget _replyContentLayout({
  required Widget header,
  required Widget message,
  bool noPad = false,
  Widget? voteOption,
  Widget? pictures,
  Widget? actions,
  Widget? subReplies,
}) {
  final EdgeInsets padding = noPad
      ? const .symmetric(horizontal: 6)
      : const .only(left: 45, right: 6);

  return Column(
    mainAxisSize: MainAxisSize.min,
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      header,
      const SizedBox(height: 10),
      if (voteOption != null) Padding(padding: padding, child: voteOption),
      Padding(padding: padding, child: message),
      if (pictures != null) ...[
        Padding(padding: padding, child: pictures),
        const SizedBox(height: 4),
      ],
      if (actions != null) ...[
        const SizedBox(height: 4),
        actions,
      ],
      if (subReplies != null)
        Padding(padding: const .only(top: 5, bottom: 12), child: subReplies),
    ],
  );
}

Widget _replyHeaderLayout({
  required BuildContext context,
  required Widget avatar,
  required Widget userInfo,
  VoidCallback? onTap,
  String? cardImage,
  String? cardNumber,
  Color? cardFanColor,
}) {
  final header = GestureDetector(
    onTap: onTap,
    child: TranslucentRow(
      spacing: 12,
      extraWidth: 46,
      children: [
        avatar,
        Flexible(child: userInfo),
      ],
    ),
  );

  if (cardImage == null) return header;

  const double cardHeight = 38;

  return Stack(
    clipBehavior: Clip.none,
    children: [
      Positioned(
        top: 0,
        right: 0,
        height: cardHeight,
        child: CachedNetworkImage(
          height: cardHeight,
          memCacheHeight: cardHeight.cacheSize(context),
          imageUrl: ImageUtils.safeThumbnailUrl(cardImage),
          placeholder: (_, _) => const SizedBox.shrink(),
        ),
      ),
      if (cardNumber != null)
        Positioned(
          top: 0,
          right: 0,
          height: cardHeight,
          child: Center(
            child: Text(
              cardNumber,
              style: TextStyle(
                fontSize: 8,
                fontFamily: Assets.digitalNum,
                color: cardFanColor,
              ),
            ),
          ),
        ),
      Padding(padding: const .only(right: 80), child: header),
    ],
  );
}

Widget _replyUserInfoLayout({
  required Widget nameRow,
  required Widget timeLine,
}) {
  return Column(
    mainAxisSize: MainAxisSize.min,
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [nameRow, timeLine],
  );
}

Widget _replyUserNameRow({
  required Widget name,
  Widget? level,
  Widget? trailing,
}) {
  return Row(
    spacing: 6,
    mainAxisSize: .min,
    children: [
      Flexible(child: name),
      ?level,
      ?trailing,
    ],
  );
}

Widget _replyTextButton({
  required String label,
  required TextStyle textStyle,
  required VoidCallback? onPressed,
  Widget? icon,
}) {
  return SizedBox(
    height: 32,
    child: TextButton(
      style: _replyTextButtonStyle,
      onPressed: onPressed,
      child: icon == null
          ? Text(label, style: textStyle)
          : Row(
              spacing: 3,
              mainAxisSize: .min,
              children: [
                icon,
                Text(label, style: textStyle),
              ],
            ),
    ),
  );
}

Widget _replyTranslateButton({
  required String label,
  required Color color,
  required TextStyle textStyle,
  required VoidCallback onPressed,
}) {
  return SizedBox(
    height: 32,
    child: TextButton(
      style: _replyTextButtonStyle,
      onPressed: onPressed,
      child: Row(
        spacing: 3,
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.translate, size: 16, color: color),
          Text(label, style: textStyle.copyWith(color: color)),
        ],
      ),
    ),
  );
}

Widget _replyActionBarLayout({
  required List<Widget> leftButtons,
  Widget? rightButton,
  required bool noPad,
}) {
  return Row(
    spacing: 2,
    children: [
      if (!noPad) const SizedBox(width: 36),
      ...leftButtons,
      if (rightButton != null) ...[
        const Spacer(),
        rightButton,
      ],
      const SizedBox(width: 5),
    ],
  );
}

Widget _replySubReplyListLayout({
  required BuildContext context,
  required List<Widget> items,
}) {
  final colorScheme = ColorScheme.of(context);
  return Padding(
    padding: const EdgeInsets.only(left: 42, right: 4),
    child: Material(
      animationDuration: Duration.zero,
      color: colorScheme.onInverseSurface,
      borderRadius: const BorderRadius.all(Radius.circular(6)),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: items,
      ),
    ),
  );
}

(EdgeInsets, BorderRadius?) _subReplyDecoration(int index, int length) {
  if (length == 1) {
    return const (.fromLTRB(8, 5, 8, 5), .all(.circular(6)));
  }
  if (index == 0) {
    return const (.fromLTRB(8, 8, 8, 4), .vertical(top: .circular(6)));
  }
  if (index == length - 1) {
    return const (.fromLTRB(8, 4, 8, 8), .vertical(bottom: .circular(6)));
  }
  return const (.fromLTRB(8, 4, 8, 4), null);
}

Widget _morePanelLayout({
  required BuildContext context,
  required List<Widget> items,
}) {
  final colorScheme = ColorScheme.of(context);
  return Padding(
    padding: EdgeInsets.only(
      bottom: MediaQuery.viewPaddingOf(context).bottom + 20,
    ),
    child: Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        InkWell(
          onTap: Get.back,
          borderRadius: Style.bottomSheetRadius,
          child: SizedBox(
            height: 35,
            child: Center(
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: colorScheme.outline,
                  borderRadius: const .all(.circular(3)),
                ),
                child: const SizedBox(width: 32, height: 3),
              ),
            ),
          ),
        ),
        ...items,
      ],
    ),
  );
}

// TODO: refa
class ReplyItemGrpc extends StatelessWidget {
  const ReplyItemGrpc({
    super.key,
    required this.replyItem,
    required this.replyLevel,
    this.replyReply,
    this.needDivider = true,
    this.onReply,
    this.onDelete,
    this.upMid,
    this.dialogueCallback,
    this.getReplyChain,
    this.getTag,
    this.onViewImage,
    this.onCheckReply,
    this.onToggleTop,

    this.depth = -1,
    this.reason,
    this.collapsed = 0,
    this.toggleCollapse,
    this.onScrollLeft,
  });
  final ReplyEntity replyItem;
  final int replyLevel;
  final void Function(ReplyEntity replyItem, int? rpid)? replyReply;
  final bool needDivider;
  final ValueChanged<ReplyEntity>? onReply;
  final void Function(ReplyEntity replyItem, int? subIndex)? onDelete;
  final int? upMid;
  final ValueChanged<ReplyEntity>? dialogueCallback;
  final ValueGetter<Iterable<ReplyEntity>>? getReplyChain;
  final ValueGetter<String>? getTag;
  final VoidCallback? onViewImage;
  final ValueChanged<ReplyEntity>? onCheckReply;
  final ValueChanged<ReplyEntity>? onToggleTop;

  final int depth;
  final ReasonType? reason;
  final int collapsed;
  final VoidCallback? toggleCollapse;
  final VoidCallback? onScrollLeft;

  static int? replyLengthLimit = Pref.replyLengthLimit;

  void _showMore(BuildContext context) => showModalBottomSheet(
    context: context,
    useSafeArea: true,
    isScrollControlled: true,
    constraints: BoxConstraints(
      maxWidth: min(640, context.mediaQueryShortestSide),
    ),
    builder: (context) => _morePanel(
      context: context,
      item: replyItem,
      onDelete: onDelete == null ? null : () => onDelete!(replyItem, null),
      isSubReply: false,
    ),
  );

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);

    return reason != null
        ? _buildPlaceholder(context, colorScheme)
        : _buildCommon(context, colorScheme);
  }

  Widget _buildPlaceholder(BuildContext context, ColorScheme colorScheme) {
    return _replyItemLayout(
      context: context,
      depth: depth,
      needDivider: needDivider,
      onTap: () {}, // 使InkWell能点着玩
      onTapLine: toggleCollapse,
      onScrollLeft: onScrollLeft,
      content: _replyContentLayout(
        noPad: true,
        header: _buildHeader(context, colorScheme),
        message: TextMore(
          reason!.desc,
          primary: colorScheme.primary,
          style: TextStyle(
            height: 1.75,
            fontSize: 14,
            color: colorScheme.error,
          ),
        ),
        actions: _replyActionBarLayout(
          noPad: true,
          leftButtons: [
            _replyTextButton(
              label: collapsed == 0 ? '折叠' : '展开 $collapsed 条评论',
              textStyle: _getActionStyle(colorScheme),
              onPressed: toggleCollapse,
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildCommon(BuildContext context, ColorScheme colorScheme) {
    final replyControl = replyItem.control;
    final noPad = replyLevel == 0 || depth >= 0;
    return _replyItemLayout(
      context: context,
      depth: depth,
      needDivider: needDivider,
      onTap: replyReply == null ? null : () => replyReply!(replyItem, null),
      onLongPress: replyLevel == 0 ? null : () => _showMore(context),
      onTapLine: toggleCollapse,
      onScrollLeft: onScrollLeft,
      content: _replyContentLayout(
        noPad: noPad,
        header: _buildHeader(context, colorScheme),
        voteOption: replyControl.voteOption == null
            ? null
            : _buildVoteOption(colorScheme, replyControl.voteOption!),
        message: _buildMessageWidget(context, colorScheme, replyControl),
        pictures: _buildPictures(),
        actions: replyLevel == 0
            ? null
            : _buttonAction(context, colorScheme, replyControl, noPad),
        subReplies: replyLevel == 1 && replyItem.count > 0
            ? _replyItemRow(context, colorScheme, replyItem.replies)
            : null,
      ),
    );
  }

  Widget _buildHeader(BuildContext context, ColorScheme colorScheme) {
    final member = replyItem.member;
    // `card` 只在服务端给了卡图时才有值（判定在 Rust），所以这里不必再
    // 问一次「有没有卡图」。
    final card = PendantAvatar.showDecorate ? replyItem.card : null;
    final cardNumber = card?.number;

    return _replyHeaderLayout(
      context: context,
      onTap: replyItem.mid == 0
          ? null
          : () {
              feedBack();
              Get.toNamed('/member?mid=${replyItem.mid}');
            },
      avatar: PendantAvatar(
        member.face,
        size: 34,
        badgeSize: 14,
        vipStatus: member.vipStatus,
        officialType: member.officialVerifyType,
        pendantImage: member.pendantImage,
      ),
      userInfo: _buildUserInfo(colorScheme),
      cardImage: card?.image,
      cardNumber: cardNumber == null
          ? null
          : '${card!.fanNumPrefix}\n$cardNumber',
      cardFanColor: cardNumber == null
          ? null
          : ColourUtils.parseColor(card!.fanColor),
    );
  }

  Widget _buildUserInfo(ColorScheme colorScheme) {
    final member = replyItem.member;
    final isVip = member.vipStatus > 0 && member.vipType == 2;

    Widget? trailing;
    // 基线判的是 `hasMid() && upMid == mid`，即「字段存在」而不是「非零」。
    // 实体把 mid 摊平成 int 之后这个区别没了，改判非零：mid 为 0 的那条
    // 评论没有作者可言，给它挂 UP 角标是基线的一个边角 bug，不跟。
    if (replyItem.mid != 0 && upMid == replyItem.mid) {
      trailing = const PBadge(
        text: 'UP',
        size: .small,
        isStack: false,
        fontSize: 9,
      );
    } else if (member.medal case final medal? when GlobalData().showMedal) {
      trailing = MedalWidget(
        medalName: medal.name,
        level: medal.level,
        backgroundColor: DmUtils.decimalToColor(medal.color),
        nameColor: DmUtils.decimalToColor(medal.colorName),
        padding: const .symmetric(horizontal: 6, vertical: 1.5),
      );
    }

    return _replyUserInfoLayout(
      nameRow: _replyUserNameRow(
        name: Text(
          member.name,
          maxLines: 1,
          overflow: .ellipsis,
          style: TextStyle(
            color: isVip ? colorScheme.vipColor : colorScheme.outline,
            fontSize: 13,
          ),
        ),
        level: BiliUtils.levelPicture(
          member.level,
          isSeniorMember: member.isSenior,
          height: 11,
        ),
        trailing: trailing,
      ),
      timeLine: Text.rich(
        maxLines: 1,
        overflow: .ellipsis,
        style: TextStyle(fontSize: 11, color: colorScheme.outline),
        TextSpan(
          text: replyLevel == 0
              ? DateFormatUtils.format(
                  replyItem.ctime,
                  format: DateFormatUtils.longFormatDs,
                )
              : DateFormatUtils.dateFormat(replyItem.ctime),
          children: switch (replyItem.control.location) {
            final location? => [TextSpan(text: ' • $location')],
            null => null,
          },
        ),
      ),
    );
  }

  Widget _buildVoteOption(
    ColorScheme colorScheme,
    ReplyVoteOption voteOption,
  ) {
    return Text.rich(
      TextSpan(
        children: [
          switch (voteOption.labelKind) {
            1 => TextSpan(
              text: '红方  ',
              style: TextStyle(color: colorScheme.vipColor),
            ),
            2 => TextSpan(
              text: '蓝方  ',
              style: TextStyle(color: colorScheme.blue),
            ),
            _ => TextSpan(
              text: '投票  ',
              style: TextStyle(color: colorScheme.outline),
            ),
          },
          TextSpan(text: voteOption.desc),
        ],
      ),
      style: TextStyle(
        height: 1.75,
        fontSize: 12,
        color: colorScheme.onSurfaceVariant,
      ),
    );
  }

  Widget _buildMessageWidget(
    BuildContext context,
    ColorScheme colorScheme,
    ReplyControlInfo replyControl,
  ) {
    return TextMore.rich(
      primary: colorScheme.primary,
      style: const TextStyle(height: 1.75, fontSize: 14),
      maxLines: replyLevel == 1 ? replyLengthLimit : null,
      TextSpan(
        children: [
          if (replyControl.isUpTop) ...[
            const WidgetSpan(
              alignment: .middle,
              child: PBadge(
                text: 'TOP',
                size: .small,
                isStack: false,
                type: .line_primary,
                fontSize: 9,
                textScaleFactor: 1,
              ),
            ),
            const TextSpan(text: ' '),
          ],
          _buildMessage(
            context,
            colorScheme,
            replyControl.showTranslation
                ? replyItem.translatedContent ?? replyItem.content
                : replyItem.content,
            replyControl,
          ),
        ],
      ),
    );
  }

  Widget? _buildPictures() {
    final pictures = replyItem.content.pictures;
    if (pictures.isEmpty) return null;
    return ImageGridView(
      picArr: pictures
          .map(
            (item) => ImageModel(
              width: item.imgWidth,
              height: item.imgHeight,
              url: item.imgSrc,
            ),
          )
          .toList(),
      onViewImage: onViewImage,
    );
  }

  Widget _buildTranslateBtn(
    BuildContext context,
    ColorScheme colorScheme,
    ReplyControlInfo replyControl,
    TextStyle textStyle,
  ) {
    late bool isProcessing = false;
    final color = replyControl.showTranslation
        ? colorScheme.primary
        : colorScheme.outline.withValues(alpha: 0.8);
    return _replyTranslateButton(
      label: replyControl.showTranslation ? '原文' : '翻译',
      color: color,
      textStyle: textStyle,
      onPressed: () async {
        if (replyControl.showTranslation) {
          replyControl.showTranslation = false;
          (context as Element).markNeedsBuild();
        } else {
          if (isProcessing) return;
          if (replyItem.translatedContent != null) {
            replyControl.showTranslation = true;
            (context as Element).markNeedsBuild();
            return;
          }
          isProcessing = true;
          final res = await RustReplyGrpc.translateReply(
            type: replyItem.kind,
            oid: replyItem.oid,
            rpid: replyItem.id,
          );
          if (res case Success(:final response)) {
            if (response != null) {
              replyControl.showTranslation = true;
              replyItem.translatedContent = response;
              if (context.mounted) {
                (context as Element).markNeedsBuild();
              }
            } else {
              SmartDialog.showToast('翻译结果为空');
            }
          } else if (res case Error(:final errMsg)) {
            SmartDialog.showToast('翻译失败: $errMsg');
          }
          isProcessing = false;
        }
      },
    );
  }

  TextStyle _getActionStyle(ColorScheme colorScheme) => TextStyle(
    height: 1,
    fontSize: 12,
    fontWeight: .normal,
    color: colorScheme.outline,
  );

  Widget _buttonAction(
    BuildContext context,
    ColorScheme colorScheme,
    ReplyControlInfo replyControl,
    bool noPad,
  ) {
    final textStyle = _getActionStyle(colorScheme);

    Widget? dialogBtn;
    if (replyLevel == 2 && needDivider && replyItem.id != replyItem.dialog) {
      dialogBtn = _replyTextButton(
        label: '查看对话',
        textStyle: textStyle,
        onPressed: () => dialogueCallback?.call(replyItem),
      );
    } else if (replyLevel == 3 && replyItem.parent != replyItem.root) {
      dialogBtn = _replyTextButton(
        label: '跳转回复',
        textStyle: textStyle,
        onPressed: () => dialogueCallback?.call(replyItem),
      );
    } else if (toggleCollapse != null) {
      dialogBtn = _replyTextButton(
        label: collapsed == 0 ? '折叠' : '展开 $collapsed 条评论',
        textStyle: textStyle,
        onPressed: toggleCollapse,
      );
    }

    return _replyActionBarLayout(
      noPad: noPad,
      leftButtons: [
        if (onReply != null)
          _replyTextButton(
            label: '回复',
            textStyle: textStyle,
            icon: Icon(
              Icons.reply,
              size: 18,
              color: colorScheme.outline.withValues(alpha: 0.8),
            ),
            onPressed: () {
              feedBack();
              onReply!(replyItem);
            },
          ),
        if (replyControl.canTranslate)
          _buildTranslateBtn(
            context,
            colorScheme,
            replyControl,
            textStyle,
          )
        else if (replyControl.cardLabels.isNotEmpty)
          Text(
            dialogBtn != null
                ? replyControl.cardLabels.first
                : replyControl.cardLabels.join('  '),
            style: textStyle.copyWith(color: colorScheme.secondary),
          ),
        ?dialogBtn,
      ],
      rightButton: ZanButtonGrpc(replyItem: replyItem),
    );
  }

  Widget _replyItemRow(
    BuildContext context,
    ColorScheme colorScheme,
    List<ReplyEntity> replies,
  ) {
    final extraRow = replies.length < replyItem.count;
    final length = replies.length + (extraRow ? 1 : 0);

    return _replySubReplyListLayout(
      context: context,
      items: [
        for (var i = 0; i < replies.length; i++)
          _buildSubReplyItem(
            context: context,
            colorScheme: colorScheme,
            childReply: replies[i],
            index: i,
            length: length,
          ),
        if (extraRow)
          _buildMoreRepliesRow(colorScheme: colorScheme, length: length),
      ],
    );
  }

  Widget _buildSubReplyItem({
    required BuildContext context,
    required ColorScheme colorScheme,
    required ReplyEntity childReply,
    required int index,
    required int length,
  }) {
    final (padding, borderRadius) = _subReplyDecoration(index, length);

    void showMore() => showModalBottomSheet(
      context: context,
      useSafeArea: true,
      isScrollControlled: true,
      constraints: BoxConstraints(
        maxWidth: min(640, context.mediaQueryShortestSide),
      ),
      builder: (context) => _morePanel(
        context: context,
        item: childReply,
        onDelete: onDelete == null ? null : () => onDelete!(replyItem, index),
        isSubReply: true,
      ),
    );

    return InkWell(
      borderRadius: borderRadius,
      onTap: () => replyReply?.call(replyItem, childReply.id),
      onLongPress: showMore,
      onSecondaryTap: PlatformUtils.isMobile ? null : showMore,
      child: Padding(
        padding: padding,
        child: TextEllipsis.rich(
          style: TextStyle(
            height: 1.6,
            fontSize: 14,
            color: colorScheme.onSurface.withValues(alpha: 0.85),
          ),
          overflow: TextOverflow.ellipsis,
          maxLines: 2,
          TextSpan(
            children: [
              TextSpan(
                text: childReply.member.name,
                style: TextStyle(color: colorScheme.primary),
                recognizer: NoDeadlineTapGestureRecognizer()
                  ..onTap = () {
                    feedBack();
                    Get.toNamed(
                      '/member?mid=${childReply.member.mid}',
                    );
                  },
              ),
              if (childReply.mid == upMid) ...[
                const TextSpan(text: ' '),
                const WidgetSpan(
                  alignment: .middle,
                  child: PBadge(
                    text: 'UP',
                    size: .small,
                    isStack: false,
                    fontSize: 9,
                    textScaleFactor: 1,
                  ),
                ),
                const TextSpan(text: ' '),
              ],
              if (childReply.root == childReply.parent)
                const TextSpan(text: ': ')
              else if (childReply.mid != upMid)
                const TextSpan(text: ' '),
              _buildMessage(
                context,
                colorScheme,
                childReply.content,
                childReply.control,
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildMoreRepliesRow({
    required ColorScheme colorScheme,
    required int length,
  }) {
    final isSingle = length == 1;
    return InkWell(
      onTap: () => replyReply?.call(replyItem, null),
      borderRadius: isSingle
          ? const .all(.circular(6))
          : const .vertical(bottom: .circular(6)),
      child: Padding(
        padding: isSingle
            ? const .fromLTRB(8, 6, 8, 6)
            : const .fromLTRB(8, 5, 8, 8),
        child: Text.rich(
          TextSpan(
            style: const TextStyle(fontSize: 12),
            children: [
              if (replyItem.control.upReply)
                TextSpan(
                  text: 'UP主等人 ',
                  style: TextStyle(
                    color: colorScheme.onSurface.withValues(alpha: 0.85),
                  ),
                ),
              TextSpan(
                text: '共${replyItem.count}条回复',
                style: TextStyle(color: colorScheme.primary),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// 当前稿件时长（毫秒），决定一个时间戳可不可点。
  ///
  /// 取不到播放器时给 null：基线在这里是 `Get.find` 抛异常后 `isValid`
  /// 保持 false，两边结果一样，只是一边靠异常一边靠空值。
  ///
  /// **先问 `isRegistered` 再 `find`**，不是洁癖：基线只在正文里真的出现
  /// 时间戳时才走这一步，而分词的入参每次 build 都要它。评论区也挂在动态
  /// 详情这类没有 [VideoDetailController] 的页面上，在那里每条评论的每次
  /// build 都抛一次异常，代价比查一次注册表大得多。
  int? _videoDurationMs() {
    try {
      final tag = getTag?.call() ?? Get.arguments?['heroTag'];
      if (tag is! String) return null;
      if (!Get.isRegistered<VideoDetailController>(tag: tag)) return null;
      return Get.find<VideoDetailController>(tag: tag).data.timeLength;
    } catch (e) {
      if (kDebugMode) debugPrint('failed to resolve duration: $e');
      return null;
    }
  }

  /// 正文。
  ///
  /// 「哪一段是什么、点下去跳哪」在 `bili_domain::reply_richtext`
  /// （#179）。这里只把段落翻成 `InlineSpan`——图片、配色、手势识别器与
  /// 具体路由，换一个宿主框架时本来就要重写的那些。
  InlineSpan _buildMessage(
    BuildContext context,
    ColorScheme colorScheme,
    ReplyContent content,
    ReplyControlInfo replyControl,
  ) {
    return TextSpan(
      children: [
        for (final segment in RustReplyGrpc.segments(
          content: content,
          control: replyControl,
          videoDurationMs: _videoDurationMs(),
        ))
          _buildSegment(context, colorScheme, segment),
      ],
    );
  }

  InlineSpan _buildSegment(
    BuildContext context,
    ColorScheme colorScheme,
    ReplySegment segment,
  ) {
    switch (segment.kind) {
      case ReplySegmentKind.text:
        return TextSpan(text: segment.text);

      case ReplySegmentKind.emote:
        final emote = segment.emote!;
        final size = emote.size * 20.0;
        return WidgetSpan(
          child: emoteTooltipBuilder(
            url: emote.displayUrl,
            emote: segment.text,
            triggerMode: .tap,
            jumpUrl: emote.jumpUrl,
            colorScheme: colorScheme,
            child: NetworkImgLayer(
              src: emote.displayUrl,
              type: .emote,
              width: size,
              height: size,
            ),
          ),
        );

      case ReplySegmentKind.mention:
        return TextSpan(
          text: segment.text,
          style: TextStyle(color: colorScheme.primary),
          recognizer: NoDeadlineTapGestureRecognizer()
            ..onTap = () => Get.toNamed('/member?mid=${segment.mid}'),
        );

      case ReplySegmentKind.vote:
        final vote = segment.vote!;
        return TextSpan(
          text: '投票: ${vote.title}',
          style: TextStyle(color: colorScheme.primary),
          recognizer: NoDeadlineTapGestureRecognizer()
            ..onTap = () => showVoteDialog(context, vote.id),
        );

      case ReplySegmentKind.timestamp:
        final seconds = segment.seekSeconds;
        if (seconds == null) return TextSpan(text: segment.text);
        // 可点的那些两侧补一个空格，让它在正文里像个按钮。排版是宿主的
        // 事，Rust 只负责说它可不可点。
        return TextSpan(
          text: ' ${segment.text} ',
          style: TextStyle(color: colorScheme.primary),
          recognizer: NoDeadlineTapGestureRecognizer()
            ..onTap = () => _seekTo(segment.text, seconds),
        );

      case ReplySegmentKind.topic:
      case ReplySegmentKind.note:
        return TextSpan(
          text: segment.text,
          style: TextStyle(
            // 两个跳转目标都没有的「[笔记]」只能画成不可点的样子。
            color: segment.action == ReplyTapAction.none
                ? colorScheme.secondary
                : colorScheme.primary,
          ),
          recognizer: _recognizer(segment),
        );

      case ReplySegmentKind.link:
        final prefixIcon = segment.prefixIcon;
        return TextSpan(
          children: [
            if (prefixIcon != null)
              WidgetSpan(
                child: CachedNetworkImage(
                  height: 19,
                  memCacheHeight: 19.cacheSize(context),
                  color: colorScheme.primary,
                  imageUrl: ImageUtils.thumbnailUrl(prefixIcon),
                  placeholder: (_, _) => const SizedBox.shrink(),
                ),
              ),
            TextSpan(
              text: segment.text,
              style: TextStyle(color: colorScheme.primary),
              recognizer: _recognizer(segment),
            ),
          ],
        );
    }
  }

  NoDeadlineTapGestureRecognizer? _recognizer(ReplySegment segment) {
    final onTap = _onTap(segment);
    if (onTap == null) return null;
    return NoDeadlineTapGestureRecognizer()..onTap = onTap;
  }

  /// 段落的点击行为。跳哪由 Rust 定（[ReplySegment.action]），这里只把它
  /// 兑换成本宿主的路由调用。
  VoidCallback? _onTap(ReplySegment segment) => switch (segment.action) {
    ReplyTapAction.none => null,
    ReplyTapAction.webview => () => PageUtils.handleWebview(segment.target),
    ReplyTapAction.archive => () => UrlUtils.matchUrlPush(segment.target, ''),
    ReplyTapAction.article => () =>
      ArticlePage.toArticlePage(id: segment.target, type: .read),
    ReplyTapAction.opus => () =>
      ArticlePage.toArticlePage(id: segment.target, type: .opus),
    ReplyTapAction.scheme => () => PiliScheme.routePushFromUrl(segment.target),
    ReplyTapAction.search => () =>
      Get.toNamed('/searchResult', parameters: {'keyword': segment.target}),
  };

  void _seekTo(String label, int seconds) {
    try {
      SmartDialog.showToast('跳转至：$label');
      Get.find<VideoDetailController>(
        tag: Get.arguments['heroTag'],
      ).plPlayerController.seekTo(Duration(seconds: seconds));
    } catch (e) {
      SmartDialog.showToast('跳转失败: $e');
    }
  }

  Widget _morePanel({
    required BuildContext context,
    required ReplyEntity item,
    required VoidCallback? onDelete,
    required bool isSubReply,
  }) {
    late final message = item.content.message;
    final ownerMid = Accounts.main.mid;
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    final errorColor = colorScheme.error;
    final style = theme.textTheme.titleSmall!;

    return _morePanelLayout(
      context: context,
      items: [
        if (kDebugMode && GStorage.reply != null) ...[
          ListTile(
            onTap: () {
              Get.back();
              if (RustReplyGrpc.encodeStored(item) case final data?) {
                GStorage.reply!.put(item.id.toString(), data);
              }
            },
            title: Text(
              'save to local',
              style: style.copyWith(color: colorScheme.primary),
            ),
          ),
          ListTile(
            onTap: () {
              Get.back();
              onDelete?.call();
              GStorage.reply!.delete(item.id.toString());
            },
            title: Text(
              'remove from local',
              style: style.copyWith(color: colorScheme.primary),
            ),
          ),
          ListTile(
            onTap: () {
              Get.back();
              final data = RustReplyGrpc.encodeStored(item);
              if (data == null) return;
              final oid = item.oid;
              GStorage.reply!.putAll({
                for (var i = oid; i < oid + 1000; i++) i.toString(): data,
              });
            },
            title: Text(
              'save to local (x1000)',
              style: style.copyWith(color: colorScheme.primary),
            ),
          ),
        ],
        if (ownerMid == upMid || ownerMid == item.member.mid)
          ListTile(
            onTap: () async {
              Get.back();
              bool? isDelete = await showDialog<bool>(
                context: context,
                builder: (context) {
                  final colorScheme = ColorScheme.of(context);
                  return AlertDialog(
                    title: const Text('删除评论'),
                    content: Text.rich(
                      TextSpan(
                        children: [
                          const TextSpan(text: '确定删除这条评论吗？\n\n'),
                          if (ownerMid != item.member.mid) ...[
                            TextSpan(
                              text: '@${item.member.name}',
                              style: TextStyle(color: colorScheme.primary),
                            ),
                            const TextSpan(text: ':\n'),
                          ],
                          TextSpan(text: message),
                        ],
                      ),
                    ),
                    actions: [
                      TextButton(
                        onPressed: () => Get.back(result: false),
                        child: Text(
                          '取消',
                          style: TextStyle(color: colorScheme.outline),
                        ),
                      ),
                      TextButton(
                        onPressed: () => Get.back(result: true),
                        child: const Text('确定'),
                      ),
                    ],
                  );
                },
              );
              if (isDelete == null || !isDelete) {
                return;
              }
              SmartDialog.showLoading(msg: '删除中...');
              final res = await RustVideoWrite.replyDel(
                type: item.kind,
                oid: item.oid,
                rpid: item.id,
              );
              SmartDialog.dismiss();
              if (res.isSuccess) {
                SmartDialog.showToast('删除成功');
                onDelete?.call();
              } else {
                SmartDialog.showToast('删除失败, $res');
              }
            },
            minLeadingWidth: 0,
            leading: Icon(Icons.delete_outlined, color: errorColor, size: 19),
            title: Text('删除', style: style.copyWith(color: errorColor)),
          ),
        if (ownerMid == 0)
          ListTile(
            onTap: () {
              Get.back();
              Pref.setBlackMid(item.mid);
              onDelete?.call();
              SmartDialog.showToast('屏蔽成功');
              return;
            },
            minLeadingWidth: 0,
            leading: Icon(Icons.error_outline, color: errorColor, size: 19),
            title: Text(
              '屏蔽',
              style: TextStyle(fontSize: 14, color: errorColor),
            ),
          )
        else
          ListTile(
            onTap: () {
              Get.back();

              final oid = item.oid;
              final rpid = item.id;

              autoWrapReportDialog(
                context,
                onRemove: onDelete,
                mid: () => item.mid,
                ReportOptions.commentReport,
                withContent: ReportOptions.withContentReply,
                contentRequired: ReportOptions.contentRequiredReply,
                reportUrl:
                    'https://www.bilibili.com/h5/comment/report?oid=$oid&pageType=${item.kind}&rpid=$rpid&platform=android&build=8430300&${ThemeUtils.themeUrl(colorScheme.isDark)}',
                (reasonType, reasonDesc, banUid) async {
                  final res = await ReplyHttp.report(
                    rpid: rpid,
                    oid: oid,
                    reasonType: reasonType,
                    reasonDesc: reasonDesc,
                    banUid: banUid,
                  );
                  if (res.isSuccess) {
                    onDelete?.call();
                  }
                  return res;
                },
              );
            },
            minLeadingWidth: 0,
            leading: Icon(Icons.error_outline, color: errorColor, size: 19),
            title: Text('举报', style: style.copyWith(color: errorColor)),
          ),
        if (replyLevel == 1 && !isSubReply && ownerMid == upMid)
          ListTile(
            onTap: () {
              Get.back();
              onToggleTop?.call(item);
            },
            minLeadingWidth: 0,
            leading: const Icon(Icons.vertical_align_top, size: 19),
            title: Text(
              '${replyItem.control.isUpTop ? '取消' : ''}置顶',
              style: style,
            ),
          ),
        ListTile(
          onTap: () {
            Get.back();
            Utils.copyText(message);
          },
          minLeadingWidth: 0,
          leading: const Icon(Icons.copy_all_outlined, size: 19),
          title: Text('复制全部', style: style),
        ),
        ListTile(
          onTap: () {
            Get.back();
            showReplyCopyDialog(context, message, item.content.emotes);
          },
          minLeadingWidth: 0,
          leading: const Icon(Icons.copy_outlined, size: 19),
          title: Text('自由复制', style: style),
        ),
        ListTile(
          onTap: () {
            Get.back();
            SavePanel.toSavePanel(upMid: upMid, item: item);
          },
          minLeadingWidth: 0,
          leading: const Icon(Icons.save_alt, size: 19),
          title: Text('保存评论', style: style),
        ),
        if (getReplyChain != null && item.id != item.root)
          ListTile(
            onTap: () {
              Get.back();
              SavePanel.toSavePanel(upMid: upMid, item: getReplyChain!());
            },
            minLeadingWidth: 0,
            leading: const Icon(Icons.save_alt, size: 19),
            title: Text('保存评论链', style: style),
          ),
        if (kDebugMode || item.mid == ownerMid)
          ListTile(
            onTap: () {
              Get.back();
              onCheckReply?.call(item);
            },
            minLeadingWidth: 0,
            leading: const Icon(CustomIcons.shield_reply, size: 19),
            title: Text('检查评论', style: style),
          ),
      ],
    );
  }
}
