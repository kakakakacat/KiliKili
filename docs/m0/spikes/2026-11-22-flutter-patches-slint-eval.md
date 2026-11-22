# SPIKE-003 后半边：34 个 Flutter 补丁逐项评估（issue #10）

**这份不是实验台产出的记录，是人读 diff 写出来的评估**，没有跑任何程序。
它回答 #10 评论里提的那半边问题：`lib/scripts/*.patch` 代表上游 Flutter
满足不了、作者改了框架才做出来的交互，**换成 Slint 之后这些还在不在**。

- 前四项（长列表 / 富文本 / IME / 无障碍）的结论在
  [2026-11-22-slint-ui-windows.md](2026-11-22-slint-ui-windows.md) 与同名
  [-notes.md](2026-11-22-slint-ui-windows-notes.md)。
- 本文的结论**不替代实测**。读源码只能证明「API 不存在」，不能证明
  「存在的 API 跑起来够用」。需要跑的条目在 §6 单列。

## 证据等级

每一条都标了依据，只有三种：

| 标记 | 含义 |
|---|---|
| **源码** | 读 Slint 1.19.0 的 crate 源码得出，给到文件:行。结论是「有/没有这个 API」 |
| **实测** | 上一轮 Windows 跑出来的，见那两份记录 |
| **推断** | 以上都没有，按 Slint 的模型推的。**这类不能单独作为 No-Go 依据** |

Slint 源码路径都相对于本机 cargo registry 里的 `i-slint-*-1.19.0`，版本与
`spikes/slint-ui/Cargo.toml` 一致。

---

## 0. 先纠正三件事：34 不是 34 个独立问题

逐个读完之后，issue #10 评论里那句「34 项代表 34 个要重新实现的交互」
站不住。实际构成是：

### 0.1 「34」是 Android 那一轮的数字

`lib/scripts/patch.ps1` 按平台发补丁，数字对不上不是记错了：

| 平台 | Flutter SDK | material_ui | cupertino_ui | 仓库内 | 合计 |
|---|---|---|---|---|---|
| Android | 22 + 3 | 8 + 1 | — | — | **34** |
| iOS | 22 + 3 | 8 + 1 | 1 | 2 | 37 |
| Windows / macOS / Linux | 22 | 8 | — | — | **30** |

（这三个数与 [BASELINE §3.1](../BASELINE.md) 当初**实测**出来的 37 / 34 / 30
对得上——那边是数构建日志里 `applied` 的行数，这边是数 `patch.ps1` 的分支，
两条路算出同一个数，说明下面的逐项拆分没有漏掉或重复算补丁。）

22 个框架补丁是所有平台共有的；Android 多 `bottom_sheet_android`、
`scroll_view`、`navigator`，iOS 多 `scroll_view`、`bottom_sheet_ios_flutter`、
`navigator`，外加 `geetest_ios` 与 `bottom_sheet_ios_piliplus`（这两个打在
**仓库自己的文件**上，不是 SDK）。

### 0.2 9 个 material 补丁里有 8 个是框架补丁的同形复制

`material_ui` 是 Flutter material 的分叉包（本仓库依赖它），于是同一处改动
要打两遍：`modal_barrier_material` / `navigation_drawer` / `popup_menu` /
`fab` / `text_field` / `refresh_indicator` / `scaffold` /
`bottom_sheet_android` 与框架同名补丁**改的是同一段逻辑**
（`material/scaffold.patch` 多包了一层泛型，其余逐字相同）。

**只有 `material/tabs.patch` 是 material 独有的。**

去重之后，真正独立的问题是 **22 + 1 + 3 = 26** 个，不是 34。

### 0.3 这 26 个里，只有一半是「交互行为」

这是按**补丁本身是什么**分的（和 §1 按「Slint 能不能做到」分的四档是两根
不同的轴，别混着读）。读 diff 的时候这个区分最要紧，否则会把两类完全不同的
东西算成同一笔账：

| 类别 | 个数 | 是哪几个 | 换 Slint 之后 |
|---|---|---|---|
| **A. 只是把私有符号暴露出来**（`typedef X = _X;`、加个 getter） | 7 | 14、15、17、18、19、20、26 | 补丁本身消失，但它背后那个自写控件要重写 |
| **B. 真正改变框架的交互行为** | 13 | 2、4、6、7、10、11、12、13、16、21、22、23、24 | 行为不会自动存在，要逐个还债 |
| **C. 上游 bug 的临时修补**（`patch.ps1` 里标了 `TODO: remove`） | 3 | 1、3、9 | 跟着 Flutter 一起消失 |
| **D. 框架内部机制 / 自带特性的开关**（重建调度、Hero、路由回调） | 3 | 5、8、25 | Slint 没有这套机制 |

C 类里有个巧合值得记一笔：`modal_barrier`（菜单按下就关）和 `mouse_cursor`
（可点控件给手型）这两个「等上游修」的补丁，**在 Slint 上恰好就是默认行为**
（见 §3）。它们本来就不是能力差距，是上游把默认值选在了另一边。

A 类不是「少一笔账」，而是**账记在别处**：它们服务的那批自写控件在
`lib/common/widgets/flutter/` 下，**29 634 行**（`editable_text.dart` 7224、
`text_field/text_selection.dart` 3701、`vertical_slider.dart` 2680、
`vertical_tabs.dart` 2403…）。补丁只是这批代码伸进 SDK 的几根手指，
**真正要重写的是这 3 万行**。

> 按 34 个补丁估 Slint 的工作量会严重低估；按补丁背后的控件估才对得上。

### 0.4 于是分档要四档，不是三档

issue 里说的三档（原生支持 / 需自行实现 / 做不到）少了一档：**不适用**
——问题随 Flutter 一起消失。把 C、D 两类塞进「原生支持」会让 Slint 显得比
实际更强，塞进「需自行实现」又会凭空多出工作量。

---

## 1. 逐项表：Flutter SDK 的 22 个（所有平台）

| # | 补丁 | 它改了什么 | 档 | 依据 |
|---|---|---|---|---|
| 1 | `modal_barrier` | 菜单类路由的遮罩改成**按下**就关，而不是抬手才关；给 `ModalRoute` 加 `isMenu` | **原生支持** | 源码 |
| 2 | `text_selection` | 桌面端也用 `TapAndHorizontalDragGestureRecognizer`；触摸设备上选择手势不等 slop | **做不到** | 源码 |
| 3 | `mouse_cursor` | 非 Web 上可点击控件也给手型光标 | **原生支持** | 源码 |
| 4 | `image_anim` | 动图不再跟随「减少动画」系统偏好暂停 | **需自行实现**（小） | 源码 |
| 5 | `layout_builder` | 注释掉 `BuildScope` / `_scheduleRebuild` | **不适用** | — |
| 6 | `navigation_drawer` | 抽屉加 `flex`；destination 包一层 `Material` | **需自行实现**（中） | 源码 |
| 7 | `popup_menu` | 菜单项图标颜色跟随文本颜色 | **需自行实现**（小） | 源码 |
| 8 | `fab` | 去掉 FAB 的 `Hero` 转场 | **不适用** | 源码 |
| 9 | `null_safety_for_selectable_region` | 上游 bug（flutter#124078），`TODO: remove` | **不适用** | — |
| 10 | `selectable_region` | 七项选择能力，含**表情图参与选择与复制** | **做不到** | 源码＋实测 |
| 11 | `editable_text` | 选区高亮按行合并、`BoxWidthStyle.tight`、拖选时不回滚到光标 | **做不到**（视觉细节） | 源码 |
| 12 | `text_field` | `selectAllOnFocus` 默认 `false` | **原生支持** | 源码 |
| 13 | `scroll_position` | 只在位置真的变了才上报 `userScrollDirection` | **需自行实现**（小） | 源码 |
| 14 | `scrollable` | 暴露 `_shouldIgnorePointer`（A 类） | **需自行实现**（小） | 源码 |
| 15 | `scaffold` | 暴露 BottomSheet 内部（A 类） | **需自行实现**（大） | 源码 |
| 16 | `scrollable_gesture` | **嵌套滚动的手势消歧** + 可注入横向拖动识别器 | **做不到** | 源码 |
| 17 | `draggable_scrollable_sheet` | 暴露内部 + `initialScrollOffset`（A 类） | **需自行实现**（大，与 15 同一笔） | 源码 |
| 18 | `text` | 暴露 `RenderParagraph.textPainter`（A 类） | **做不到** | 源码＋实测 |
| 19 | `text_painter` | 暴露 `layoutCache`（A 类） | **做不到**（同 18） | 源码 |
| 20 | `sliver` | 暴露两个私有 sliver 控件（A 类） | **需自行实现**（中） | 源码 |
| 21 | `refresh_indicator` | `size == .zero` 时不画 | **需自行实现**（中，整个下拉刷新要自写） | 源码 |
| 22 | `double_tap_gesture` | **可配置的双击超时** + 暴露 `checkUp`（#143） | **需自行实现**（中） | 源码 |

### 平台专属的 3 个

| # | 补丁 | 平台 | 它改了什么 | 档 |
|---|---|---|---|---|
| 23 | `bottom_sheet_android` | Android | 底部弹窗的拖动识别器改用系统 touch slop | **做不到**（影响小） |
| 24 | `scroll_view` | Android / iOS | 屏幕**底部 30px 内不响应竖直拖动**，给系统手势让路 | **需自行实现**（中） |
| 25 | `navigator` | Android / iOS | 只在真的 pop 时才 `_afterNavigation` | **不适用** |

### material_ui 独有的 1 个

| # | 补丁 | 它改了什么 | 档 |
|---|---|---|---|
| 26 | `material/tabs` | 暴露 `TabBarState` / `IndicatorPainter` / `TabBarLayoutCallback`，让 `TabBarView` 能换方向、能注入横向拖动识别器；并短路 `tabAlignment` 断言 | **需自行实现**（中-大） |

---

### 档位统计

26 项合起来：**原生支持 3**（1、3、12）、**不适用 4**（5、8、9、25）、
**需自行实现 12**（4、6、7、13、14、15、17、20、21、22、24、26）、
**做不到 7**（2、10、11、16、18、19、23）。

---

## 2. 七个「做不到」归成四件事：它们才是 Go/No-Go 的内容

「做不到」在这里的定义是：**以 Slint 1.19 公开的能力拿不到等价行为**。
七项里 #23（用系统 touch slop）单独影响太小，只留在表里；其余六项归成下面
四件事。四件里有两件能靠「自己写一个更大的东西」绕过去，代价写在各条里；
绕不过去的只有改 Slint 上游。

### 2.1 跨元素的文本选择（#10、#2 的一部分）—— 最贵

Flutter 的 `SelectableRegion` 把**一棵子树**（昵称 + 正文 + 表情图 + 时间）
当成一个选择域，手指从昵称拖到正文末尾是一次连续选择，复制出来是一整段。
补丁在此之上还加了七件事，其中一件对本项目是刚需：**`WidgetSpan` 参与选择**
——`PlaceholderSpan.rawText` 加上新增的 `InlineSpan.toPlainTextV2()`，让选中
带表情的评论复制出来是 `[doge]` 而不是 `￼`。

Slint 这一侧：

- 可选中的元素只有两个：`TextInput` 和 `StyledText`
  （`i-slint-compiler/builtin_elements.rs:1294` 的 `selectable` 属性只长在
  `StyledTextItem` 上）。**普通 `Text` 没有这个属性**，不可选中。
- 选择 API 全是**元素自己的方法**：`select-all()` / `set-selection-offsets()`
  / `copy()`（同文件 1302–1314 行）。没有「选择域」这个概念，**相邻两个元素
  之间的选择连不起来**。
- `StyledText` 不支持行内图片（上一轮**实测**：`from_markdown` 原样报
  `Markdown images are not supported`），所以哪怕只在一个元素里，表情也不在
  可选中的内容里。

**代价**：要等价就得自己写一整层选择——命中测试（点到了哪个 run 的第几个
字）、跨元素的区间模型、选择手柄与它的钳制、自动滚动、剪贴板文本组装
（表情还原成 `[doge]`）。而命中测试需要字形级的位置信息，见 2.3：这是同一个
坑的两端。

**降级的下限**：把「整条评论」做成一个只读 `TextInput`，放弃行内表情（退回
`[doge]` 字面量）。那等于砍掉富文本。

### 2.2 嵌套滚动的手势消歧（#16、#24）

`scrollable_gesture.patch` 是 22 个里改动最大的一个（318 行），做三件事：

1. `NestedRawGestureDetector` / `NestedListener` / `_RenderPointerListener`
   ——让外层 `Scrollable` 的命中测试跟随 `ignoring()` 动态变化，于是竖直列表
   里嵌横向 `TabBarView` 时，外层不会把横向拖动吞掉；
2. 给 `PageView` / `TabBarView` 开 `horizontalDragGestureRecognizer` 注入口，
   仓库里塞进去的是 `lib/common/widgets/gesture/` 下那批自写识别器（侧滑返回
   与横向翻页共存要靠它）；
3. 开 `hitTestBehavior` / `scrollDirection`。

Slint 这一侧：

- 没有 gesture arena。`Flickable` 与子 `TouchArea` 的竞争写死在
  `i-slint-core/items/flickable.rs:483-500`：距离阈值 8px（桌面）/ 10px
  （触摸）、按住 500ms 判定、转发延迟 100ms、滚轮过滤 800ms。**全是常量，
  `.slint` 里没有对应属性，也没有注入口。**
- 嵌套 `Flickable` 的裁决同样落在这套常量上，**不能按方向裁决**——而 Flutter
  这边正是靠「横向识别器 vs 竖直识别器」在竞技场里分胜负的。
- `Flickable` 对外只有 `interactive` 和 `mouse-drag-pan-enabled` 两个开关
  （`builtin_elements.rs:1592-1606`），粒度是「整个滚动区开/关」。

**代价**：要等价就不能用 `Flickable`——得用 `TouchArea` 自己收指针事件、自己
做速度跟踪与惯性、自己实现虚拟化列表的偏移。那等于把上一轮**实测**出来的
「ListView 滚动 p95 0.9ms」整套换掉，结论要重跑。

**这是四条里唯一可能逼出「改 Slint 上游」的**：把那几个常量变成属性是小改
动，把「按方向裁决」做进去不是。

### 2.3 文本测量（#18、#19）

`text.patch` / `text_painter.patch` 看起来只是两个 getter，用处是
`lib/common/widgets/text_ellipsis/paragraph_ellipsis.dart` 与
`text_more/paragraph_more.dart`：**算出第 N 行末尾该截在哪**，好把「展开
全文」接在省略号后面而不是另起一行。`lib/utils/grid.dart` 与 `member_video`
也用它算布局。

Slint 这一侧：测量函数存在，但**不在应用够得着的地方**。
`i-slint-core/renderer.rs:68` 的 `Renderer::text_size()` 要一个
`RenderString` item 加一个 `ItemRc`——只能量「已经在元素树里的那个元素」，
不能问「这段字用这个字号排出来多宽」。`Text` / `TextInput` 的 `font-metrics`
给的是 ascent/descent 这类**字体级**的量，不是字符串的排版结果。

这和上一轮 notes §2 记的是同一件事：手写排布那一列只能按字号估字宽。

**代价**：引 `cosmic-text` / `rustybuzz` 自己量，然后**自己排版**——否则自己
量出来的断行位置与 Slint 实际画出来的不一致，省略号会错位。一旦自己排版，
`StyledText` 就用不上了，富文本那条路要整条自己走。

### 2.4 无障碍：Android 上根本没接（2026-11-22 真机补充）

这一条不在 34 个补丁里，是跑 Android 那一轮时撞出来的，但它和 §2 其余三条是
同一级别的东西，所以记在这里。

`uiautomator dump` 抓到的树里，这个应用的内容部分是**一个空的
`android.view.View`**：没有子节点、没有名字、没有角色。
`i-slint-backend-android-activity` 1.19.0 整个 crate 没有一处无障碍代码，
`cargo tree -i accesskit` 在 Android 的依赖图里一个都找不到（Windows 的图里
是完整的 `accesskit → accesskit_windows → accesskit_winit`）。

**`accessible-role` / `accessible-label` 在 `.slint` 里照写不误、编译不报错、
桌面上 UIA 读得出来，到了 Android 安静地什么都不做。**

和另外三条的区别是：**这一条绕不过去**——无障碍树必须由框架实现，应用层给不
出来。

**但 2026-11-22 查了上游之后，语气要改**：`slint-ui/slint` 上有一组 7 个 PR
（`[a11y n/7]`，作者 fidderr）正在给 Android 接 AccessKit，1/7 已合、6/7 就是
Android 那一块，按 Slint 约两个月一个小版本的节奏，最早出现在 1.20。所以这条
从「做不到」降为「**等上游，并且要自己验**」——7 个里只合了 1 个，PR 下面那份
第三方实测还列着 ComboBox、空 LineEdit 没标签等一串缺口。细节与风险（包括
`accessibility` 开着会拖慢大列表的 slint#3867）见
[2026-11-22-slint-ui-android-notes.md](2026-11-22-slint-ui-android-notes.md) §0.1、§0.2。

详见 [2026-11-22-slint-ui-android-notes.md](2026-11-22-slint-ui-android-notes.md) §0。

### 2.5 选区高亮的合并绘制（#11）

`editable_text.patch` 把选区高亮从「每个 TextBox 一个矩形」改成「同一行合并
成一个矩形，选到行尾时延伸到控件宽度」，就是中文换行时选区不再是一截一截的
毛边。Slint 的 `TextInput` 只给 `selection-background-color` 一个颜色属性，
绘制过程不可干预。

单独看是视觉细节，**不构成 No-Go**；记在这里是因为它和 2.1 共用一个结论：
Slint 的文本控件是黑盒，要动里面就只能不用它。

---

## 3. 「原生支持」的三项，以及为什么值得单列

| 补丁 | Slint 的对应 |
|---|---|
| `modal_barrier` | `PopupClosePolicy::CloseOnClickOutside` 的判定是 `!mouse_inside_popup() && pressed_event`（`i-slint-core/window.rs:1083`）——**按下就关，正是补丁要的行为**，Slint 默认如此 |
| `mouse_cursor` | `TouchArea.mouse-cursor` 是公开属性（`builtin_elements.rs:1367` 段），想给哪个控件什么光标就给 |
| `text_field` | `LineEdit` 聚焦时不全选，全选只出现在右键菜单的 Select All 里（`widgets/common/lineedit-base.slint:144`） |

值得单列是因为它们说明了一件事：**这三个补丁存在，是因为 Flutter 选了另一个
默认值，不是因为 Flutter 做不到。** 评估框架能力时，这类「默认值之争」不该
算进差距。真正的差距是 §2 那四条。

---

## 4. Phase 2 要排期的东西（按这份评估能点名的）

把「需自行实现」合并同类项之后，剩下这些是**确定要写**的：

| 要写的东西 | 对应补丁 | 量级 |
|---|---|---|
| 底部弹窗（拖拽、吸附、与列表滚动的交接、保留上次滚动位置） | 15、17、23 | 大 |
| 自己的排版器（断行、省略、图文混排、字宽测量） | 18、19、10 | 大 |
| Tab 指示器跟手 + 可横滑的 TabBarView | 26 | 中-大 |
| 下拉刷新 | 21 | 中 |
| 吸顶 / 可折叠头部（sliver 的替代） | 20 | 中 |
| 抽屉与水波纹 | 6 | 中 |
| 双击/单击互斥与可配超时 | 22 | 中 |
| 屏幕边缘的手势让位 | 24 | 中 |
| 滚动方向上报（上滑隐藏栏） | 13、14 | 小 |
| 动图的暂停（换成静帧） | 4 | 小 |
| 菜单项图标着色 | 7 | 小 |

Slint 这边**白捡**的：动图解码本身（GIF/APNG/WebP，
`i-slint-core/graphics/image/animated.rs`）、惯性与橡皮筋回弹
（`items/flickable.rs` 里按平台分的速度跟踪器与 `rubber_band`）、列表虚拟化
（上一轮实测）、无障碍属性与角色（上一轮实测 UIA 这一侧）。

关于动图还有一条要记：Slint 支持解码，但**没有暂停/播放控制**，每个动图
item 自带一个 Timer 按时间轴推进（`items/image/animation.rs`）。想在页面切走
时省电，只能把 `source` 换成静帧自己管。

---

## 5. 对 RFC 风险登记簿的修订

R9 原文是「Slint 列表虚拟化、富文本、IME、无障碍及移动端成熟度」。跑完前四项
加读完 34 个补丁之后，这句话里**排在前面的三项都不是风险**（虚拟化、IME、
无障碍在 Windows 这一侧都过了），真正的风险是它没提的另外三件：

1. 跨元素文本选择（含表情参与复制）；
2. 嵌套滚动的手势消歧；
3. 文本测量 / 自排版。

**2026-11-22 真机之后要加第 4 条：Android 上的无障碍**（见 §2.4）。它比前三条
更硬——前三条是「自己写一个更大的东西能绕过去」，这一条应用层绕不过去。

修订随本次改动落到
[`RFC-0001`](../../architecture/RFC-0001-rust-slint-migration.md) 的风险表。

---

## 6. 这份评估不能回答的（必须跑才算数）

读源码只证明了「API 不存在」。下面这些要么是「API 存在但不知道够不够用」，
要么是源码里看不出来的：

| 要验的 | 为什么读源码不够 |
|---|---|
| 自写选择层的**可行性**（命中测试能不能落到字形级） | `text_size` 不可达是确定的，但 Skia 渲染器在 `TextInput` 里怎么做命中测试、有没有能借的口子，要试 |
| 嵌套 `Flickable` 的实际表现 | 常量写死是确定的，但 8px / 500ms / 100ms 这组值在真实的「竖列表 + 横 Tab」里够不够用，只有搭一个双层列表才知道 |
| ~~触控惯性~~ **已验**（2026-11-22 真机）：手指驱动 122fps、渲染 p95 6.0ms，惯性是 `Flickable` 自带的。下拉刷新仍没验 | 下拉刷新要先自己写一个控件才谈得上手感 |
| `StyledText` + 自排版混用的代价 | 要真写一版才知道「一半交给 Slint、一半自己排」会不会打架 |
| ~~Android 那一轮~~ **已跑**（2026-11-22）；macOS 仍没跑 | Android 的结论见 §2.4 与那份 notes。IME 三个平台三套机制，Android 上的中文组字仍没驱动得了；VoiceOver 要 mac（#8） |

**建议**：如果只能再做一个实验，做表里第二行那个——双层嵌套滚动。它是四条
「做不到」里唯一**可能**靠改 Slint 几个常量解决的，验明白了能把 Phase 2 的
风险从「做不到」降到「要给上游提个 PR」。另外两条（选择层、排版器）不用再
验：代价是确定的，只是大。

---

## 7. #10 现在还差什么

本文补上了「34 个补丁逐项评估」。**#10 仍然不能关**，剩下的是：

- 触控惯性与下拉刷新（没有触摸屏）；
- VoiceOver（mac，#8）与 TalkBack（Android 真机）实读；
- macOS 上再跑一轮前四项（Android 已跑，见 §2.4）；
- §6 建议的那个嵌套滚动实验（可选，但它决定 §2.2 的最终档位）。
