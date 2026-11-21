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
  Windows + femtovg 上实测需要它，macOS 上是什么样要跑了才知道。
- `--loop-file`：样片比脚本短时必须加，否则后半段测的是「已经播完」。
- `--hwdec`：默认 `auto-safe`；要对照软解就 `--hwdec no`。
- `--danmaku-rate 0`：只放 fixture 自带的那几条。
- `--max-active`：同屏上限，默认 200。

没有 `--video` 就是 no-video 模式：只跑弹幕层，用来拿**弹幕自己的帧率基线**。
有视频那次掉帧时，只有和它对照才知道是文本渲染还是视频合成的锅。

## Android（SPIKE-002 / issue #9）

Android 上入口不是 `main` 而是 cdylib 导出的 `android_main`（`src/lib.rs` +
`src/android_probe.rs`），用 cargo-apk 打包。**这一轮只回答一个问题**：Slint
的渲染回调给不给 OpenGL 上下文、libmpv 能不能把视频画进同一个 framebuffer。
旋转、后台音频、PiP、Media3 对照都不做。

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

cargo apk run --release --lib
```

媒体文件放应用自己的外部数据目录（不需要任何存储权限）：

```sh
adb push probe.mp4 /sdcard/Android/data/io.github.kakacat.kilikili.spike/files/probe.mp4
```

没有这个文件时探针**照样跑**，只验 GL 上下文那一半，mpv 那一半记 BLOCKED。

### 看结论

```sh
adb logcat -s kk-spike
```

同样的内容也会显示在屏幕 HUD 上——前者能脚本化取，后者能截图留证，
实验记录里两样都要。

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
src/metrics.rs   帧耗时分位数、CPU、RSS
src/gl.rs        三个 GL 函数 + 截图
src/record.rs    实验记录（含路径脱敏）
ui/spike.slint   只负责画

src/lib.rs           Android 的 cdylib（桌面上是空的）
src/android_probe.rs SPIKE-002 的探针：GL 上下文 + mpv 画进同一 FBO
```

`cargo test` 跑的是其中的纯逻辑部分（34 条），不需要 libmpv，也不开窗口。
