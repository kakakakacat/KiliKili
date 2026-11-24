# Player Port 独立验收宿主（#4）

这不是产品入口。它在 `build/player-port-gui` 生成独立包名的 Flutter app，
直接引用当前 checkout 的适配器和 FRB 生成代码，复用锁定的 media-kit、
`SimpleVideo`、canvas_danmaku 和真实脱敏 protobuf。无需账号，不覆盖产品应用。

先执行 `tool/player-port-runtime-test.sh`，然后：

```sh
python3 tool/player_port_probe/prepare.py
cd build/player-port-gui
flutter build macos --debug --no-pub
# 启动生成的 kilikili.app，按「运行验收」。
```

macOS/iOS 用已静态链接的 Rust 库，Android 用 bridge_flutter 插件打包的动态库。
探针生成工程继承产品工程的旧 Android 插件 compileSdk/JVM 兼容设置，
仅编译 arm64，避免 debug 的模拟器 ABI 占满磁盘。prepare 不改全局 SDK 或缓存。
iOS 开发团队由本机 Flutter/Xcode 配置选择，不在脚本里固定个人身份。

Android 14 / arm64 设备的自动脚本：

```sh
flutter build apk --debug --no-pub --target-platform android-arm64 \
  --dart-define=AUTORUN=true --dart-define=LIFECYCLE_PROBE=true \
  --dart-define=PIP_PROBE=true
```

安装生成的独立 APK 后启动；在日志 `WAIT_BACKGROUND` 时切回桌面，再回应用。
`PIP_PLAYING` 时应看到小窗持续播放，随后返回应用。两处各有 45 秒期限，
不是收到命令就判通过；实际生命周期、PiP 回调和后端时钟推进都要成立。
`events.jsonl` / `result.json` 写入 app 的临时目录，日志带 `PLAYER_PROBE` 前缀。

断言覆盖：真实弹幕上屏、暂停位置/弹幕冻结、前跳清屏、回跳、0.5×/2×、
320→640 分辨率切换、受控 HTTP 缓冲时位置/弹幕冻结及恢复、销毁后会话数归零、重开后旧订阅不回写。
Android PiP 调用系统 API；iOS 则直接复制当前产品的 `PipPlugin.swift` 到生成工程，
并启用后台音频会话，接收实际状态回调。探针不覆盖完整产品页面的交互服务。
本探针使用 `ao=null` / `hwdec=no`，不能作为可听音轨、硬解或完整产品页面的证据。

iPhone 使用 profile 模式，避免 iOS 要求调试器才能启动 debug FlutterEngine：

```sh
flutter build ios --profile --no-pub --dart-define=AUTORUN=true \
  --dart-define=LIFECYCLE_PROBE=true --dart-define=PIP_PROBE=true
# 用本机开发证书签名所有内嵌 framework，再签 app，安装到设备。
# CoreDevice 驱动脚本只操作独立测试包和 Settings（用来切到后台，不改设置）。
IOS_DEVICE=<设备 ID> python3 ../../tool/player_port_probe/ios-run.py
```

Android 驱动脚本：启动独立包后在仓库根目录运行
`ANDROID_SERIAL=<设备 ID> python3 tool/player_port_probe/android-run.py`。
两者需要 `adb` / Xcode 的 `devicectl` 在 PATH 中；生成 JSON 和截图放在 `build/`。
首次 iOS 无线数据提示需要在手机处理；免费开发签名的三应用上限可能阻止安装，
脚本不会卸载其他应用。签名与安装要用本机开发身份，不把任何证书/私钥写入仓库。

同素材基线：`tool/player-port-benchmark.sh` 在两个独立 Flutter test 进程里
各重复三轮，比较 raw media-kit 与 Player Port 的 wall time / RSS / 回调数。
当前脚本使用 macOS dylib；它量的是无窗口软件解码，不是 UI 帧耗时。
`tool/player-port-native-runtime-test.sh` 复用 Slint 原型的 MpvPlayer，
比较暂停、前后 Seek、倍速、不同分辨率资源和同样限速/阻塞条件下的缓冲恢复；视频表面另跑 Slint 窗口脚本。
