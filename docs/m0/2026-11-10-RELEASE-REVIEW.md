# 发布产物复盘 — 2026-11-10

状态：**已完成**（结论均经实测，非推断）
关联：[VALIDATION-CAPABILITY.md](VALIDATION-CAPABILITY.md) §2.1、
[upstream-divergence.md](../architecture/upstream-divergence.md)、
`tool/inspect-artifact.py`

本文记录一次复盘的过程与结论。详细证据写在上列文档里，此处不重复，只保留
**为什么要查**和**查出了什么规律**——这部分原本散落在 PR 正文中，仓库里没有
一处把它们串起来。

## 1. 起因

起点只是「看一下待办计划」。读 `VALIDATION-CAPABILITY.md` 时发现其中两条
已记录的 CI 保障在仓库里并不存在，于是逐条核对配置与产物，一路牵出四个缺陷。

## 2. 四种形态

四个问题形态不同，但有一个共同点：**没有任何 status check 会因此变红**。

| 形态 | 实例 | 为什么 CI 看不见 |
| --- | --- | --- |
| 保障是想象的 | 文档称每个 PR 自动编译，实际五端构建全部只有 `workflow_dispatch` | 配置与文档从未对账 |
| 绿灯证明不了产物内容 | Windows 构建成功，日志里 `cargo` / `rustc` 零出现 | `flutter build` 吞掉插件编译输出 |
| 包在，但不是应用 | Linux 发布的 RPM 是 11,289 字节的 debuginfo 子包 | `find -exec mv {} <固定名> \;` 分次执行，后者静默覆盖前者 |
| 包对，元数据错 | deb 少声明 `libwebkit2gtk-4.1-0` 与 `libsoup-3.0-0` | 构建机永远装着这些库，缺口只在用户的干净系统上暴露 |

第三种的触发时机值得单独记：它由 `d9b23ba4e`（为 RPM 打包装 elfutils）引入。
装上 elfutils 后 `rpmbuild` 才开始额外产出 debuginfo 包，于是那个一直存在的
覆盖缺陷第一次具备触发条件。**同一次改动让构建从红转绿，让产物从「没有」
变成「错的」。**

## 3. 由此得到的判断

- **构建绿 ≠ 产物对。** 日志同样不是证据——成功与失败的日志可以完全一样。
- **体积闸门只能挡住第三种形态。** 第四种的包体积完全正确，错的是元数据，
  只有解析 ELF 的 `DT_NEEDED` 才能发现。
- **凡是「从多个候选里挑一个」的打包步骤都值得怀疑。** 审计五端后确认
  只有 Linux 一处用了会静默覆盖的写法，其余 `mv a b c` 形式在多匹配时会
  直接报错。
- **自动生成优于手工维护。** rpm 的依赖面完整而 deb 不完整，唯一区别是
  前者由 rpmbuild 从 ELF 生成，后者是手写的。

## 4. 变更

| PR | 内容 |
| --- | --- |
| #33 | 更正 `VALIDATION-CAPABILITY.md` 中两条不存在的 CI 保障 |
| #34 | **修复 RPM 发布 debuginfo 子包**，并为 RPM 加体积闸门 |
| #35 | 体积闸门推广到 Linux / Windows / macOS 的全部重打包产物 |
| #37 | 记录五端产物核查结果与各类产物的核对方法 |
| #38 | **修复 deb 缺失 webkit2gtk 与 libsoup3 依赖**（issue #36） |
| #39 | `tool/inspect-artifact.py`、deb 依赖改为 `dpkg-shlibdeps` 生成、上游分叉清单 |

## 5. 留下的能力

- **`tool/inspect-artifact.py`**：解开 apk / ipa / zip / deb / rpm / tar.gz /
  目录，报告内含的原生库与依赖声明。纯标准库，三端行为一致。`--require`
  带退出码，可直接用于 CI。
- **上传前体积闸门**：三个 workflow 各一处，阈值集中可读。
- **deb 依赖自动生成**：不再手写，缺关键库则构建失败。

## 6. 未解决

- **产物核对尚未进入 CI。** 工具已就位，但目前仍是人工抽查。要变成每次
  构建的硬门槛，需要决定在哪些 workflow 上以什么断言运行（关联 issue #13
  QA-001 的发布前清单）。
- **运行时仍未验证。** 五端产物都确认含有 Rust 动态库，但「装进去了」不等于
  「加载成功」。`RustCore.isReady` 在真机上是否为 true 仍然没人看过
  （issue #25）。本次复盘只覆盖到打包边界为止。
- **Linux 无真机验证环境**，deb 的依赖修复未在干净系统上实际安装验证，
  结论来自 ELF 与包元数据的静态比对（见 VALIDATION-CAPABILITY §3）。
