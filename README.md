# KiliKili

[English](README.en.md)

KiliKili 是一个面向 Android、iOS、macOS、Windows 和 Linux 的第三方哔哩哔哩客户端，基于 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus) fork 开发。感谢 PiliPlus 及其上游 [PiliPalaX](https://github.com/orz12/PiliPalaX)、[PiliPala](https://github.com/guozhigq/pilipala) 的作者和贡献者。KiliKili 是独立维护的衍生项目，不代表这些上游项目或哔哩哔哩官方。

## 项目目标

我们希望在保留 PiliPlus 已有功能和用户数据兼容性的基础上，逐步构建一个稳定、易维护的跨平台客户端：

1. **先迁移业务核心**：保留现有 Flutter 界面和播放器，将网络协议、数据处理等可复用逻辑逐步迁移到 Rust。每一步都经过对照测试，并保留回退路径。
2. **再验证新界面**：评估 Slint 在视频播放、弹幕、移动端生命周期、输入法和无障碍等场景中的可行性。只有验证通过后，才会逐步替换界面；Flutter + Rust 版本本身也会继续保持可用。
3. **持续改进体验**：完善跨平台构建、兼容性、性能、稳定性和问题修复，而不是只完成一次技术迁移。

目前项目处于 **Flutter + Rust 逐步迁移阶段**。Slint 是后续目标，尚未成为当前应用的界面。详细方案和验收条件见 [架构 RFC](docs/architecture/RFC-0001-rust-slint-migration.md)。

## 长期维护承诺

**KiliKili 会长期更新和维护。** 我们会持续跟进上游的重要修复、哔哩哔哩接口变化及各平台兼容性问题，并公开记录迁移进展与已知限制。更新会以质量和实际可用性为先，不承诺固定的发版周期；即使 Slint 方案未通过验证，也会继续维护可用的 Flutter + Rust 版本。

欢迎通过 [Issues](https://github.com/kakakakacat/KiliKili/issues) 反馈问题或提出建议，也欢迎提交 Pull Request。

## 获取与构建

代码和发布信息以[本仓库](https://github.com/kakakakacat/KiliKili)为准。构建前请阅读 [构建基线与环境说明](docs/m0/BASELINE.md)：当前 Flutter 构建依赖指定 SDK 版本和平台补丁，直接使用未打补丁的 Flutter SDK 可能失败。

## 许可与致谢

本项目遵循仓库中的 [GPL-3.0 许可证](LICENSE)。衍生代码与资源保留其原有版权及许可要求。再次感谢 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus) 和所有上游贡献者。

本项目是非官方客户端，不提供破解内容；使用时请遵守相关服务条款和当地法律。
