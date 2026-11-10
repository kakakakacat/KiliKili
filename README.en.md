<div align="center">

# KiliKili

**Five platforms · A third-party Bilibili client being rebuilt around Rust**

[简体中文](README.md) · [English](README.en.md)

</div>

KiliKili is forked from [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus). We are keeping the familiar viewing experience while gradually rebuilding the shared core in Rust for a more reliable, maintainable app across platforms.

## Platforms

| Platform | Current app |
| :--- | :---: |
| Android | ✅ |
| iPhone / iPad | ✅ |
| macOS | ✅ |
| Windows | ✅ |
| Linux | ✅ |

Platform support comes from PiliPlus. Official KiliKili builds are not yet available; watch [Releases](https://github.com/kakakakacat/KiliKili/releases) for updates.

## Rebuilding with Rust

| Today | In progress | Future direction |
| :--- | :--- | :--- |
| Keep the current Flutter UI and player | Move networking, data processing, and business logic to Rust step by step | Consider a Slint UI after thorough validation |

The transition is gradual, with existing features, user data, and playback taking priority. If a Rust path fails, the app can fall back to the existing implementation.

## Project activity

These charts are generated from the `main` branch and update after every push.

![Recent commits](assets/stats/commits.svg)

![Source-code changes](assets/stats/code-changes.svg)

<sub>Statistics start at the fork baseline. Line counts include source code only, excluding generated files and upstream syncs. GitHub Actions keeps the charts updated.</sub>

## Long-term maintenance

**We commit to updating KiliKili for the long term.** We will continue to follow upstream fixes, API changes, and platform compatibility issues. Share feedback in [Issues](https://github.com/kakakakacat/KiliKili/issues).

Thanks to [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus), [PiliPalaX](https://github.com/orz12/PiliPalaX), [PiliPala](https://github.com/guozhigq/pilipala), and all contributors. KiliKili follows [GPL-3.0](LICENSE) and is an independently maintained, unofficial client.
