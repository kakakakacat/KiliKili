import 'dart:async';

import 'package:kilikili/common/assets.dart';
import 'package:kilikili/common/widgets/flutter/list_tile.dart';
import 'package:kilikili/common/widgets/flutter/refresh_indicator.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/mine_tab_type.dart';
import 'package:kilikili/models/common/nav_bar_config.dart';
import 'package:kilikili/models/common/theme/theme_type.dart';
import 'package:kilikili/pages/common/common_page.dart';
import 'package:kilikili/pages/home/view.dart';
import 'package:kilikili/pages/login/controller.dart';
import 'package:kilikili/pages/main/controller.dart';
import 'package:kilikili/pages/mine/controller.dart';
import 'package:kilikili/pages/mine/widgets/fav_item.dart';
import 'package:kilikili/pages/mine/widgets/history_item.dart';
import 'package:kilikili/pages/mine/widgets/later_item.dart';
import 'package:kilikili/pages/mine/widgets/mine_surfaces.dart';
import 'package:kilikili/pages/mine/widgets/sub_item.dart';
import 'package:kilikili/utils/bili_utils.dart';
import 'package:kilikili/utils/extension/get_ext.dart';
import 'package:kilikili/utils/extension/theme_ext.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get/get.dart';
import 'package:material_design_icons_flutter/material_design_icons_flutter.dart';
import 'package:material_ui/material_ui.dart' hide ListTile;

class MinePage extends StatefulWidget {
  const MinePage({super.key, this.showBackBtn = false});

  final bool showBackBtn;

  @override
  State<MinePage> createState() => _MediaPageState();
}

class _MediaPageState extends CommonPageState<MinePage>
    with AutomaticKeepAliveClientMixin {
  final MineController controller = Get.putOrFind(MineController.new);
  late final MainController _mainController = Get.find<MainController>();
  late ThemeData theme;

  @override
  bool get wantKeepAlive => true;

  bool get checkPage =>
      _mainController.navigationBars[0] != NavigationBarType.mine &&
      _mainController.selectedIndex.value == 0;

  @override
  bool onNotificationType1(UserScrollNotification notification) {
    if (checkPage) {
      return false;
    }
    return super.onNotificationType1(notification);
  }

  @override
  bool onNotificationType2(ScrollNotification notification) {
    if (checkPage) {
      return false;
    }
    return super.onNotificationType2(notification);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    theme = Theme.of(context);
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return MinePageBackground(
      child: Column(
        children: [
          Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 960),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 14, 20, 20),
                child: _buildHeaderActions,
              ),
            ),
          ),
          Expanded(
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 960),
                child: Material(
                  type: MaterialType.transparency,
                  child: refreshIndicator(
                    onRefresh: controller.onRefresh,
                    child: onBuild(
                      ListView(
                        padding: const EdgeInsets.fromLTRB(20, 0, 20, 150),
                        physics: const AlwaysScrollableScrollPhysics(),
                        children: [
                          _buildUserInfo(),
                          const SizedBox(height: 18),
                          MineActionDock(actions: controller.list),
                          const SizedBox(height: 24),
                          for (final tab in controller.mineTabs)
                            switch (tab) {
                              .favorite => Obx(_buildFav),
                              .history => Obx(_buildHistory),
                              .later => Obx(_buildLater),
                              .sub => Obx(_buildSub),
                            },
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget get _buildHeaderActions => Wrap(
    alignment: WrapAlignment.spaceBetween,
    crossAxisAlignment: WrapCrossAlignment.center,
    spacing: 12,
    runSpacing: 12,
    children: [
      Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (widget.showBackBtn) const BackButton(),
          Text(
            '我的',
            style: theme.textTheme.headlineSmall?.copyWith(
              fontWeight: FontWeight.w600,
            ),
          ),
        ],
      ),
      MineToolCapsule(
        children: [
          if (!_mainController.hasHome) ...[
            MineToolButton(
              tooltip: '搜索',
              onPressed: () => Get.toNamed('/search'),
              icon: Icons.search,
            ),
            msgBadge(_mainController),
          ],
          if (GStorage.reply != null)
            MineToolButton(
              tooltip: '评论记录',
              onPressed: () => Get.toNamed('/myReply'),
              icon: Icons.comment_outlined,
            ),
          Obx(() {
            final anonymity = MineController.anonymity.value;
            return MineToolButton(
              tooltip: '无痕模式：${anonymity ? '已开启，点击关闭' : '已关闭，点击开启'}',
              selected: anonymity,
              onPressed: MineController.onChangeAnonymity,
              // 固定图形、用底色区分状态，避免斜杠把眼镜和帽子的轮廓盖住。
              icon: MdiIcons.incognito,
            );
          }),
          MineToolButton(
            tooltip: '切换账号',
            onPressed: () => LoginPageController.switchAccountDialog(context),
            icon: MdiIcons.accountSwitchOutline,
          ),
          Obx(
            () => PopupMenuButton<ThemeType>(
              tooltip: '主题：${controller.themeType.value.label}',
              initialValue: controller.themeType.value,
              onSelected: controller.onChangeTheme,
              itemBuilder: (context) => [
                for (final type in ThemeType.values)
                  CheckedPopupMenuItem(
                    value: type,
                    checked: type == controller.themeType.value,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        type.icon,
                        const SizedBox(width: 12),
                        Text(type.label),
                      ],
                    ),
                  ),
              ],
              child: SizedBox.square(
                dimension: 44,
                child: Center(
                  child: IconTheme(
                    data: IconThemeData(
                      size: 21,
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                    child: controller.themeType.value.icon,
                  ),
                ),
              ),
            ),
          ),
          MineToolButton(
            tooltip: '设置',
            onPressed: () => Get.toNamed('/setting', preventDuplicates: false),
            icon: Icons.settings_outlined,
          ),
        ],
      ),
    ],
  );

  Widget _buildUserInfo() => Obx(() {
    final userInfo = controller.userInfo.value;
    final levelInfo = userInfo.levelInfo;
    final isVip = userInfo.vipStatus != null && userInfo.vipStatus! > 0;
    final userStat = controller.userStat.value;
    final colors = theme.colorScheme;
    return MineAccountCard(
      avatar: Stack(
        clipBehavior: Clip.none,
        children: [
          Semantics(
            label: userInfo.face == null ? '默认头像' : '用户头像',
            child: NetworkImgLayer(
              src: userInfo.face,
              type: .avatar,
              width: 64,
              height: 64,
            ),
          ),
          if (isVip)
            Positioned(
              right: -1,
              bottom: -2,
              child: SvgPicture.asset(
                Assets.vipIcon,
                height: 20,
                semanticsLabel: '大会员',
              ),
            ),
        ],
      ),
      name: userInfo.uname ?? '点击登录',
      nameColor: isVip && userInfo.vipType == 2 ? colors.vipColor : null,
      level: BiliUtils.levelPicture(
        levelInfo?.currentLevel ?? 0,
        isSeniorMember: userInfo.isSeniorMember == 1,
        height: 12,
      ),
      details: [
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
          decoration: BoxDecoration(
            color: colors.primaryContainer.withValues(alpha: 0.45),
            borderRadius: BorderRadius.circular(20),
          ),
          child: Text(
            '硬币 ${userInfo.money?.toString() ?? '-'}',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: theme.textTheme.labelMedium?.copyWith(
              color: colors.onPrimaryContainer,
            ),
          ),
        ),
        Text(
          '经验 ${levelInfo?.currentExp ?? '-'}/${levelInfo?.nextExp ?? '-'}',
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: theme.textTheme.labelSmall?.copyWith(
            color: colors.onSurfaceVariant,
          ),
        ),
      ],
      statistics: [
        MineStatButton(
          count: userStat.dynamicCount?.toString() ?? '-',
          label: '动态',
          onTap: () => controller.push('memberDynamics'),
        ),
        MineStatButton(
          count: userStat.following?.toString() ?? '-',
          label: '关注',
          onTap: () => controller.push('follow'),
        ),
        MineStatButton(
          count: userStat.follower?.toString() ?? '-',
          label: '粉丝',
          onTap: () => controller.push('fan'),
        ),
      ],
      onTap: controller.onLogin,
      onLongPress: () {
        Feedback.forLongPress(context);
        controller.onLogin(true);
      },
      onSecondaryTap: PlatformUtils.isMobile
          ? null
          : () => controller.onLogin(true),
    );
  });

  void _autoRefresh(MineRefreshMixin tabCtr) => Timer(
    const Duration(milliseconds: 150),
    () => tabCtr.onRefresh(isManual: false),
  );

  Widget _buildFav() {
    return _buildTab(
      .favorite,
      controller.favCtr!,
      itemBuilder: (element) => FavFolderItem(
        item: element,
        onPop: () => _autoRefresh(controller.favCtr!),
      ),
      onPushDetail: () => Get.toNamed(
        '/fav',
      )?.whenComplete(() => _autoRefresh(controller.favCtr!)),
    );
  }

  Widget _buildHistory() {
    return _buildTab(
      .history,
      controller.historyCtr!,
      itemBuilder: (element) => MineHistoryItem(
        item: element,
        onPop: () => _autoRefresh(controller.historyCtr!),
      ),
      onPushDetail: () => Get.toNamed(
        '/history',
      )?.whenComplete(() => _autoRefresh(controller.historyCtr!)),
    );
  }

  Widget _buildLater() {
    return _buildTab(
      .later,
      controller.laterCtr!,
      itemBuilder: (element) => MineLaterItem(
        item: element,
        onPop: () => _autoRefresh(controller.laterCtr!),
      ),
      onPushDetail: () => Get.toNamed(
        '/later',
      )?.whenComplete(() => _autoRefresh(controller.laterCtr!)),
    );
  }

  Widget _buildSub() {
    return _buildTab(
      .sub,
      controller.subCtr!,
      itemBuilder: (element) => MineSubItem(
        item: element,
        onPop: () => _autoRefresh(controller.subCtr!),
      ),
      onPushDetail: () => Get.toNamed(
        '/subscription',
      )?.whenComplete(() => _autoRefresh(controller.subCtr!)),
    );
  }

  Widget _buildTab<R, T>(
    MineTabType tab,
    MineRefreshMixin<R, T> tabCtr, {
    required Widget Function(T element) itemBuilder,
    required VoidCallback onPushDetail,
  }) {
    final Widget child;
    switch (tabCtr.loadingState.value) {
      case Loading():
        return const SizedBox.shrink();
      case Success(:final response):
        if (response == null || response.isEmpty) {
          return const SizedBox.shrink();
        }
        child = SizedBox(
          height: tab == MineTabType.favorite
              ? 132 +
                    (MediaQuery.textScalerOf(context).scale(14) - 14).clamp(
                          0,
                          double.infinity,
                        ) *
                        8
              : 180,
          child: ListView.separated(
            itemCount: response.length,
            controller: tabCtr.scrollController,
            physics: const AlwaysScrollableScrollPhysics(),
            padding: const EdgeInsets.only(top: 10),
            itemBuilder: (context, index) {
              if (index == response.length - 1) {
                tabCtr.onLoadMore();
              }
              return SizedBox(
                width: tab == MineTabType.favorite
                    ? (MediaQuery.sizeOf(context).width - 40)
                          .clamp(0, 360)
                          .toDouble()
                    : 180,
                child: itemBuilder(response[index]),
              );
            },
            scrollDirection: .horizontal,
            separatorBuilder: (_, _) => const SizedBox(width: 14),
          ),
        );
      case Error(:final errMsg):
        if (errMsg == null) return const SizedBox.shrink();
        child = SizedBox(
          height: 160,
          child: Center(child: Text(errMsg, textAlign: .center)),
        );
    }
    final secondary = theme.colorScheme.secondary;
    return Column(
      children: [
        const SizedBox(height: 8),
        ListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          onTap: onPushDetail,
          title: Padding(
            padding: EdgeInsets.zero,
            child: Text.rich(
              TextSpan(
                children: [
                  TextSpan(
                    text: '${tab.label}  ',
                    style: const TextStyle(fontSize: 16, fontWeight: .bold),
                  ),
                  if (tabCtr.count != null)
                    TextSpan(
                      text: '${tabCtr.count}  ',
                      style: TextStyle(color: secondary),
                    ),
                  WidgetSpan(
                    child: Icon(
                      Icons.chevron_right,
                      size: 20,
                      color: secondary,
                    ),
                  ),
                ],
              ),
            ),
          ),
          trailing: IconButton(
            tooltip: '刷新',
            onPressed: tabCtr.onRefresh,
            icon: const Icon(Icons.refresh, size: 20),
          ),
        ),
        child,
        const SizedBox(height: 16),
      ],
    );
  }
}
