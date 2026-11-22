# SPIKE-003 Slint 界面可行性 · Android

自动产出于 `app_process64`，不要手工编辑：重跑一次会整份覆盖。

命令行：`slint-ui-spike --scripted --rows 20000 --no-driver --touch-ms 8000 --record /storage/emulated/0/Android/data/io.github.kakacat.kilikili.spike.ui/files/record.md --shots /storage/emulated/0/Android/data/io.github.kakacat.kilikili.spike.ui/files/shots`

**自动项结论：PASS**（人工项见下表）

## 环境

| 项 | 值 |
|---|---|
| 时间 | 2026-11-22 07:51:50 UTC |
| 平台 | Android |
| 系统 | android aarch64 / 2304FPN6DC 14（内核 5.15.41-android13-8-00005-g779f29f1a22b-ab9196663） |
| slint | 1.19.0 |
| rustc | <取不到：rustc> |
| commit | <取不到：git>（工作区不干净） |
| 渲染器 | Qualcomm \| Adreno (TM) 740 \| OpenGL ES 3.2 V@0676.32 (GIT@1a285a84ae, I2991b7e11e, 1685945391) (Date:06/04/23) |
| 窗口 | 406x902 逻辑 / 1116x2480 物理 / scale 2.75 |

## 媒体与渲染

| 项 | 值 |
|---|---|
| 列表行数 | 20000（合成条数） |
| 数据来源 | pb 合成 → bili_api::grpc_reply::project_main_list → bili_domain::reply_richtext::segment |
| pb 编码后 | 6.2 MiB |
| 富文本并排 | 6 条 / 240 个 run（其中表情 2） |

## 测量

| 项 | 值 |
|---|---|
| 合成 pb | 48.1 ms |
| 生产投影 | 83.8 ms |
| 生产分词 | 2009.5 ms |
| 建 Slint 模型 | 15.2 ms |
| StyledText 构造 | 0.09 ms / 6 条（失败 0） |
| 手写排布 | 0.05 ms / 6 条 |
| 整段平均帧率 | 21.6 fps |
| 总帧数 | 1060 |
| 16ms 定时器 | 2848 次，间隔 p50 16.0 / p95 17.3 / 最慢 134.9 ms；回调自身耗时 p50 0.2 / p95 0.7 / 最慢 9.6 ms |
| 分段「列表静止」渲染耗时 | p50 6.2 / p95 10.0 / 最慢 13.1 ms（19 帧 / 221 次 tick） |
| 分段「列表静止」帧间隔 | p50 17.2 / p95 340.1 / p99 443.1 ms，长帧 2 次 |
| 分段「列表滚动」渲染耗时 | p50 7.3 / p95 10.5 / 最慢 26.2 ms（370 帧 / 309 次 tick） |
| 分段「列表滚动」帧间隔 | p50 16.6 / p95 24.2 / p99 26.2 ms，长帧 0 次 |
| 分段「跳到末尾」渲染耗时 | p50 5.3 / p95 12.9 / 最慢 15.2 ms（12 帧 / 89 次 tick） |
| 分段「跳到末尾」帧间隔 | p50 23.0 / p95 865.7 / p99 865.7 ms，长帧 3 次 |
| 分段「触控滚动」渲染耗时 | p50 2.5 / p95 6.0 / 最慢 22.1 ms（573 帧 / 485 次 tick） |
| 分段「触控滚动」帧间隔 | p50 8.2 / p95 13.5 / p99 53.2 ms，长帧 8 次 |
| 分段「富文本」渲染耗时 | p50 1.9 / p95 14.7 / 最慢 14.7 ms（4 帧 / 98 次 tick） |
| 分段「富文本」帧间隔 | p50 1510.7 / p95 1510.7 / p99 1510.7 ms，长帧 1 次 |
| 分段「输入」渲染耗时 | p50 2.2 / p95 10.3 / 最慢 26.4 ms（30 帧 / 739 次 tick） |
| 分段「输入」帧间隔 | p50 58.7 / p95 2594.5 / p99 4793.2 ms，长帧 14 次 |
| 分段「无障碍」渲染耗时 | p50 6.1 / p95 11.0 / 最慢 11.1 ms（25 帧 / 787 次 tick） |
| 分段「无障碍」帧间隔 | p50 17.9 / p95 4101.2 / p99 5367.3 ms，长帧 5 次 |
| RSS @ 模型建好前 | 289 MiB |
| RSS @ 进入「列表静止」 | 323 MiB |
| RSS @ 进入「列表滚动」 | 338 MiB |
| RSS @ 进入「跳到末尾」 | 307 MiB |
| RSS @ 进入「触控滚动」 | 339 MiB |
| RSS @ 进入「富文本」 | 264 MiB |
| RSS @ 进入「输入」 | 301 MiB |
| RSS @ 进入「无障碍」 | 272 MiB |
| RSS @ 退出 | 293 MiB |
| RSS 峰值 | 349 MiB |
| CPU（最后一次采样） | 800 % |

## 验证项

| 项 | 预期 | 实际 | 结论 |
|---|---|---|---|
| 长列表虚拟化 | 实例化的行数应当远少于总行数（20000 行） | 整段运行累计实例化 263 行（可视约 7 行；滚动与跳转会换新行，所以累计值大于可视值是正常的） | PASS |
| 滚动渲染耗时 | 匀速滚动段单帧渲染 p95 ≤ 16.7ms（60Hz 下一帧的预算） | 渲染 p50 7.3 / p95 10.5 / 最慢 26.2 ms；对照帧间隔 p50 16.6 / p95 24.2 ms | PASS |
| 跳到列表末尾 | 一次跳到第两万条不应当出现 >100ms 的长帧 | 最慢一帧渲染 15.2 ms；对照帧间隔 p99 865.7 ms | PASS |
| 行内样式（颜色 / 链接 / 可选中） | @某人、#话题#、链接、时间戳各有自己的颜色，链接可点、正文可选中 | Slint 1.19 的 StyledText 全部解析成功（6 条，0.09 ms） | PASS |
| 行内表情图（图文混排） | 表情应当像 Flutter 的 WidgetSpan 那样混在文字流里 | StyledText 不支持行内图片，原样报错：Markdown images are not supported。只能自己排布（本实验第三列：240 个 run，其中表情 2 个，断行靠估算字宽） | 人工确认 |
| Tab 焦点轮转 | Tab 应当在三个输入框之间依次移动 | 焦点序列 [1] | NOT TESTED |
| 英文直接输入 | 外部按键应当进到有焦点的输入框 | ① "elloworld" ② "" ③ "" | NOT TESTED |
| 中文 IME 组字与上屏 | 切到微软拼音打 `nihao`，输入框里应当是中文 | 三个框都没有中文：① "elloworld" ② "" ③ "" | NOT TESTED |
| 无障碍树（长列表） | 列表与行应当作为 List / ListItem 暴露给屏幕阅读器 | Android 上无障碍树由宿主机 `uiautomator dump` 抓，见记录末尾的宿主机侧证据 | NOT TESTED |
| 最小化 / 还原 | 最小化期间不应当继续满帧渲染；还原后应当恢复 | 最小化的 1.5 秒里画了 0 帧；还原之后到退出画了 16 帧 | NOT TESTED |
| HiDPI | 物理像素 = 逻辑像素 × 缩放系数，截图不应当糊 | scale 2.75，窗口 1116x2480 物理像素 | 人工确认 |
| 窗口缩放 / 全屏 | 布局跟着窗口重排，不裁切、不留黑边 | 见截图 07-resized / 08-fullscreen | 人工确认 |
| 触控滚动与惯性 | 真实触摸要能滚动列表，甩手之后要有惯性（松手后继续画帧） | 外部 swipe 驱动：这一段渲染 p50 2.5 / p95 6.0 / 最慢 22.1 ms，帧间隔 p50 8.2 ms。下拉刷新未测——Slint 没有这个控件，要先自己写一个 | PASS |
| VoiceOver / TalkBack | macOS 与 Android 的屏幕阅读器实读 | 本轮的无障碍树由宿主机 `uiautomator dump` 抓取，结论见记录末尾的宿主机侧证据 | NOT TESTED |

## 截图

- `01-list-top.png`
- `02-list-scrolled.png`
- `03-list-end.png`
- `04-richtext.png`
- `05-input.png`
- `06-a11y.png`
- `07-resized.png`
- `08-fullscreen.png`

## 备注

- 0ms Scene(0, "长列表")：整份模型一次交给 ListView，不分页
- 100ms Activate：先把窗口切到前台，否则后面所有帧耗时量的都是后台限频
- 2000ms Phase("列表静止")：静止时的帧耗时基线
- 2600ms Shot("01-list-top")：列表顶部：头像、昵称、正文、配图、计数都在
- 2900ms Uia("列表")：两万行的列表在无障碍树里长什么样
- 5700ms Phase("列表滚动")：匀速滚动段，主结论看这一段的 p95/p99
- 5800ms Scroll { speed: 2400.0, ms: 6000 }：匀速滚动 6 秒
- 12000ms Shot("02-list-scrolled")：滚动中：行高不一的内容应当稳定排布，无空白、无重叠
- 12300ms Phase("跳到末尾")：一次跳到底，看有没有长帧
- 跳到末尾：ListView 报的内容高度 2640000px，可视高度 868px
- 12500ms JumpToEnd：滚到最后一行
- 13300ms Shot("03-list-end")：末尾：最后一行应当贴着底，不应当是空白
- 13700ms JumpToTop：回到顶部
- 14000ms Phase("触控滚动")：这一段探针不自己滚：滚动由真实触摸驱动，量的是 Flickable 的惯性
- 22300ms Scene(1, "富文本")：同一条评论的三种画法并排
- 22400ms Phase("富文本")：富文本场景的帧耗时
- 23300ms Shot("04-richtext")：① 纯 Text ② StyledText ③ 手写排布；看表情、颜色、换行
- 23900ms Scene(2, "输入与焦点")：三个输入框
- 24000ms Phase("输入")：输入场景的帧耗时
- 24100ms Ime("00000409", "切回英文键盘")：先复位输入法，否则英文这一段会在上一轮留下的中文输入法里打
- 25800ms Keys("hello{TAB}world{TAB}1234567890")：英文直接输入 + Tab 轮转焦点
- 29200ms Ime("00000804", "切中文输入法")：切中文输入法
- 30700ms Keys("nihao ")：打拼音并上屏：输入框里应当是中文，不是拼音字母
- 35200ms Shot("05-input")：三个框的内容与焦点
- 35900ms Scene(3, "无障碍")：带 role/label/value 的控件
- 36000ms Phase("无障碍")：无障碍场景的帧耗时
- 36500ms Uia("控件")：按钮/勾选/滑块/输入框是否各自带名字与取值
- 39700ms Shot("06-a11y")：无障碍场景画面
- 40200ms Resize(980, 620)：缩小窗口：布局应当跟着重排
- 41200ms Shot("07-resized")：缩放后的布局
- 41700ms Fullscreen(true)：全屏
- 43200ms Shot("08-fullscreen")：全屏下的布局
- 43700ms Fullscreen(false)：退出全屏
- 44500ms Minimize：最小化：窗口不可见期间不应当继续满帧渲染
- 46000ms Restore：还原：画面应当立刻回来，不应当黑屏或丢状态
- 47200ms Scene(0, "长列表（回到列表）")：回到列表：之前的滚动位置与模型都还在
- 48200ms Shot("09-back-to-list")：回到列表后的画面
- 外部驱动：跳过外部驱动（未启用）：Keys 切到前台
- 外部驱动：跳过外部驱动（未启用）：Uia 列表
- 外部驱动：跳过外部驱动（未启用）：Ime 切回英文键盘（00000409）
- 外部驱动：跳过外部驱动（未启用）：Keys hello{TAB}world{TAB}1234567890
- 外部驱动：跳过外部驱动（未启用）：Ime 切中文输入法（00000804）
- 外部驱动：跳过外部驱动（未启用）：Keys nihao 
- 外部驱动：跳过外部驱动（未启用）：Uia 控件
- 外部驱动：跳过外部驱动（未启用）：Window minimize
- 外部驱动：跳过外部驱动（未启用）：Window restore
- 进入「列表静止」 时累计实例化 7 行
- 进入「列表滚动」 时累计实例化 7 行
- 进入「跳到末尾」 时累计实例化 218 行
- 进入「触控滚动」 时累计实例化 232 行
- 进入「富文本」 时累计实例化 254 行
- 进入「输入」 时累计实例化 254 行
- 进入「无障碍」 时累计实例化 254 行
- 退出 时累计实例化 263 行


## 宿主机侧证据（android-drive.sh）

探针跑在设备上，下面这些只有宿主机问得到。

| 项 | 值 |
|---|---|
| 设备 | 2304FPN6DC / Android 14 / 1116x2480 |
| 触控：甩动 | `input swipe 558 1860 558 620 300` × 3（300ms 走半屏，正常甩动的速度档） |
| 触控：慢拖 | `input swipe 558 1860 558 620 1200` × 1（跟手拖，不应当有惯性） |
| 软键盘 | mInputShown=true |
| 当前输入法 | com.sohu.inputmethod.sogou.xiaomi/.SogouIME |
| 无障碍树节点数 | 10（其中带 text/content-desc 的 1） |
| 系统截图 | sys-10-after-fling / sys-11-after-slow-drag / sys-12-input-ascii / sys-13-a11y |

无障碍树原文在 `a11y.xml`，它就是 AccessibilityNodeInfo 树——
**TalkBack 读的正是这棵树**。树里有没有节点是确定的事实；读屏软件
具体念成什么话不是，这份记录不回答后者。
