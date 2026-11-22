import 'package:kilikili/common/widgets/dialog/simple_dialog_option.dart';
import 'package:kilikili/common/widgets/loading_widget/loading_widget.dart';
import 'package:kilikili/common/widgets/scaffold/simple_scaffold.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/whisper_block/view.dart';
import 'package:kilikili/pages/whisper_settings/controller.dart';
import 'package:kilikili/pages/whisper_settings/widgets/item.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class WhisperSettingsPage extends StatefulWidget {
  const WhisperSettingsPage({
    super.key,
    required this.imSettingType,
  });

  /// 协议的 `ImSettingType`。跳子设置页时由
  /// `ImSettingRedirect.parentSettingType` 给。
  final int imSettingType;

  @override
  State<WhisperSettingsPage> createState() => _WhisperSettingsPageState();
}

class _WhisperSettingsPageState extends State<WhisperSettingsPage> {
  late final WhisperSettingsController _controller;

  @override
  void initState() {
    super.initState();
    _controller = Get.put(
      WhisperSettingsController(imSettingType: widget.imSettingType),
      tag: '${widget.imSettingType}',
    );
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return SimpleScaffold(
      appBar: AppBar(
        title: Obx(() => Text(_controller.title.value)),
      ),
      body: Obx(() => _buildBody(theme, _controller.loadingState.value)),
    );
  }

  /// 跳转项点下去。分支顺序与基线一致。
  ///
  /// 两处按**标题文字**分流的特例（「黑名单」「消息屏蔽词」）也照搬——
  /// 服务端给的是一个 url，而这两页在 app 内有原生实现，基线就是靠标题
  /// 认出来的。它看着脆，但换成按 url 认一样脆，而且会与基线分叉。
  void _onRedirect(ImSetting item) {
    final redirect = item.redirect;
    if (redirect == null) return;

    if (redirect.parentSettingType case final type?) {
      Get.to(
        WhisperSettingsPage(imSettingType: type),
        preventDuplicates: false,
      // 子页保存成功后自己会重拉；父页这一行右侧的 trailing 文字在返回
      // 时重拉一次对齐。
      )?.then((_) => _controller.queryData());
      return;
    }

    if (redirect.windowOptions.isNotEmpty) {
      showDialog(
        context: context,
        builder: (context) => SimpleDialog(
          clipBehavior: Clip.hardEdge,
          contentPadding: const EdgeInsets.symmetric(vertical: 12),
          children: List.generate(redirect.windowOptions.length, (i) {
            final option = redirect.windowOptions[i];
            final selected = redirect.windowSelectedIndex == i;
            return DialogOption(
              onPressed: () {
                if (selected) return;
                Get.back();
                _controller.onSet(key: item.key, selectedValue: option.value);
              },
              child: Text(
                option.title ?? '',
                style: TextStyle(
                  fontSize: 14,
                  color: selected
                      ? Theme.of(context).colorScheme.primary
                      : null,
                ),
              ),
            );
          }),
        ),
      );
      return;
    }

    if (redirect.otherUrl case final url?) {
      if (item.title == '黑名单') {
        Get.toNamed('/blackListPage');
      } else if (url.startsWith('http')) {
        Get.toNamed('/webview', parameters: {'url': url});
      } else {
        SmartDialog.showToast(url);
      }
      return;
    }

    if (redirect.pageUrl case final url?) {
      if (item.title == '消息屏蔽词') {
        Get.to(const WhisperBlockPage());
      } else if (url.startsWith('http')) {
        Get.toNamed('/webview', parameters: {'url': url});
      } else {
        SmartDialog.showToast(url);
      }
    }
  }

  Widget _buildBody(
    ThemeData theme,
    LoadingState<ImSettingsSnapshot> loadingState,
  ) {
    switch (loadingState) {
      case Loading():
        return const SizedBox.shrink();
      case Success(:final response):
        final divider = Divider(
          height: 1,
          color: theme.colorScheme.outline.withValues(alpha: 0.1),
        );
        return Builder(
          builder: (context) {
            // Rust 侧按 key 的自然顺序摊平（prost 的 map 是 BTreeMap），
            // 这里不用再排一次。
            final settings = response.settings;
            return ListView.separated(
              padding: EdgeInsets.only(
                bottom: MediaQuery.viewPaddingOf(context).bottom + 100,
              ),
              itemCount: settings.length,
              itemBuilder: (context, index) {
                final item = settings[index];
                return ImSettingsItem(
                  item: item,
                  onSwitch: (on) =>
                      _controller.onSet(key: item.key, switchOn: on),
                  onSelect: (value) =>
                      _controller.onSet(key: item.key, selectedValue: value),
                  onRedirect: () => _onRedirect(item),
                );
              },
              separatorBuilder: (context, index) => divider,
            );
          },
        );
      case Error(:final errMsg):
        return scrollErrorWidget(
          errMsg: errMsg,
          onReload: _controller.onReload,
        );
    }
  }
}
