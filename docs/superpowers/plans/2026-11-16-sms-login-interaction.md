# 短信登录交互实现计划

> **For agentic workers:** 使用 superpowers:executing-plans 在当前分支按步骤实施。

**Goal:** 完成 #140 的自适应验证码弹窗、自动聚焦与六位自动登录。

**Architecture:** 保持现有登录 HTTP 和验证码桥接；新增可独立测试的验证码输入与弹窗外壳 Widget，控制器负责共享提交锁及输入快照。

**Tech Stack:** Flutter、Dart、flutter_inappwebview、flutter_test。

- [ ] 新增 `test/pages/login/sms_code_field_test.dart` 与 `test/pages/login/geetest/captcha_dialog_frame_test.dart`，运行并确认失败。
- [ ] 新增 `lib/pages/login/sms_code_field.dart` 与 `lib/pages/login/geetest/captcha_dialog_frame.dart`：验证码六位输入和异步提交保护、自适应 Dialog 外壳。
- [ ] 修改 `lib/pages/login/view.dart` 接入输入组件及焦点；修改 `controller.dart` 管理焦点生命周期，发送成功请求焦点，手动/自动登录共用并发保护及输入快照。
- [ ] 修改 `geetest_webview_dialog.dart` 使用弹窗外壳，保留错误与关闭功能；移除 WebView 的 iOS 三倍缩放，原生 SDK 补丁需验证仍可应用。
- [ ] 更新 `docs/qa/RELEASE-VALIDATION.md`、`docs/m0/VALIDATION-CAPABILITY.md` 与 `docs/architecture/upstream-divergence.md`；在 #140 记录宿主例外。
- [ ] 运行 `.fvm/flutter_sdk/bin/flutter test --no-pub test/pages/login`、`.fvm/flutter_sdk/bin/dart analyze lib`，检查本次文件的错误交集。
- [ ] 构建 Android arm64 release，`adb install -r` 覆盖安装并验证启动。保留用户 `.vscode/settings.json` 修改，清理构建产生的无关文件。
