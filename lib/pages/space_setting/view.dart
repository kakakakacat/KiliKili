import 'dart:math';

import 'package:kilikili/common/widgets/loading_widget/loading_widget.dart';
import 'package:kilikili/common/widgets/scaffold/simple_scaffold.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_user.dart';
import 'package:kilikili/pages/space_setting/controller.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class SpaceSettingPage extends StatefulWidget {
  const SpaceSettingPage({super.key});

  @override
  State<SpaceSettingPage> createState() => _SpaceSettingPageState();
}

class _SpaceSettingPageState extends State<SpaceSettingPage> {
  final _controller = Get.put(SpaceSettingController());

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SimpleScaffold(
      appBar: AppBar(title: const Text('空间设置')),
      body: Obx(() => _buildBody(theme, _controller.loadingState.value)),
    );
  }

  @override
  void dispose() {
    _controller.onMod();
    super.dispose();
  }

  Widget _buildBody(
    ThemeData theme,
    LoadingState<List<PrivacySetting>> loadingState,
  ) {
    return switch (loadingState) {
      Loading() => const SizedBox.shrink(),
      Success<List<PrivacySetting>>(:final response) =>
        response.isEmpty
            ? scrollErrorWidget(onReload: _controller.onReload)
            : Builder(
                builder: (context) {
                  final padding = MediaQuery.viewPaddingOf(context);
                  final divider = Divider(
                    height: 1,
                    indent: max(16, padding.left),
                    color: theme.colorScheme.outline.withValues(alpha: 0.1),
                  );
                  final dividerL = SliverToBoxAdapter(
                    child: Divider(
                      height: 12,
                      thickness: 12,
                      color: theme.colorScheme.outline.withValues(alpha: 0.1),
                    ),
                  );
                  return CustomScrollView(
                    slivers: [
                      dividerL,
                      SliverList.separated(
                        itemCount: _group(response, 1).length,
                        itemBuilder: (context, index) {
                          return _item(_group(response, 1)[index]);
                        },
                        separatorBuilder: (context, index) => divider,
                      ),
                      dividerL,
                      SliverList.separated(
                        itemCount: _group(response, 2).length,
                        itemBuilder: (context, index) {
                          return _item(_group(response, 2)[index]);
                        },
                        separatorBuilder: (context, index) => divider,
                      ),
                      dividerL,
                      SliverList.separated(
                        itemCount: _group(response, 3).length,
                        itemBuilder: (context, index) {
                          return _item(_group(response, 3)[index]);
                        },
                        separatorBuilder: (context, index) => divider,
                      ),
                      dividerL,
                      SliverToBoxAdapter(
                        child: SizedBox(height: padding.bottom + 100),
                      ),
                    ],
                  );
                },
              ),
      Error(:final errMsg) => scrollErrorWidget(
        errMsg: errMsg,
        onReload: _controller.onReload,
      ),
    };
  }

  /// 界面上的第几组。分组由 Rust 给（它与 key、是否反相一样属于协议语义），
  /// 这里只按它筛一遍。
  List<PrivacySetting> _group(List<PrivacySetting> all, int group) =>
      all.where((e) => e.group == group).toList();

  Widget _item(PrivacySetting item) {
    return Builder(
      builder: (context) {
        void onChanged([bool? value]) {
          _controller.toggle(item, value);
          (context as Element).markNeedsBuild();
        }

        return ListTile(
          dense: true,
          onTap: onChanged,
          title: Text(
            // 标题是文案，留在宿主；key 与「是否反相」在 Rust。
            RustUser.privacyLabels[item.key] ?? item.key,
            style: const TextStyle(fontSize: 14),
          ),
          trailing: Transform.scale(
            alignment: Alignment.centerRight,
            scale: 0.8,
            child: Switch(
              value: _controller.isOn(item),
              onChanged: onChanged,
            ),
          ),
        );
      },
    );
  }
}
