# slint-mpv-spike

M0 的实验台：**一个 Slint 窗口，底下是 libmpv 画的视频，上面是真实弹幕层**，
再加一层 HUD 报帧率、帧耗时、CPU、RSS。对应 issue #8（SPIKE-001，macOS）与
#9（SPIKE-002，Android）里「视频表面 + 弹幕覆盖」那一半。

它**不是产品代码**，自成一个 cargo workspace（理由写在 `Cargo.toml` 开头），
`rust.yml` 不会碰它。Go/No-Go 结论下来之后，它要么长成 Phase 2 的
`NativeMpvAdapter`，要么连同结论一起删掉。

## 它到底在验什么

一句话：**换掉 Flutter 之后，播放这条路还成不成立。**拆成能被证伪的几条：

| 问题 | 怎么验 | 证据 |
|---|---|---|
| Slint 能不能把 mpv 的画面垫在 UI 底下 | 透明背景的 Window + `BeforeRendering` 里 `mpv_render_context_render` 到同一个 FBO | 截图 + 帧率 |
| 几百条弹幕叠上去还剩多少帧 | 真实 `DanmakuSession` 取桶 → Rust 算轨迹 → Slint 画 | p50/p95/p99 帧耗时 |
| ADR-003 的 PlayerPort 契约在原生后端下成不成立 | libmpv 包成 `PlayerPort`，seek/暂停/代际全走契约 | 验证项表 |
| 硬解有没有真的开 | `hwdec-current` | 记录里的「hwdec 实际」 |

弹幕**形状**取自 `rust/crates/bili-api/tests/fixtures/danmaku_seg/` 的 golden
fixture（真实响应脱敏而来），**密度**由 `--danmaku-rate` 合成——fixture 是为
解码正确性挑的，只有几条，压不动渲染。记录里会写明这一点，别把合成密度当成
线上弹幕量。

## 跑之前要有 libmpv

构建期**不链接** libmpv，运行期 dlopen，所以不需要 dev 包、pkg-config 或
bindgen：

- macOS：`brew install mpv`（会带上 `libmpv.2.dylib`）
- Windows：取 media_kit 给 app 用的那一份就行。
  `~/AppData/Local/Pub/Cache/git/media-kit-*/libs/windows/media_kit_libs_windows_video/windows/CMakeLists.txt`
  里写着 URL 与 MD5；下下来用 `tar -xf`（Windows 自带的 bsdtar 认 7z）解开，
  得到 `libmpv-2.dll`。
- Linux：`libmpv.so.2`（发行版的 `libmpv2` / `mpv-libs` 包）。

找不到时用 `--mpv-lib <路径>` 或 `KILIKILI_MPV_LIB` 指定。**加载失败不会中断
实验**：它会退成 no-video 模式，照样给出弹幕层自己的帧率，记录里写明这次没有
视频。

## 跑

脚本化（实验记录就靠它可比，跑完自动退出）：

```sh
cd spikes/slint-mpv
cargo run --release -- \
    --video <视频 URL 或文件> --audio <音频 URL>  \
    --scripted --seek-to 30000 --danmaku-rate 30 \
    --shots ../../target/spike-shots \
    --record ../../docs/m0/spikes/<平台>-<日期>.md
```

手动（自己按键看）：

```sh
cargo run --release -- --video <...>
```

| 键 | 作用 |
|---|---|
| 空格 | 暂停 / 继续 |
| ← → | ±5 秒 |
| F | 全屏 |
| S | 截图 |
| Q | 退出 |

常用开关：

- `--flip-y`：视频上下颠倒时加上。**不同平台/驱动给的 FBO 方向不一样**，
  Windows + femtovg、Android 真机、macOS（M4）上实测都需要它。
- `--loop-file`：样片比脚本短时必须加，否则后半段测的是「已经播完」。
- `--hwdec`：默认 `auto-safe`；要对照软解就 `--hwdec no`。
- `--danmaku-rate 0`：只放 fixture 自带的那几条。
- `--max-active`：同屏上限，默认 200。

没有 `--video` 就是 no-video 模式：只跑弹幕层，用来拿**弹幕自己的帧率基线**。
有视频那次掉帧时，只有和它对照才知道是文本渲染还是视频合成的锅。

## Android（SPIKE-002 / issue #9）

Android 上入口不是 `main` 而是 cdylib 导出的 `android_main`（`src/lib.rs` +
`src/android_probe.rs`），用 cargo-apk 打包。

探针分两轮做完了 #9 的全部范围：

- 第一轮（PR #185）：Slint 的渲染回调给不给 OpenGL 上下文、libmpv 能不能把
  视频画进同一个 framebuffer。这是其余所有项的前提。
- 第二轮：surface 生命周期、旋转、后台音频、PiP、触摸分发，外加
  `spikes/media3-probe` 那个 Media3 回退对照。

结论与证据都在 `docs/m0/spikes/SPIKE-002-android.md`。

为什么桌面的结论不能直接推广：**Android 上 Slint 用 skia，不是桌面那条
femtovg**（slint 1.19 把 femtovg 挂在 `cfg(not(target_os = "android"))` 下），
而 skia 有 OpenGL / wgpu / software 三种 surface，只有 OpenGL 那种会交出
`GraphicsAPI::NativeOpenGL`。

### 取 libmpv.so（不进仓库）

用 app 已经在用的那一份（media_kit 的 Android 依赖，带校验和）：

```sh
curl -L -o /tmp/mpv.jar   https://github.com/My-Responsitories/libmpv-android-video-build/releases/download/20260906/default-arm64-v8a.jar
sha256sum /tmp/mpv.jar   # 98df6410375cc7a4be7e6eff56f9ccd88fa52678973cc23bcf7e934ab8c8682d
unzip -j /tmp/mpv.jar lib/arm64-v8a/libmpv.so -d android-libs/arm64-v8a/
```

URL 与 sha256 的出处是
`libs/android/media_kit_libs_android_video/android/build.gradle`，换 media_kit
版本时跟着它改。`android-libs/` 已被 `.gitignore`——14MB 的二进制不该进仓库。

### 构建与运行

```sh
cargo install cargo-apk

export JAVA_HOME=<JDK>              # slint 的 android 后端 build.rs 要 javac
export ANDROID_HOME=<SDK>
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/30.0.16248370"
export ANDROID_PLATFORM=android-37.0   # 不设的话 build.rs 自己猜，猜错就编不过

cargo apk build --release --lib
./android-repack.sh                 # 换 manifest（PiP 要用），见下
adb install -r target/release/apk/spike-pip.apk
```

**PiP 那一项必须走 `android-repack.sh`。** `android:supportsPictureInPicture`
是系统的硬门槛，没有它 `enterPictureInPictureMode` 直接抛异常；而 cargo-apk
0.10 用的 ndk-build 0.10 的 manifest 模型里没有这个字段，Cargo.toml 怎么写都
生成不出来。脚本的做法是打完包把 manifest 换掉（`aapt2 link` 编译一份新的，
替换进 APK，再 `zipalign` + `apksigner`）。理由写在脚本开头。

`./android-repack.sh --recreate` 打的是**对照包**：去掉 `configChanges`，
于是转屏时系统会重建 activity。两种行为都要测过，才知道 Phase 2 该选哪种。

媒体文件放应用自己的外部数据目录（不需要任何存储权限）：

```sh
D=/sdcard/Android/data/io.github.kakacat.kilikili.spike/files
adb push probe.mp4 $D/probe.mp4
python make-probe-audio.py probe-audio.wav && adb push probe-audio.wav $D/
```

**音轨是单独一条。** 手上那份 probe.mp4 没有音频轨，拿它测「后台还出不出声」
等于拿默片证明音响好不好。`make-probe-audio.py` 用标准库合成一条带节拍的
WAV（本机没有 ffmpeg），探针看到它就按 `MediaSource::Dash` 分轨开——跟真实的
B 站取流是同一条路径。

没有 probe.mp4 时探针**照样跑**，只验 GL 上下文那一半，mpv 那一半记 BLOCKED。

### 跑完整的一轮

```sh
./android-drive.sh [输出目录]        # 默认 target/android-evidence
```

**放网络流（#210 补测）**：往 `/sdcard/Android/data/io.github.kakacat.kilikili.spike/files/`
推一份 `source.txt`（`key=value` 每行一个：`video`、`audio`、`ua`、`referer`，可选
`video2`、`audio2`），探针就不用 `probe.mp4`。里面是带签名的取流地址：**只临时推到设备，跑完删掉，
不进仓库**。额外命令：`switch`（从当前位置换到 `video2`/`audio2`，即切清晰度）；seek、倍速用
`prop time-pos 60`、`prop speed 2`。STATUS 行多了 `ui_fps`、mpv 渲染耗时、`cpu`、`rss`、`avsync`。
注意 `cmd.txt` 是 4Hz 轮询、读完清空，两条命令间隔要大于 250ms，否则前一条会被覆盖。

它把 #9 剩下那五项按固定顺序跑一遍：触摸分发 → 旋转 → PiP → 后台音频（原样）
→ 后台音频（带后台策略），每一步都留系统截图、帧缓冲回读与 `dumpsys` 片段，
最后把 logcat 和设备上的 PNG 一起拉回来。**手机必须解着锁**——锁屏时 activity
不可见，Slint 不渲染，什么都测不到。

### 探针的命令通道

驱动脚本靠往应用外部数据目录里写一行命令来指挥探针（NativeActivity 收不到
intent，而 `adb shell` 本来就写得进那个目录）：

```sh
adb shell "echo 'pip' > $D/cmd.txt"
```

| 命令 | 作用 |
|---|---|
| `note <文字>` | 往 logcat 里打一个阶段标记，事后对时间轴 |
| `pip` / `pip noparams` | 进画中画（带宽高比参数 / 无参的老接口） |
| `pip-auto on\|off` | API 31 的自动进入：上滑回桌面时直接变小窗 |
| `bg` / `fg` | 下发 `SetLifecycle`，手动切后台策略 |
| `auto-lifecycle on\|off` | 让探针在 Pause/重建时自动走上面那条策略 |
| `reopen` | 重新 `Open + Play`（surface 重建之后救画面唯一有效的办法） |
| `prop <名> <值>` | 直接写 mpv 属性，排查用（`hwdec`、`vid`…） |
| `shot <名>` / `shot-pair <名>` | 回读一帧 / 回读一对（mpv 画完、整帧合成完各一张） |
| `quit` | 退出进程 |

`shot-pair` 是分层定位用的：两张一比就知道画面是在 mpv 那一层没出来，还是被
Slint 这一层盖掉了。

### 看结论

```sh
adb logcat -s kk-spike
```

结论行都带固定前缀，驱动脚本按前缀取：

| 前缀 | 含义 |
|---|---|
| `EV` | android-activity 的 `MainEvent`（生命周期第一手证据） |
| `STATUS` | 每秒一条的播放状态，由**独立线程**打——UI 停了它还在，后台音频靠它作证 |
| `SURFACE` | setup / teardown 与 render context 的生死 |
| `SIZE` / `BUTTONS` | 窗口尺寸与按钮的物理坐标（驱动脚本点按钮要用） |
| `TOUCH` / `PIP` / `SHOT` / `CMD` | 同名的那几项 |

同样的内容也会显示在屏幕 HUD 上——前者能脚本化取，后者能截图留证，
实验记录里两样都要。

## 插帧 spike（#218）

同一个 package 里的第二个实验：mpv 的每张新源帧先画进我们自己的纹理环，按
`target_time` 排程，做块匹配 + 运动补偿后再画进 Slint 的 FBO。结论与数字见
[`docs/m0/spikes/2026-11-24-interp-218.md`](../../docs/m0/spikes/2026-11-24-interp-218.md)。

- 代码在 `src/interp/`：`gpu.rs`（纹理、着色器调度、计时、回读）、`sched.rs`（源帧环与
  选帧，有单测）、`shaders.rs`（全部 GLSL，只用 fragment shader）、`app.rs`（参数、播放
  循环、实验室模式、记录）。`gpu.rs` / `sched.rs` / `shaders.rs` 不依赖 clap、Slint 或 mpv，
  `spikes/slint-ios` 用 `#[path]` 原样引用。
- 桌面：`cargo run --release --bin interp -- --help`。`--mode direct|hold|blend|mc`；
  `--lab <PNG 目录>` 是实验室模式（用 60 fps 真值的第 k−1、k+1 帧插出第 k 帧）。
- Android：`cargo apk build --release --lib --features interp`（换掉探针的 `android_main`），
  再用 `./interp-android.sh [--push <文件>]... -- <同桌面的参数>` 驱动。参数写进应用外部数据
  目录的 `args.txt`，记录逐行进 logcat（tag `kk-interp`），`--record` 的文件拉回
  `android-out/`。

## 已知偏差（看数字之前先读这个）

- **文本宽度是估算的**（CJK 按一个字宽、其余 0.55），没问字体引擎。只影响
  分轨紧凑度，不影响帧率结论。
- **mode 7 / 9（高级弹幕、BAS）不渲染**，只计数。它们要的是一个小动画解释器，
  基线也走单独路径；放进来只会污染测量。
- **合并角标**直接拼进文本（`内容 ×7`），不是基线那种独立角标。
- 轨道只保证「前一条的尾巴进屏后」才放下一条，**挡不住短弹幕追上长弹幕**，
  基线也挡不住。
- 截图是在事件循环里同步 `glReadPixels` + 压缩，**会把那一帧堵住**；帧表会
  跳过紧随其后的那个间隔，否则报告的 p99 量的是 PNG 压缩速度。
- 每帧都让 mpv 画一次，没有按 `mpv_render_context_update` 跳过：Slint 在背景
  透明时不清屏，跳过那一帧会留下上一帧弹幕的残影（见 `MpvRender::has_new_frame`
  的注释）。

## 踩过的坑（Phase 2 的适配器会再踩一遍）

1. **同步的 `mpv_command` 不能从渲染线程发。** 核心等渲染线程、渲染线程等
   核心，窗口冻死。手动回退 seek 时稳定复现，脚本化跑不出来。现在一律走
   `mpv_command_async` / `mpv_set_property_async`。
2. **render context 要在 `loadfile` 之前建好**，否则有声音、有进度、永远黑屏，
   而且不报任何错。现在开播统一等渲染侧就绪（`RenderState::is_ready`）。
3. **`mpv_render_context_render` 默认阻塞到该帧的目标显示时刻**，不关掉的话
   整个 UI 被片源帧率拖住（30fps 的片子把窗口锁死在 31.6fps）。现在显式传
   `BLOCK_FOR_TARGET_TIME = 0`。

## 结构

```
src/main.rs      窗口、16ms tick、脚本执行、证据汇总
src/mpv/sys.rs   libmpv 的最小 FFI 面（运行期 dlopen）
src/mpv/mod.rs   NativeMpvAdapter 原型：PlayerPort + render context
src/fake.rs      不带视频的同契约后端
src/overlay.rs   弹幕分轨与轨迹（纯逻辑，有单测）
src/source.rs    golden fixture → DanmakuSession
ui/spike.slint   只负责画

../spike-harness/  帧耗时分位数与 CPU/RSS、GL 截图、实验记录（含路径脱敏）。
                   原先是本工程的 src/metrics.rs、src/gl.rs、src/record.rs，
                   SPIKE-003（`spikes/slint-ui`）要量同样的东西时抽出来共用——
                   复制一份的话两边的记录格式迟早分叉，而跨平台逐行对照正是
                   这套模板存在的理由

src/lib.rs           Android 的 cdylib（桌面上是空的）
src/android_probe.rs SPIKE-002 的探针：GL 上下文、surface 生命周期、旋转、
                     后台音频、PiP、触摸分发
android-repack.sh    换 manifest（PiP 的 supportsPictureInPicture）
android-drive.sh     整轮实验的驱动脚本
make-probe-audio.py  合成一条带节拍的 WAV（后台音频要有声音才测得出来）
```

Media3 的回退对照是另一个工程：`spikes/media3-probe`（Kotlin + Gradle）。

`cargo test` 跑的是其中的纯逻辑部分（25 条），不需要 libmpv，也不开窗口。
另外 10 条随记录与分位数一起搬到了 `../spike-harness`，在那边跑。
