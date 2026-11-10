# KiliKili

[简体中文](README.md)

KiliKili is a third-party Bilibili client for Android, iOS, macOS, Windows, and Linux. It is forked from [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus). We thank the authors and contributors of PiliPlus and its predecessors, [PiliPalaX](https://github.com/orz12/PiliPalaX) and [PiliPala](https://github.com/guozhigq/pilipala). KiliKili is maintained independently and is not affiliated with those projects or Bilibili.

## Goals

We aim to build a stable, maintainable cross-platform client while preserving PiliPlus features and compatibility with existing user data:

1. **Migrate the shared core first.** Keep the current Flutter UI and player while gradually moving reusable protocol and data logic to Rust. Each step is checked against the existing behavior and retains a fallback path.
2. **Validate a new UI before adopting it.** Evaluate Slint for video playback, danmaku overlays, mobile lifecycle, text input, and accessibility. We will replace the UI gradually only if those checks pass; the Flutter + Rust version remains a usable product in its own right.
3. **Keep improving the client.** Invest in cross-platform builds, compatibility, performance, stability, and bug fixes beyond the initial migration.

The project is currently in the **Flutter + Rust migration stage**. Slint is a future goal, not the UI used by the current app. See the [architecture RFC](docs/architecture/RFC-0001-rust-slint-migration.md) for the plan and acceptance criteria.

## Long-term maintenance

**We commit to maintaining and updating KiliKili for the long term.** We will continue to follow important upstream fixes, Bilibili API changes, and platform compatibility issues, while documenting migration progress and known limitations. Releases will prioritize quality and usability over a fixed schedule. If the Slint approach does not pass validation, we will continue maintaining the usable Flutter + Rust version.

Bug reports, suggestions, and pull requests are welcome through this repository's [Issues](https://github.com/kakakakacat/KiliKili/issues) and Pull Requests.

## Source and builds

Use [this repository](https://github.com/kakakakacat/KiliKili) for source code and release information. Before building, read the [build baseline and environment notes](docs/m0/BASELINE.md): Flutter builds currently require a specific SDK version and platform patches, and may fail with an unpatched SDK.

## License and credits

This project follows the [GPL-3.0 license](LICENSE) in this repository. Derived code and assets retain their applicable copyright and license requirements. Thanks again to [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus) and all upstream contributors.

KiliKili is an unofficial client and does not provide cracked content. Please follow applicable service terms and local laws.
