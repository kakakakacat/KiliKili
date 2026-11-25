import 'package:kilikili/common/constants.dart';
import 'package:kilikili/common/dial_prefix.dart';
import 'package:kilikili/common/widgets/capsule_segmented_tab_bar.dart';
import 'package:kilikili/common/widgets/loading_widget/http_error.dart';
import 'package:kilikili/common/widgets/loading_widget/loading_widget.dart';
import 'package:kilikili/common/widgets/scaffold/simple_scaffold.dart';
import 'package:kilikili/common/widgets/scroll_physics.dart' show tabBarView;
import 'package:kilikili/common/widgets/view_insets_safe_area.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/login/controller.dart';
import 'package:kilikili/pages/login/sms_code_field.dart';
import 'package:kilikili/utils/extension/widget_ext.dart';
import 'package:kilikili/utils/image_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pretty_qr_code/pretty_qr_code.dart';
import 'package:url_launcher/url_launcher.dart';

class LoginPage extends StatefulWidget {
  const LoginPage({super.key});

  @override
  State<LoginPage> createState() => _LoginPageState();
}

class _LoginPageState extends State<LoginPage> {
  final LoginPageController _loginPageCtr = Get.put(LoginPageController());
  // 二维码生成时间
  bool showPassword = false;
  GlobalKey globalKey = GlobalKey();

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _loginPageCtr.didChangeDependencies(context);
  }

  /// 主操作：提交登录。**整行填充胶囊**，一屏只会有一个（#162、#236）。
  ///
  /// 整行而不是按内容宽度：这是这一屏唯一的落点，宽度本身就是层级。
  /// 宽屏不会拉成长条——外层 [tabViewOuter] 已经把内容列限在 625（见
  /// `widget_ext.dart` 的 `constraintWidth`）。
  ///
  /// 这里不设 `shape`：M3 的 FilledButton 默认就是 [StadiumBorder]，再写一遍
  /// 只会在以后换主题时多一处要同步的地方。
  Widget _primaryAction({
    required VoidCallback? onPressed,
    required IconData icon,
    required String label,
  }) => Padding(
    // 左右 20 与表单一致；上间距收进这里，免得每个 tab 各写一个 SizedBox，
    // 写漏一个就是按钮贴着输入框（Cookie 页原来就是这样）。
    padding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
    child: FilledButton.icon(
      onPressed: onPressed,
      icon: Icon(icon, size: 20),
      label: Text(label),
      style: FilledButton.styleFrom(
        minimumSize: const Size.fromHeight(56),
        padding: const EdgeInsets.symmetric(horizontal: 28),
        textStyle: const TextStyle(fontWeight: FontWeight.w600, fontSize: 16),
      ),
    ),
  );

  /// 次操作：获取验证码、刷新二维码、保存至相册等。tonal 胶囊，比主操作轻一级。
  ///
  /// 禁用态（如验证码冷却）刻意不自绘：tonal 的默认禁用色本身就够暗，自己调
  /// 反而会和别处的禁用按钮对不上。
  Widget _secondaryAction({
    required VoidCallback? onPressed,
    required IconData icon,
    required String label,
  }) => FilledButton.tonalIcon(
    onPressed: onPressed,
    icon: Icon(icon, size: 18),
    label: Text(label),
    style: FilledButton.styleFrom(
      minimumSize: const Size(0, 40),
      padding: const EdgeInsets.symmetric(horizontal: 18),
      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
    ),
  );

  /// 胶囊输入栏：填充 + 全圆角，和分段胶囊、整行按钮同一套形状语言（#236）。
  ///
  /// 标签不上浮（[FloatingLabelBehavior.never]）：胶囊里没有放上浮标签的位置，
  /// 硬塞会把文字挤到描边上。登录这几个字段（账号 / 密码 / 手机号 / 验证码 /
  /// Cookie）内容本身足够自明，标签当占位符用就够。
  ///
  /// 多行（Cookie）用大圆角而不是胶囊：几行高的 stadium 会变成一个椭圆。
  InputDecoration _pillInput(
    ThemeData theme, {
    required String label,
    String? hint,
    Widget? prefixIcon,
    Widget? suffixIcon,
    bool multiline = false,
  }) {
    final radius = BorderRadius.circular(multiline ? 24 : 28);
    OutlineInputBorder border([Color? color, double width = 1]) =>
        OutlineInputBorder(
          borderRadius: radius,
          borderSide: color == null
              ? BorderSide.none
              : BorderSide(color: color, width: width),
        );
    return InputDecoration(
      filled: true,
      fillColor: theme.colorScheme.surfaceContainerHighest,
      labelText: label,
      hintText: hint,
      floatingLabelBehavior: FloatingLabelBehavior.never,
      prefixIcon: prefixIcon,
      suffixIcon: suffixIcon,
      contentPadding: EdgeInsets.symmetric(
        horizontal: prefixIcon == null ? 20 : 8,
        vertical: multiline ? 18 : 16,
      ),
      // 平时不描边，靠填充色表达形状；聚焦和报错才上描边，否则一排胶囊
      // 全是轮廓线会很吵。
      border: border(),
      enabledBorder: border(),
      focusedBorder: border(theme.colorScheme.primary, 2),
      errorBorder: border(theme.colorScheme.error),
      focusedErrorBorder: border(theme.colorScheme.error, 2),
    );
  }

  /// 手机号、验证码这两行是「前缀 + 输入框 + 动作」的组合，塞不进
  /// [_pillInput]，所以单独给一个同样外观的壳，保证五个输入栏长得一致。
  Widget _pillShell(ThemeData theme, {required Widget child}) => DecoratedBox(
    decoration: ShapeDecoration(
      color: theme.colorScheme.surfaceContainerHighest,
      shape: const StadiumBorder(),
    ),
    child: child,
  );

  Widget loginByQRCode(ThemeData theme) {
    return Column(
      children: [
        const SizedBox(height: 20),
        const Text('使用 bilibili 官方 App 扫码登录'),
        const SizedBox(height: 20),
        Obx(
          () => Text(
            '剩余有效时间: ${_loginPageCtr.qrCodeLeftTime} 秒',
            style: TextStyle(
              fontFeatures: const [FontFeature.tabularFigures()],
              color: theme.colorScheme.primaryFixedDim,
            ),
          ),
        ),
        const SizedBox(height: 5),
        // 用 Wrap 而不是 Row：三个胶囊比原来的文字按钮宽，窄屏上 Row 会直接
        // 溢出报黄黑条（#162 验收第二条）。
        Wrap(
          alignment: WrapAlignment.center,
          spacing: 8,
          runSpacing: 8,
          children: [
            _secondaryAction(
              onPressed: _loginPageCtr.refreshQRCode,
              icon: Icons.refresh,
              label: '刷新二维码',
            ),
            _secondaryAction(
              onPressed: () async {
                SmartDialog.showLoading(msg: '正在生成截图');
                final boundary =
                    globalKey.currentContext!.findRenderObject()
                        as RenderRepaintBoundary;
                final image = await boundary.toImage(pixelRatio: 3);
                final byteData = await image.toByteData(format: .png);
                final pngBytes = byteData!.buffer.asUint8List();
                image.dispose();
                SmartDialog.dismiss();
                final picName =
                    "${Constants.appName}_loginQRCode_${_loginPageCtr.codeInfo.value.data.authCode.hashCode.toUnsigned(32).toRadixString(16)}";
                ImageUtils.saveByteImg(bytes: pngBytes, fileName: picName);
              },
              icon: Icons.save_alt_outlined,
              label: '保存至相册',
            ),
            if (kDebugMode || PlatformUtils.isMobile)
              _secondaryAction(
                onPressed: () => PageUtils.launchURL(
                  'bilibili://browser?url=${Uri.encodeComponent(_loginPageCtr.codeInfo.value.data.url)}',
                  mode: LaunchMode.externalNonBrowserApplication,
                ),
                icon: Icons.open_in_browser_outlined,
                label: '其他应用打开',
              ),
          ],
        ),
        RepaintBoundary(
          key: globalKey,
          child: Obx(
            () => switch (_loginPageCtr.codeInfo.value) {
              Loading() => const SizedBox(
                height: 200,
                width: 200,
                child: m3eLoading,
              ),
              Success(:final response) => Container(
                width: 200,
                height: 200,
                color: Colors.white,
                padding: const EdgeInsets.all(8),
                child: PrettyQrView.data(
                  data: response.url,
                  decoration: const PrettyQrDecoration(
                    shape: PrettyQrSquaresSymbol(
                      color: Colors.black87,
                    ),
                  ),
                ),
              ),
              Error(:final errMsg) => HttpError(
                isSliver: false,
                errMsg: errMsg,
                onReload: _loginPageCtr.refreshQRCode,
              ),
            },
          ),
        ),
        const SizedBox(height: 10),
        Obx(
          () => Text(
            _loginPageCtr.statusQRCode.value,
            style: TextStyle(color: theme.colorScheme.secondaryFixedDim),
          ),
        ),
        Obx(
          () {
            final url = _loginPageCtr.codeInfo.value.dataOrNull?.url ?? '';
            return GestureDetector(
              onTap: () => Utils.copyText(
                url,
                toastText: '已复制到剪贴板，可粘贴至已登录的app私信处发送，然后点击已发送的链接打开',
              ),
              child: Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 20,
                  vertical: 20,
                ),
                child: Text(
                  url,
                  style: theme.textTheme.labelSmall!.copyWith(
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.4),
                  ),
                ),
              ),
            );
          },
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20),
          child: Text(
            '请务必在 ${Constants.appName} 开源仓库等可信渠道下载安装。',
            style: theme.textTheme.labelSmall!.copyWith(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.4),
            ),
          ),
        ),
      ],
    );
  }

  Widget loginByCookie(ThemeData theme) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        const SizedBox(height: 20),
        const Text('使用Cookie登录'),
        const SizedBox(height: 10),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20),
          child: Text(
            '使用App端Api实现的功能将不可用',
            style: theme.textTheme.labelMedium!.copyWith(
              color: theme.colorScheme.primary,
            ),
          ),
        ),
        const SizedBox(height: 10),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
          child: TextField(
            minLines: 1,
            maxLines: 10,
            controller: _loginPageCtr.cookieTextController,
            inputFormatters: [FilteringTextInputFormatter.deny(RegExp(r"\s"))],
            decoration: _pillInput(
              theme,
              label: 'Cookie',
              multiline: true,
              prefixIcon: const Icon(Icons.cookie_outlined),
              suffixIcon: IconButton(
                onPressed: _loginPageCtr.cookieTextController.clear,
                icon: const Icon(Icons.clear),
              ),
            ),
          ),
        ),
        _primaryAction(
          onPressed: _loginPageCtr.loginByCookie,
          icon: Icons.login,
          label: '登录',
        ),
      ],
    );
  }

  Widget loginByPassword(ThemeData theme) {
    return Column(
      children: [
        const SizedBox(height: 20),
        const Text('使用账号密码登录'),
        const SizedBox(height: 20),
        Padding(
          padding: const .symmetric(horizontal: 20),
          child: AutofillGroup(
            child: Column(
              spacing: 10,
              children: [
                TextField(
                  controller: _loginPageCtr.usernameTextController,
                  autofillHints: const [
                    AutofillHints.telephoneNumber,
                    AutofillHints.email,
                  ],
                  inputFormatters: [
                    FilteringTextInputFormatter.deny(RegExp(r"\s")),
                  ],
                  decoration: _pillInput(
                    theme,
                    label: '账号',
                    hint: '邮箱/手机号',
                    prefixIcon: const Icon(Icons.account_box),
                    suffixIcon: IconButton(
                      onPressed: _loginPageCtr.usernameTextController.clear,
                      icon: const Icon(Icons.clear),
                    ),
                  ),
                ),
                TextField(
                  obscureText: !showPassword,
                  keyboardType: TextInputType.visiblePassword,
                  inputFormatters: [
                    FilteringTextInputFormatter.deny(RegExp(r"\s")),
                  ],
                  controller: _loginPageCtr.passwordTextController,
                  autofillHints: const [AutofillHints.password],
                  decoration: _pillInput(
                    theme,
                    label: '密码',
                    prefixIcon: const Icon(Icons.password),
                    suffixIcon: IconButton(
                      onPressed: _loginPageCtr.passwordTextController.clear,
                      icon: const Icon(Icons.clear),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        Row(
          children: [
            const SizedBox(width: 10),
            Checkbox(
              value: showPassword,
              onChanged: (value) => setState(() => showPassword = value!),
            ),
            const Text('显示密码'),
            const Spacer(),
            // 第三层级：刻意留成无背景的文字按钮，只补一个图标。做成胶囊会和
            // 下面的「登录」抢，而它本就是条出口链接，不是这页要你点的操作。
            TextButton.icon(
              icon: const Icon(Icons.help_outline, size: 18),
              onPressed: () {
                //https://passport.bilibili.com/h5-app/passport/login/findPassword
                //https://passport.bilibili.com/passport/findPassword
                showDialog(
                  context: context,
                  builder: (context) => SimpleDialog(
                    clipBehavior: Clip.hardEdge,
                    title: const Text('忘记密码？'),
                    contentPadding: const .only(top: 2, bottom: 16),
                    children: [
                      const Padding(
                        padding: .fromLTRB(25, 0, 25, 10),
                        child: Text("试试扫码、手机号登录，或选择"),
                      ),
                      ListTile(
                        title: const Text(
                          '找回密码（手机版）',
                        ),
                        leading: const Icon(Icons.smartphone_outlined),
                        subtitle: const Text(
                          'https://passport.bilibili.com/h5-app/passport/login/findPassword',
                        ),
                        dense: false,
                        onTap: () => Get
                          ..back()
                          ..toNamed(
                            '/webview',
                            parameters: {
                              'url': 'https://passport.bilibili.com/h5-app/passport/login/findPassword',
                              'type': 'url',
                              'pageTitle': '忘记密码',
                            },
                          ),
                      ),
                      ListTile(
                        title: const Text('找回密码（电脑版）'),
                        leading: const Icon(Icons.desktop_windows_outlined),
                        subtitle: const Text(
                          'https://passport.bilibili.com/pc/passport/findPassword',
                        ),
                        dense: false,
                        onTap: () => Get
                          ..back()
                          ..toNamed(
                            '/webview',
                            parameters: {
                              'url': 'https://passport.bilibili.com/pc/passport/findPassword',
                              'type': 'url',
                              'pageTitle': '忘记密码',
                              'uaType': 'pc',
                            },
                          ),
                      ),
                    ],
                  ),
                );
              },
              label: const Text('忘记密码'),
            ),
            const SizedBox(width: 20),
          ],
        ),
        _primaryAction(
          onPressed: _loginPageCtr.loginByPassword,
          icon: Icons.login,
          label: '登录',
        ),
        const SizedBox(height: 20),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20),
          child: Text(
            '根据 bilibili 官方登录接口规范，密码将在本地加盐、加密后传输。\n'
            '盐与公钥均由官方提供；以 RSA/ECB/PKCS1Padding 方式加密。\n'
            '账号密码仅用于该登录接口，不予保存；本地仅存储登录凭证。\n'
            '请务必在 ${Constants.appName} 开源仓库等可信渠道下载安装。',
            textAlign: TextAlign.center,
            style: theme.textTheme.labelSmall!.copyWith(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.4),
            ),
          ),
        ),
      ],
    );
  }

  Widget loginBySmS(ThemeData theme) {
    return Column(
      children: [
        const SizedBox(height: 20),
        const Text('使用手机短信验证码登录'),
        const SizedBox(height: 10),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
          child: _pillShell(
            theme,
            child: Row(
              children: [
                const SizedBox(width: 12),
                Builder(
                  builder: (context) {
                    return PopupMenuButton(
                      padding: EdgeInsets.zero,
                      tooltip:
                          '选择国际冠码，'
                          '当前为${_loginPageCtr.selectedCountryCodeId.cname}，'
                          '+${_loginPageCtr.selectedCountryCodeId.countryId}',
                      onSelected: (item) {
                        _loginPageCtr.selectedCountryCodeId = item;
                        (context as Element).markNeedsBuild();
                      },
                      initialValue: _loginPageCtr.selectedCountryCodeId,
                      itemBuilder: (_) => Login.dialPrefix.map((item) {
                        return PopupMenuItem(
                          value: item,
                          child: Row(
                            children: [
                              Text(item.cname),
                              const Spacer(),
                              Text("+${item.countryId}"),
                            ],
                          ),
                        );
                      }).toList(),
                      child: Row(
                        children: [
                          Icon(
                            Icons.phone,
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                          const SizedBox(width: 12),
                          Text(
                            "+${_loginPageCtr.selectedCountryCodeId.countryId}",
                          ),
                        ],
                      ),
                    );
                  },
                ),
                const SizedBox(width: 6),
                SizedBox(
                  height: 24,
                  child: VerticalDivider(
                    color: theme.colorScheme.outline.withValues(alpha: 0.5),
                  ),
                ),
                const SizedBox(width: 6),
                Expanded(
                  child: TextField(
                    controller: _loginPageCtr.telTextController,
                    keyboardType: TextInputType.number,
                    inputFormatters: <TextInputFormatter>[
                      FilteringTextInputFormatter.digitsOnly,
                    ],
                    decoration: InputDecoration(
                      border: InputBorder.none,
                      floatingLabelBehavior: FloatingLabelBehavior.never,
                      labelText: '手机号',
                      suffixIcon: IconButton(
                        onPressed: _loginPageCtr.telTextController.clear,
                        icon: const Icon(Icons.clear),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
          child: _pillShell(
            theme,
            child: Row(
              children: [
                Expanded(
                  child: SmsCodeField(
                    controller: _loginPageCtr.smsCodeTextController,
                    focusNode: _loginPageCtr.smsCodeFocusNode,
                    onSubmit: _loginPageCtr.loginBySmsCode,
                  ),
                ),
                Obx(
                  () {
                    final cooling = _loginPageCtr.smsSendCooldown > 0;
                    return Padding(
                      // 壳本身已经是胶囊，这里只留与输入区的间隔。
                      padding: const EdgeInsets.only(left: 8, right: 4),
                      child: _secondaryAction(
                        onPressed: cooling
                            ? null
                            : () {
                                _loginPageCtr.smsCodeFocusNode.requestFocus();
                                _loginPageCtr.sendSmsCode();
                              },
                        // 冷却中换成表盘图标：光看文字「等待 N 秒」要读一遍才
                        // 知道按不动，图标一眼就能分辨（#162 验收第二条）。
                        icon: cooling
                            ? Icons.hourglass_bottom_outlined
                            : Icons.send_outlined,
                        label: cooling
                            ? '等待${_loginPageCtr.smsSendCooldown}秒'
                            : '获取验证码',
                      ),
                    );
                  },
                ),
              ],
            ),
          ),
        ),
        _primaryAction(
          onPressed: _loginPageCtr.loginBySmsCode,
          icon: Icons.login,
          label: '登录',
        ),
        const SizedBox(height: 20),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20),
          child: Text(
            '手机号仅用于 bilibili 官方发送验证码与登录接口，不予保存；\n'
            '本地仅存储登录凭证。\n'
            '请务必在 ${Constants.appName} 开源仓库等可信渠道下载安装。',
            textAlign: TextAlign.center,
            style: theme.textTheme.labelSmall!.copyWith(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.4),
            ),
          ),
        ),
      ],
    );
  }

  late EdgeInsets padding;

  /// 四个登录方式入口：一条分段胶囊，选中项在轨道里滑动。
  ///
  /// 改成分段控件而不是沿用首页那种分开的胶囊（#138），理由写在
  /// [CapsuleSegmentedTabBar] 的类注释里：固定几选一和数量不定的分类是
  /// 两种场景。位置也从「竖屏占一行 / 横屏塞进 AppBar」统一成了内容列顶部，
  /// 两种朝向一份布局，并且和下面的输入框、整行按钮左右对齐。
  Widget _methodTabBar() => CapsuleSegmentedTabBar(
    controller: _loginPageCtr.tabController,
    items: _loginMethods,
  );

  static const List<(IconData, String)> _loginMethods = [
    (Icons.password, '密码'),
    (Icons.sms_outlined, '短信'),
    (Icons.qr_code, '扫码'),
    (Icons.cookie_outlined, 'Cookie'),
  ];

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    padding =
        MediaQuery.viewPaddingOf(context).copyWith(top: 0) +
        const EdgeInsets.only(bottom: 25);
    return SimpleScaffold(
      appBar: AppBar(
        leading: IconButton(
          tooltip: '关闭',
          icon: const Icon(Icons.close_outlined),
          onPressed: Get.back,
        ),
        title: const Text('登录'),
      ),
      body: Column(
        children: [
          // 和下面的表单同宽同边距，三者（分段胶囊 / 输入框 / 整行按钮）左右
          // 对齐成一列。横屏不再把它塞进 AppBar——那里宽度不可控，分段控件
          // 会被挤扁；body 本身可滚动，少掉的那点竖向空间不是问题。
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 4, 20, 12),
            child: _methodTabBar().constraintWidth(),
          ),
          Expanded(
            child: NotificationListener<ScrollStartNotification>(
              onNotification: (notification) {
                if (notification.metrics.axis == Axis.horizontal) {
                  FocusScope.of(context).unfocus();
                }
                return false;
              },
              child: ViewInsetsSafeArea(
                child: tabBarView(
                  controller: _loginPageCtr.tabController,
                  children: [
                    tabViewOuter(loginByPassword(theme)),
                    tabViewOuter(loginBySmS(theme)),
                    tabViewOuter(loginByQRCode(theme)),
                    tabViewOuter(loginByCookie(theme)),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget tabViewOuter(Widget child) {
    return SingleChildScrollView(
      padding: padding,
      child: child.constraintWidth(),
    );
  }
}
