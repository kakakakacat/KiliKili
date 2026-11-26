# 首页导航首次加载探针（#127）

在 `build/` 中复制指定提交的**完整产品应用**，仅给复制件插桩，生成单独包名
`io.github.kakacat.kilikili.navprofile` 的 profile 包。不改 SDK/pub cache，不覆盖
现有应用，不清数据；不伪造登录态。用户在这个测试包里实际登录后可重复相同采样。
正常产品入口不引用这些探针模板。

```sh
python3 tool/navigation_profile/prepare.py --name candidate
cd build/navigation-profile-candidate
flutter build apk --profile --no-pub --target-platform android-arm64 \
  -t lib/navigation_profile_main.dart
adb install -r build/app/outputs/flutter-apk/app-profile.apk
cd ../..
python3 tool/navigation_profile/run.py --rounds 5 --expect-login no \
  --output build/navigation-profile-results/anonymous-candidate
python3 tool/navigation_profile/summarize.py build/navigation-profile-results/anonymous-candidate
```

需要 root 项目的 `android/local.properties`、Flutter SDK、Android arm64 工具链和
已打补丁的依赖。`--revision` 选择提交。`--reuse-build` 仅重置 Dart 与插桩所需
Android 文件，限原生代码相同的导航变更对比；原生代码变更时须重新生成独立目录。

起点是导航回调；每次进程中先等待首页 6 秒，再首次进入目标页及回访两次。
先跑一轮预热，正式每页 5 次新进程。脚本只停止该独立包，不清系统日志。
在采样结束前不要手动操作设备或登录。结束后保持应用打开，完成真实登录再用 `--expect-login yes` 重跑；登录态不符时停止。

汇总默认要求每页五轮（预热可传 `--rounds 1`），每轮要求三个完整样本，缺失事件、数据仍 Loading、超时、被其他应用切到后台或缺少对应 FrameTiming
均不能视作成功。JSONL 仅保存 `NAV_PROFILE` 事件：不保存账号、URL、响应正文，
也不保存 VM service 授权地址。失败时保留原始记录以便排查。

`first_paint` 名称表示目标页进入可见合成帧，不要求缓存页面层重新绘制。
`raster_finish_ms` 为该帧光栅结束，不是面板展示时间。
`data_terminal_ms` 是等页面到位后以 10ms 轮询观察到终态的时间，可能晚于真实
请求完成；请求 await 返回点另记 `request_result`。匿名“我的”没有数据请求。
报告须区分这些边界，不能把整帧开销归因于单一 Widget。
