# SPIKE-003 Android 这一跑：人看出来的结论（issue #10）

机器记录是 [2026-11-22-slint-ui-android.md](2026-11-22-slint-ui-android.md)，
跑法是 [`spikes/slint-ui/android-drive.sh`](../../../spikes/slint-ui/android-drive.sh)。
那份由程序整份覆盖（末尾的「宿主机侧证据」由驱动脚本追加），这份放它写不出来
的东西。

设备：小米 13 Ultra（2304FPN6DC），Android 14，Adreno 740，1116×2480 @ 2.75，
120Hz 屏。**跑的是和 Windows 那一轮同一个 `app::run`**，不是另写的探针——
两份记录要能逐行对照，流程就不能各写一份。

本轮覆盖 #10 的长列表 / 富文本 / 输入 / 无障碍四项，外加 Windows 上答不了的
**触控惯性**。**#10 仍然不能关**，还差的在 §7。

---

## 0. 最贵的结论：Android 上 Slint 的无障碍树是空的

`uiautomator dump` 抓回来的整棵树只有 **10 个节点**，属于这个应用的内容部分
是**一个空的 `android.view.View`**——没有子节点、没有 `text`、没有
`content-desc`、没有角色。树原文留在
[`assets/2026-11-22-slint-ui-android-a11y.xml`](assets/2026-11-22-slint-ui-android-a11y.xml)（3.6 KB）。

它读的就是 AccessibilityNodeInfo 树，**TalkBack 读的正是这棵树**。也就是说
屏幕上那一屏评论、按钮、勾选框、滑块，**读屏软件一个都拿不到**。

三条证据互相独立，指向同一件事：

| 证据 | 说明 |
|---|---|
| 抓到的树 | 10 个节点，带名字的只有 1 个，且那一个是系统画的标题 |
| `i-slint-backend-android-activity` 1.19.0 的源码 | 整个 crate **没有一处**无障碍代码，唯一出现 "Accessibility" 的地方是读「减少动画」设置（`javahelper.rs:587`） |
| `cargo tree -i accesskit` | Windows 的依赖图里是 `accesskit → accesskit_windows → accesskit_winit → i-slint-backend-winit`；**Android 的依赖图里一个 accesskit 都没有** |

**这推翻了上一轮的一个说法。** 2026-11-22 Windows 那份 notes 的 §4 写着
「Windows 这一侧能读，但这只是三分之一」，当时的预期是另外两个平台「各走另
一套 API、都没验」。现在验了一个：**不是「另一套 API」，是根本没接**。
`accessible-role` / `accessible-label` 这些属性在 `.slint` 里照写不误、
编译也不报错，桌面上 UIA 读得出来，到了 Android 就安静地什么都不做。

> **失败是静默的**：没有报错、没有警告、属性照常生效（它们同时还影响
> 焦点与命中），只有去抓树才知道外面什么都看不见。

对 Phase 2 的意义：这是第四件「做不到」，而且和另外三件不同——
[补丁评估](2026-11-22-flutter-patches-slint-eval.md) §2 那三件都是「自己写一个
更大的东西能绕过去」，这一件绕不过去：无障碍树必须由**框架**在 JNI 那一侧
实现，应用层给不出来。

### 0.1 但上游正在做，而且做得很完整（2026-11-22 查）

查完上游之后这条结论要改一次语气：**不是「要么自己提 PR 要么放弃」，是「等」。**

`slint-ui/slint` 上有一组 7 个 PR 的系列，作者 `fidderr`，标题前缀
`[a11y n/7]`，就是在给 Android 接 AccessKit：

| PR | 内容 | 状态（2026-11-22 查） |
|---|---|---|
| [#15055](https://github.com/slint-ui/slint/pull/15055) | 1/7 把 AccessKit 的树代码在各后端之间共用 | **已合** |
| [#15056](https://github.com/slint-ui/slint/pull/15056) | 2/7 让辅助技术能滚动 `Flickable` 的内容 | 待评审 |
| [#15057](https://github.com/slint-ui/slint/pull/15057) | 3/7 popup 打开时把窗口其余部分藏起来 | 待评审 |
| [#14977](https://github.com/slint-ui/slint/pull/14977) | 4/7 加 `accessible-modal` 属性 | 待评审 |
| [#15058](https://github.com/slint-ui/slint/pull/15058) | 5/7 不要立刻复用已释放的组件 id | 待评审 |
| [#14976](https://github.com/slint-ui/slint/pull/14976) | **6/7 android: 通过 AccessKit 支持无障碍**（+744/-7，13 个文件） | 待评审 |
| [#15059](https://github.com/slint-ui/slint/pull/15059) | 7/7 给 NativeActivity 画无障碍焦点框 | 草稿，等 6/7 |

6/7 的描述里那句话和本文 §0 是同一件事：「Slint apps on Android are invisible
to TalkBack, Switch Access and UI test tools: they only see a single view.」
它走的正是 `accesskit_android`，**支持 NativeActivity**——也就是我们这个形态。

PR 下面有一份 2026-11-21 的第三方实测（Android 13 模拟器，`uiautomator` +
`UiAutomation`），结论值得抄过来，因为它正好接着我们这一轮：

- **打补丁前**：「the app is one opaque view, 8 nodes with the window
  decoration」——**和我们抓到的 10 个节点是同一个现象**，互相印证；
- 打补丁后：`EditText` / `Button` / `CheckBox` / `ListView` / `Spinner` /
  `SeekBar` 都出现在树里，带标签与状态，`ACTION_CLICK`、`ACTION_SET_PROGRESS`
  可用，缩放 1.875 / 2.756 / 3.75 与横屏下 bounds 都对得上；
- 仍有缺口：`ComboBox` 不能靠 action 操作、`LineEdit` 没有 `ACTION_SET_TEXT`
  且空框没有标签、禁用的按钮 `performAction` 也返回 true、服务刚连上的头
  一次查询拿到占位树（9 次里有 3 次 `uiautomator dump` 抓不到 Slint 节点）。

Slint 的发版节奏是约两个月一个小版本（1.17 六月 / 1.18 九月 /
**1.19 2026-11-16**），所以这一串如果在接下来几周合掉，**最早出现在 1.20**。

> **结论改成**：Android 无障碍从「做不到」降为「**等上游，并且要自己验**」。
> 但 Phase 2 的计划里不能把它当成已经有了——7 个 PR 里只合了 1 个，而且合进
> 去之后上面那一串缺口要按我们自己的界面再验一遍。

### 0.2 顺带查到一个会反过来咬性能的东西

[slint#3867](https://github.com/slint-ui/slint/issues/3867)「`accessibility`
feature have big performance impact on big listview」**2023 年开到现在还开着**
（最后更新 2026-10-11）。

这条对我们特别要紧，因为两件事叠在一起：

- 本轮 Android 的帧耗时（滚动 p95 10.5ms）是在**完全没有无障碍树**的情况下
  量的——后端根本没接，一行树都没建；
- 我们的列表是**两万行**，正是那个 issue 抱怨的形状。

也就是说，等上游把 Android 无障碍合进来之后，**这一轮的性能数字要重新量**。
别把「10.5ms，还有余量」当成接上无障碍之后仍然成立。

---

## 1. 长列表：过，但余量从 18 倍掉到 1.6 倍

两万条真实形状的评论，同一份脚本、同一个 `run()`：

| 项 | Windows（RX 7900 GRE，1470×930@1.5） | **Android（Adreno 740，1116×2480@2.75）** |
|---|---|---|
| 匀速滚动 单帧渲染 p50 | 0.7 ms | **7.3 ms** |
| 匀速滚动 单帧渲染 p95 | 0.9 ms | **10.5 ms** |
| 匀速滚动 最慢一帧 | 2.1 ms | **26.2 ms** |
| 跳到第 19999 行 最慢一帧 | 8.4 ms | **15.2 ms** |
| 整段累计实例化 | 342 / 20000 | **251 / 20000** |
| RSS 峰值 | 265 MiB | **349 MiB** |
| 生产分词（两万条） | 3.1 s | **2.0 s** |

**结论仍然是过**：60Hz 一帧的预算 16.7ms，滚动 p95 用掉 10.5ms。但余量不再是
「用掉 5%」，是**用掉 63%**。两件事要分开看：

- **像素多了 2 倍，代价高了 10 倍。** 1116×2480 = 2.77M 像素对 1470×930 =
  1.37M，移动 GPU 的填充率差距就在这里。真机上的 margin 要按这个数规划，
  别拿桌面的数做 Phase 2 的预算。
- **最慢一帧 26.2ms 已经掉帧了**，120Hz 下更明显（一帧预算只有 8.3ms）。
  滚动段 370 帧里长帧 0 次，所以是偶发不是常态，但「桌面上最慢 2.1ms」那种
  安全感在真机上不存在。

有意思的是**生产分词反而更快**（2.0s vs 3.1s）：这是 CPU 单核性能，不是 GPU。
结论不变——分词必须离开 UI 线程。

## 2. 触控惯性：Slint 自带，而且是这一轮最漂亮的数

Windows 那台没有触摸屏，这一项一直是 NOT TESTED。这次在列表场景末尾留了
8 秒空窗（`--touch-ms 8000`，探针自己不滚），滚动完全由 `adb input swipe`
驱动——三次甩动（300ms 走半屏）加一次慢拖（1200ms）：

| 项 | 值 |
|---|---|
| 这一段画了多少帧 | **573**（8 秒） |
| 单帧渲染 | p50 **2.5** / p95 **6.0** / 最慢 22.1 ms |
| 帧间隔 | p50 **8.2** ms（= 122 fps） |

松手之后列表继续滚（截图 `sys-10-after-fling` 里 HUD 写着 **114 fps**，
列表停在第 19~26 行），**惯性与橡皮筋是 `Flickable` 自己给的，不用自己写**。
这和[补丁评估](2026-11-22-flutter-patches-slint-eval.md) §4 里「白捡的那几项」
对得上。

**但这里有一个比数字更重要的观察**：同一台设备上，

- 探针**自己驱动**的匀速滚动：帧间隔 p50 16.6ms（60fps）；
- **手指驱动**的滚动：帧间隔 p50 8.2ms（120fps）。

差别不在 Slint 画得快慢（渲染耗时反而是触摸那段更低），在**谁触发重绘**：
探针那段是 16ms 定时器改属性，触摸那段是输入事件直接驱动。Slint 是事件驱动
渲染的，**宿主侧用定时器推动画面，等于把自己限在定时器的频率上**。Phase 2
里凡是「跟手」的东西（滚动、拖拽、手势），都不要经过自己的定时器。

> 下拉刷新**仍然没测**：Slint 没有这个控件，要先自己写一个才谈得上手感。
> 补丁评估里把它算在「需自行实现（中）」，这一轮没有改变那个判断。

## 3. 输入：软键盘起得来，字进得去，但**焦点后的第一个字符会丢**

- 点一下输入框，`dumpsys input_method` 报 `mInputShown=true`，软键盘确实起来
  （当前输入法是搜狗，`com.sohu.inputmethod.sogou.xiaomi/.SogouIME`）；
- `adb shell input text "hello"` → 输入框里是 **`"ello"`**，`h` 没了；
- 隔一秒再发 `"world"` → 变成 **`"elloworld"`**，第二次**一个字都没丢**。

分两次发是刻意的：只发一次的话，「Slint 吞掉了获得焦点后的第一个字符」和
「软键盘刚起来时头一个字符没人接」长得一模一样。两次的结果把它们分开了——
**丢的只是获得焦点之后的第一个字符，不是每次注入都丢。**

**到底丢在哪一层还没定**，候选有三个：Slint 的 android-activity 文本输入在
焦点刚到时 InputConnection 还没建好；搜狗输入法吞掉；`input text` 的注入路径
与窗口焦点切换竞争。这一轮没有继续追——要定位它得换一个干净的 IME（或者直接
用 `am instrument` 注入），属于下一轮的事。**但它必须记下来**：真机上用户点进
输入框打的第一个字如果会丢，那是个会被当成「这 App 有毛病」的缺陷。

**Tab 焦点轮转与中文组字这一轮都没测。** 软键盘上没有 Tab 键；中文组字要在
候选栏上点选，`adb` 驱动不了那一层，`input text` 直接上屏走的不是组字路径。
这两项要么用带硬件键盘的设备，要么写一个测试用 IME，都不是改脚本能解决的。

## 4. 富文本：与 Windows 同结论，补一条

`StyledText` 六条全部解析成功（0.09ms），行内图片仍然直接报
`Markdown images are not supported`——和 Windows 一字不差，这是 Slint 的能力
边界，不是平台差异。

补一条 Windows 上没留意的：**系统 emoji 画得出来**（截图里 😂🎉 正常上色）。
所以「图文混排」要自己排的是**站内表情图**（`[doge]` 这类 webp），不包括
Unicode emoji。

## 5. 实验台这一侧的坑（下一个人省得再踩）

| 坑 | 症状 | 真因 |
|---|---|---|
| **上一轮的 spike 还活着** | 第一次冒烟测试时 logcat 里一直在打 `STATUS t=66.4 audio-pts=…` | SPIKE-002（#9）的 mpv 探针还在后台解码放音，用户自己的 KiliKili 也占着 442MiB。**量帧耗时之前必须 `am force-stop` 掉所有相关进程**，否则量的是两个程序抢 GPU 的结果 |
| 记录标题写成「· Linux」 | 第一轮记录的标题与「平台」两栏都是 Linux | `platform_label()` 的 else 分支吃掉了 Android——它的内核确实是 Linux。已把 `target_os = "android"` 排到最前面 |
| 窗口尺寸「1280x800 逻辑 / 1116x2480 物理 / scale 2.75」 | 三个数除不出来 | 逻辑尺寸写的是**命令行参数**，而 Android 上窗口大小是屏幕说了算。改成按实际物理尺寸除以缩放算 |
| `adb pull a b` | 第二轮的截图落在 `shots/shots/`，上一轮的还留在 `shots/` | b 已存在时 pull 会把 a 塞进 b **里面**。两轮证据混在一起，看文件名完全分不出来。清场时要 `rm -rf` 目录，不能只删 `*.png` |
| 无障碍那行写着「没有抓到 UIA 树（未启用外部驱动？）」 | 读起来像工具坏了 | 在 Android 上这棵树**本来就该从外面抓**。把「失败」和「分工如此」混成一句，下一个人会去修一个不存在的 bug |
| 记录里 `rustc` / `commit` 取不到 | 两栏都是 `<取不到>` | 设备上没有 rustc 也没有 git。这两栏要么由宿主机补，要么编译期固化——**目前是缺的**，下一轮补 |
| 熄屏 | 跑到一半画面没了 | 默认 60 秒熄屏，而一轮要 50 秒。跑之前 `svc power stayon true` |

## 6. 对 #10 与 RFC 的影响

- **R9 要再改一次。** 上一轮刚把「无障碍」从风险里拿掉（Windows 上 UIA 读得
  出来），这一轮发现 Android 上**根本没接**。无障碍重新成为风险，而且是四件
  「做不到」里唯一**应用层绕不过去**的那一件。
- 长列表与触控**不是**风险，两个平台都有余量，Android 的余量小得多但够用。
- 富文本的结论不变：行内图片要自己排，而 Slint 不给文本测量 API。

## 7. #10 还差什么

- **macOS 那一轮**（#8，等机器）：VoiceOver 是第三套无障碍 API，Windows 能读
  不代表 mac 能读——Android 这一轮已经证明「平台之间不能互相推断」；
- **下拉刷新**：要先自己写一个控件才谈得上验；
- **中文组字与 Tab 轮转**：要带硬件键盘的设备或一个测试用 IME；
- **首字符丢失定位**：换干净 IME 或改用 `am instrument` 注入，分清是 Slint、
  输入法还是注入路径；
- **无障碍这条线的下一步**：确认 Slint 上游有没有在做 Android 的
  `AccessibilityNodeProvider`，没有的话评估自己实现的代价——这决定 Phase 2
  能不能在 Android 上交付。
