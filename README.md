<div align="center">

# KiliKili

**五端覆盖 · Rust 重构中的哔哩哔哩第三方客户端**

[简体中文](README.md) · [English](README.en.md)

</div>

KiliKili fork 自 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)。我们保留熟悉的观看体验，逐步用 Rust 重构核心能力，让各平台版本更稳定、更易维护。

## 支持平台

| 平台 | 当前客户端 |
| :--- | :---: |
| Android | ✅ |
| iPhone / iPad | ✅ |
| macOS | ✅ |
| Windows | ✅ |
| Linux | ✅ |

平台支持继承自 PiliPlus；KiliKili 正式安装包尚未发布，请关注 [Releases](https://github.com/kakakakacat/KiliKili/releases)。

## Rust 重构

| 现在 | 正在推进 | 未来方向 |
| :--- | :--- | :--- |
| 保留现有 Flutter 界面与播放器 | 将网络、数据处理和业务逻辑逐步迁移到 Rust | 充分验证后再考虑 Slint 界面 |

重构会分步进行，优先保证现有功能、用户数据和播放体验。Rust 路径出现问题时，仍可回退到原有实现。

## 项目动态

下图根据本仓库 `main` 分支的提交记录自动生成，在每次推送后更新。

![最近提交记录](assets/stats/commits.svg)

![代码行变更](assets/stats/code-changes.svg)

<sub>统计从 fork 基线开始；代码行数仅计源码，排除生成文件和上游同步提交。图表由 GitHub Actions 自动更新。</sub>

## 长期维护

**我们承诺长期更新 KiliKili。** 会持续跟进上游修复、接口变化与平台兼容问题。欢迎在 [Issues](https://github.com/kakakakacat/KiliKili/issues) 反馈体验。

感谢 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)、[PiliPalaX](https://github.com/orz12/PiliPalaX)、[PiliPala](https://github.com/guozhigq/pilipala) 和所有贡献者。本项目遵循 [GPL-3.0](LICENSE)，是独立维护的非官方客户端。
