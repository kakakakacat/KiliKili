# SPIKE-001 · macOS：Slint + libmpv 视频与弹幕覆盖

状态：**NOT TESTED**（实验台已就绪，等一台 mac）
对应 issue：[#8](https://github.com/kakakakacat/KiliKili/issues/8)
最后更新：2026-11-21

这份是**待填的记录位**，不是结论。真正的记录由实验台自己写出来覆盖本文件
下半部分；在那之前，这里只说明「要跑什么、跑完算不算数」。

## 为什么还没跑

实验台在一台 Windows 开发机上写成并跑通（见
[2026-11-21-windows-dryrun.md](2026-11-21-windows-dryrun.md)），那台机器上
没有 macOS。[VALIDATION-CAPABILITY](../VALIDATION-CAPABILITY.md) §1 记着
macOS 的真机 GUI 验证在**本机 M4** 上做——这一步只能在那台机器上完成，
CI 替不了：GitHub 的 macOS runner 没有可用的 GPU 合成与显示输出，
编译成功与「视频能不能垫在 UI 底下」是两个问题。

## 怎么跑

```sh
brew install mpv                     # 提供 libmpv.2.dylib
cd spikes/slint-mpv
cargo run --release -- \
    --video <DASH 视频 URL> --audio <DASH 音频 URL> \
    --user-agent <UA> --referer https://www.bilibili.com \
    --scripted --seek-to 30000 --danmaku-rate 30 \
    --hwdec videotoolbox \
    --shots ../../target/spike-shots \
    --record ../../docs/m0/spikes/SPIKE-001-macos.md
```

跑完把生成的文件**连同截图一起**看一遍，再按下面「人工项」逐条判。

取流 URL 从哪来：`rust/crates/bili-api` 的取流路径已经迁完（issue #79），
用真实账号取一次 DASH 的音视频地址即可；URL 带签名和时效，**不要写进记录**
（记录会脱敏主目录，但不会脱敏 URL，自己别粘上去）。手边没有取流条件时，
先用本地文件把前七项跑出来，DASH 分轨那项标 BLOCKED，别空着。

## 这次要额外盯的三件事（Windows 上问不出来的）

1. **FBO 方向**。Windows + femtovg 上实测需要 `--flip-y`，macOS 上未知。
   画面上下颠倒就加这个开关，并把结论记进本文件——两端不一致的话，
   Phase 2 的适配器里得按平台分支。
2. **OpenGL 在 macOS 上已废弃**。mpv 的 render API 走的是 CGL，能用，但
   Apple 随时可能把它拿掉。如果这条路在 macOS 上就已经吃力（掉帧、色彩
   不对、HiDPI 下模糊），那 ADR-004 要认真考虑 `MPV_RENDER_PARAM_SW_*`
   或者 Metal 层的方案，而不是把问题留到 Phase 2 再撞。
3. **videotoolbox 是不是真的开了**。记录里的「hwdec 实际」若是
   `videotoolbox-copy`，说明每帧还在 GPU→CPU→GPU 走一趟（Windows 这次就是
   `d3d11va-copy`），CPU 占用与功耗都要重新看——这正是 #9 里写的
   「避免每帧 CPU RGBA 拷贝」在桌面端的同一个问题。

## 人工项怎么判

实验台只敢判机器能判的（帧率、暂停是否冻结位置、SeekCompleted 是否真的来过、
hwdec 字符串）。剩下这些对着截图和屏幕自己判，判完写进本文件：

| 项 | 怎么算过 |
|---|---|
| 层级 | 视频在最下、弹幕在视频之上、HUD 在最上；弹幕不被视频盖住，也不糊成一片 |
| 画面 | 颜色正常、不颠倒、不拉伸；窗口缩放与全屏切换后比例仍对 |
| HiDPI | Retina 下文字边缘锐利，不是放大的模糊位图；FBO 像素尺寸 = 逻辑尺寸 × 缩放 |
| 音画同步 | 有声音，且和画面对得上（DASH 分轨时尤其要听） |
| 暂停 | 画面与弹幕同时静止，恢复后不跳帧 |
| Seek | 跳转后旧弹幕不残留，新位置的弹幕在一两秒内出来 |

## Go / No-Go 的判据

[SPIKE-PLAN](../SPIKE-PLAN.md) 写的是：视频合成/覆盖、移动端生命周期、长列表、
必要富文本、IME、无障碍达不到可接受质量就 NO-GO。本条只覆盖**视频合成与覆盖**
这一项，它过了不代表 Phase 2 可以走——还要 #9、#10、#11。

---

## 实验记录（跑完由工具覆盖写入）

**NOT TESTED** —— 本节在 mac 上跑一次 `--record` 即被整段替换。
