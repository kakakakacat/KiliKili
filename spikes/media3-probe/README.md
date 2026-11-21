# media3-probe

SPIKE-002（issue #9）里「Media3 SurfaceView/TextureView 回退对照」那一项。

它**不是**要证明 Media3 能放视频——那没有悬念。要量的是：**如果 Slint +
libmpv 那条路走不通，回退到 Media3 之后，#9 关心的那几件事各自变成什么样子，
以及代价是什么。**

对照的落点只有一个：`spikes/slint-mpv` 那个探针回答的那五项
（surface 生命周期、旋转、后台音频、PiP、触摸分发），在这边各要写多少代码、
行为差在哪儿。结论写在 `docs/m0/spikes/SPIKE-002-android.md`。

## 结构上的关键差别（这才是对照的重点）

| | Slint + libmpv | Media3 + SurfaceView |
|---|---|---|
| 视频去哪儿 | 画进 Slint **同一个 FBO** | 自己一层 surface，由 SurfaceFlinger 合成 |
| UI 怎么叠 | 本来就在同一个 framebuffer 里 | 另起一层（`setZOrderMediaOverlay` + 透明）或走视图树 |
| Activity | 系统的 `android.app.NativeActivity` | **自己的 Java/Kotlin 类** |
| PiP 回调 | 收不到（没有自己的 Activity） | `onPictureInPictureModeChanged` / `onUserLeaveHint` 都在 |
| 宿主可替换性 | 逻辑全在 Rust，换宿主不用重写 | 一层 Kotlin 要跟着宿主走 |

所以这个工程里**刻意摆了三层**：底下 `SurfaceView` 放视频，上面一层透明的
`GLSurfaceView` 当 UI 层的替身，再上面一个普通 `Button`。三层叠得住、触摸分得
开，回退路线才算成立。

## 跑

```sh
export JAVA_HOME=<JDK>
export ANDROID_HOME=<SDK>
./gradlew assembleRelease
./drive.sh                # 阶段与 slint-mpv/android-drive.sh 一一对应
adb logcat -s kk-media3
```

素材直接复用 Slint 探针那边推上去的 `probe.mp4` + `probe-audio.wav`
（`drive.sh` 自己 `cp` 过来）：**同一份素材，对照才有意义**。probe.mp4 本身
没有音频轨，所以音频是外挂的一条 WAV，用 `MergingMediaSource` 合进来——
跟 B 站的 DASH 分轨、跟 Slint 那边的 `MediaSource::Dash` 都是同一个形状。

## 日志前缀

跟 Slint 探针对齐，两边能用同一套眼睛看：

| 前缀 | 含义 |
|---|---|
| `EV` | 生命周期回调 |
| `STATUS` | 每秒一条的播放状态 |
| `DECODER` | MediaCodec 选中的解码器名（`c2.qti.*` 是硬件，`c2.android.*` 是软件） |
| `DROP` | 丢帧 |
| `TOUCH` / `PIP` / `SIZE` / `CMD` | 同名的那几项 |

## 不进仓库的东西

`/.gradle/`、`build/`、`local.properties` 都在 `.gitignore` 里。SDK 路径走
`ANDROID_HOME` 环境变量，不写 `local.properties`——那是本机路径，不该跟着仓库走。
