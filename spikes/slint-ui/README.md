# slint-ui — SPIKE-003 的实验台（issue #10）

一个 Slint 窗口，四个场景，跑一次产出一份实验记录：

| 场景 | 要回答的问题 |
|---|---|
| 长列表 | 两万条评论摆进 `ListView`，虚拟化成不成立、滚动一帧要画多久 |
| 富文本 | 同一条评论的三种画法并排：纯 `Text` / `StyledText` / 手写排布 |
| 输入与焦点 | Tab 轮不轮转、外部按键进不进得去、中文输入法组得了字吗 |
| 无障碍 | 屏幕阅读器能从这棵树上读到什么 |

记录模板、分位数、CPU/RSS 采样与截图都来自 [`../spike-harness`](../spike-harness)，
和 SPIKE-001/002 共用同一份——两份记录格式一旦分叉，跨平台就没法逐行对照。

## 跑

```sh
cargo build --release
./target/release/slint-ui-spike --scripted --rows 20000 \
    --record ../../docs/m0/spikes/2026-11-22-slint-ui-windows.md \
    --shots /tmp/shots
```

**两条都不是洁癖**：

- **先 `cargo build --release` 再跑**。`cargo test` 建的是测试壳，不会刷新
  `target/release/` 下的 exe——曾经整整一轮跑的是旧二进制。
- **直接跑 exe，不要用 `cargo run`**。2026-11-22 复跑时 2/2 复现：经
  `cargo run` 跑出来的那一轮，按键 / Tab / IME 三项全 FAIL（焦点序列只剩
  `[1]`、输入框全空），而外部驱动照常报 `activate=true`。机制没查清，但
  **失败的样子像结论**——记录上会写着「Slint 收不到键盘输入」。详见那份
  notes 的 §5。

`--scripted` 会跑完整条时间线并自动退出；不加就是手动模式，自己点。
其他常用参数：`--rows`（列表条数）、`--keys`（英文那段打什么）、
`--ime-keys`（拼音）、`--scale`（覆盖 HiDPI 缩放）、`--no-driver`（不起外部
驱动，于是按键 / IME / 最小化 / 无障碍四项变 NOT TESTED）。

手动模式下 `--scene <0-3>` 直接停在某个场景（0 长列表 / 1 富文本 / 2 输入 /
3 无障碍），给屏幕阅读器实读用——脚本化一轮 40 秒切完四个场景，人跟不上。

## 在 macOS 上跑

外部驱动只有 Windows 版，所以 macOS 按 `--no-driver` 跑，Tab / IME / 最小化
三项是 NOT TESTED。无障碍走另一条路：`mac/vo-drive.sh` 驱动真实的旁白
（VoiceOver）并读回它念的话，`mac/ax-dump.swift` 抓 AX 树。四个必须先开的
开关写在 `vo-drive.sh` 开头，缺一个都静默失败。结论见
`docs/m0/spikes/2026-11-23-slint-ui-macos-notes.md`。

另有一个对照程序 `frame-floor`，用来量**这台机器上 Slint 的呈现上限**：

```sh
cargo run --release --bin frame-floor -- --mode scroll --rows 20000
```

主实验台的帧数要先减掉这一层才算数。查「滚动只有 7fps」时就是它把
「Slint 画不动」和「根本没触发重绘」分开的。

## 在 Android 真机上跑

同一个 `app::run`，不是另写的探针——两份记录要能逐行对照，流程就不能各写
一份。Android 的入口是 cdylib 里导出的 `android_main`（`src/lib.rs`），它自己
拼一份与上面那行命令等价的 `Args`。

```sh
export JAVA_HOME=<Android Studio 的 jbr>
export ANDROID_HOME=<SDK>   ANDROID_NDK_ROOT=$ANDROID_HOME/ndk/<版本>
cargo apk build --release --lib
adb install -r target/release/apk/slint_ui_spike.apk
./android-drive.sh target/android-evidence
```

三件**跑之前必须做**的事，少一件就得到一份看不出错的脏数据：

1. **把别的 spike 和 App 停掉**：`adb shell am force-stop <包名>`。SPIKE-002
   的 mpv 探针会在后台继续解码放音，量到的就是两个程序抢 GPU 的结果；
2. **解锁并常亮**：`adb shell svc power stayon true`。锁屏时 activity 不可见，
   Slint 不渲染，看着像卡死；默认 60 秒熄屏而一轮要 50 秒；
3. 驱动脚本靠 logcat 里的 `STEP` 行对时间轴，**别改成按绝对时间 sleep**：
   真机冷启动能差好几秒，错位不会报错，只会让「对着列表发按键」这种事
   静静发生。

外部驱动在**宿主机**上（adb），不在进程里，所以 Android 这一轮按
`--no-driver` 跑，按键 / 输入法 / 无障碍三项由 `android-drive.sh` 从外面做，
结果追加在记录末尾的「宿主机侧证据」一节。

`--touch-ms` 是 Android 专用的：它在列表场景末尾留一段**探针自己不滚**的空窗，
滚动全部由 swipe 驱动，于是「触控惯性」有自己的帧耗时分段。桌面保持 0，脚本
与 2026-11-22 那一轮逐字相同。

## 数据是真的，条数是假的

列表里每一行都走**生产管线**：pb 字节 → `bili_api::grpc_reply::project_main_list`
→ `ReplyEntity` → `bili_domain::reply_richtext::segment`。用假结构问不出
「真实评论的段数与配图能不能撑住 60fps」。**只有条数是合成的**，记录里写明。

## 外部驱动（Windows 是 `win/`，Android 是 `android-drive.sh`）

IME 组字、Tab 轮转、最小化再回来、无障碍树这四项，**只能从窗口外面做**：
程序自己 `set_text` 等于把被测的那一段跳过去了。所以起 `powershell.exe` 跑
`win/drive.ps1`（按键 / 切输入法 / 最小化还原）和 `win/uia-dump.ps1`（抓 UIA
树）。一律异步起、下一帧再收——同步等于自己给自己造一个长帧，然后把它记成
「Slint 卡了」。

**`win/*.ps1` 必须存成带 BOM 的 UTF-8。** 没有 BOM 时 `powershell.exe` 5.1 按
本机 ANSI 代码页读，中文注释被拆成半个字符，整份脚本在解析阶段就失败，
而报出来的是 `[string]$Keys = '',` 这类正常行「缺少表达式」。失败是静默的：
spike 这边只看到非零退出码，记录里按键 / IME / UIA 一起变 FAIL，**看起来像
Slint 收不到输入**。`Driver::new` 会在开跑前检查 BOM 并在记录里喊一声。

反过来，`uia-dump.ps1` **写出去的 JSON 不要 BOM**：`[Text.Encoding]::UTF8`
这个现成实例是带 BOM 的，serde_json 读到就报
`expected value at line 1 column 1`。源码给 PowerShell 读、JSON 给 Rust 读，
两边的规矩不是一回事。

## 量的是哪个数

记录里每个分段都有两行：

- **渲染耗时**——`BeforeRendering` 到 `AfterRendering`，即「画一帧要多久」。
  **结论看它。**
- **帧间隔**——两帧之间隔了多久，即「多久画一帧」。界面没变时 Slint 不重绘
  是正确行为，帧间隔会把它记成一个 500ms 的长帧。

再加一行 16ms 定时器的次数与间隔做对照：定时器每次都改 HUD 文本，它跑了
多少次，本来就该有多少次重绘的机会。**帧间隔大而渲染耗时小，说明是没触发
重绘，不是画得慢**——只有一组数的时候这两种情况分不开，而它们的结论正好
相反。

## 为什么自成一个 workspace

同 `slint-mpv`：`rust.yml` 按 `rust/**` 触发，spike 要链 GPU 栈、要开窗口、
要调系统的无障碍接口，进那个 workspace 等于给所有 CI 作业加一堆它们装不了
的依赖。它是证据产出工具，Go/No-Go 结论下来之后可以整个删掉。
