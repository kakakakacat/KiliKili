# 发布签名：各平台的身份、材料与验证

状态：**Linux 已启用并实跑验证通过（密钥已配，run 44876645273）；Windows / macOS 已接好流程，等证书；iOS 结论为「仓库侧没有发布身份」**
日期：2026-11-21
关联：[issue #121](https://github.com/kakakakacat/KiliKili/issues/121)、[issue #146](https://github.com/kakakakacat/KiliKili/issues/146)、[AGENTS.md §8](../../AGENTS.md)、[发布验证](../qa/RELEASE-VALIDATION.md)

本文只写**跨版本不变的约定**：每个平台的发布身份是什么、材料放在哪、CI 怎么用、
使用者怎么验、轮换时动哪些地方。进度与待办在 issue 里，不在这儿维护副本。

**私钥、证书、口令一律不进仓库。** 进仓库的只有公钥和指纹——它们本来就是要
公开的，而且正是「下一版还是同一个发布者」这件事的锚点。

---

## 0. 现状一览

| 平台 | 实际分发方式 | 发布身份 | 当前状态 | 材料形态 |
|---|---|---|---|---|
| Linux | GitHub Release 的 deb / rpm / AppImage / tar.gz | 一把 GPG 密钥 | **已启用**（2026-11-21 实跑通过） | GPG 签名子钥（无口令） |
| Windows | GitHub Release 的 setup.exe / 便携版 zip | 代码签名证书 | 未签名，接入点已留 | OV/EV 证书（见 §3 的硬件存储约束） |
| macOS | GitHub Release 的 dmg | Developer ID Application + 公证 | 未签名未公证，接入点已留 | Apple 证书 p12 + 公证 API key |
| iOS | 侧载（AltStore / Sideloadly 等） | **没有，也不该有**（§5） | 故意未签名 | — |
| Android | GitHub Release 的 apk | 独立的 Android 发布证书 | 两套证书已建立，验证包覆盖安装通过；首次正式发布尚未验收（#146） | JKS/PKCS12，**只用于 Android** |

一条贯穿全文的规矩：**同一平台的后续版本必须沿用同一身份**。换身份的代价
按平台不同——Android 换签名等于无法原地升级，Linux 换 GPG 键等于所有老用户
的校验突然失败，Windows/macOS 换证书等于重新积累信誉——所以每个平台都在
CI 里钉了指纹，不一致就直接红（见 §2.4、§3.3）。

---

## 1. 通用约定

1. **一个平台一套材料，不混用。** Android 的 JKS 不拿去签别的平台，GPG 密钥
   也不拿去充当 Apple 身份。各平台的信任链本来就不互通，混用只会让轮换时
   一处出错全线崩。
2. **CI 只拿到够用的那一份。** Linux 传进 CI 的是**签名子钥**，主钥（能签发/
   吊销子钥的那把）留在离线备份里；Apple 传的是证书 p12，不是开发者账号密码。
   CI 的材料泄露时，能补救的范围越小越好。
3. **secret 配好之后要有人确认它真的生效了。** Android 的 #146 修复把密钥
   缺失改为直接失败，并在构建前后核对证书指纹。其他平台缺失 secret 时仍会
   显式提示未签名；配好 secret 的第一次构建，要去日志里看到「签名身份: …」
   那行才算数。
4. **可复用工作流默认拿不到调用方的 secrets。** `build.yml` 调 `ios.yml` /
   `mac.yml` / `win_x64.yml` / `linux_x64.yml` 时必须带 `secrets: inherit`，
   少这一行的话，Secrets 配好了签名也照样跳过，而且一声不吭。

---

## 2. Linux：GPG

### 2.1 为什么是 GPG，签在哪

Linux 这边没有「证书颁发机构」这一层，来源校验靠发布者自己的 GPG 公钥。
三种包各有自己的包内签名位置，另外统一配一份分离签名：

| 产物 | 包内签名 | 谁会去验 |
|---|---|---|
| `.rpm` | RPM 头部签名（`rpmsign --addsign`） | `dnf` / `rpm -K`，导入公钥后自动验 |
| `.deb` | ar 归档里的 `_gpgorigin` 成员（`debsigs`） | `debsig-verify`，**需要本机配策略**，默认不验 |
| `.AppImage` | **没有**（`appimagetool --sign` 会把包弄坏，见下） | — |
| 四者全部 | `<文件名>.asc` 分离签名 + 签过名的 `SHA256SUMS` | 使用者手动 `gpg --verify`，或跑 `tool/verify-release-signatures.sh` |

**分离签名才是实际最常被用到的那一条**：apt 验的是软件源的 `InRelease`，不是
单个 `.deb`；AppImage 双击运行时也不校验签名。包内签名仍然要做——走包管理器
安装的人能免费得到一道校验——但别指望它覆盖「从 Release 页点下载」的场景。

**AppImage 刻意不做包内签名。** 2026-11-21 在 run 44875708097 上实测
（appimagetool continuous build 370 / git `11d54c9`）：`--sign` 自己是成功的，
日志里有 `[sign] Success`，连公钥都嵌进去了——但产出的 AppImage 只剩 944632
字节，**正好等于它下载的 runtime 大小**，22.6MB 的 squashfs 被整段截掉。它往
ELF 段里写签名时重写了整个 ELF，追加在后面的 squashfs 没被带上；同一份 AppDir
不签名时是 21–24MB。是体积闸门拦下来的（PR #34 当初加它就是为这种事），
`[sign] Success` 本身毫无保护作用。要恢复包内签名，得先钉一个确定能用的
appimagetool 版本，并**重新实测产物体积**，而不是只看它说成功。

### 2.2 生成密钥（仓库所有者操作，一次性）

在**本机**做，不要在 CI 或任何共享机器上生成。

```sh
# 全程在一个临时钥匙环里做，别动自己的 ~/.gnupg
export GNUPGHOME=$(mktemp -d); chmod 700 "$GNUPGHOME"

# 1. 主钥。**先不设口令**——第 3 步要导出一份无口令的 CI 子钥；口令留到
#    第 5 步再给本机这份加上，之后导出的离线备份就是带口令的。
#    算法用 RSA 4096 而不是 ed25519：rpm 对 EdDSA 的支持要到较新的版本，
#    老一点的发行版上 `rpm --checksig` 会直接判不过——而签名的意义正是
#    给别人验的。dpkg / AppImage 两侧没有这个顾虑，但没必要为它们分家。
gpg --batch --passphrase '' --quick-generate-key \
    "KiliKili Release Signing <你的邮箱>" rsa4096 sign never
FPR=$(gpg --list-keys --with-colons | awk -F: '/^fpr:/ {print $10; exit}')
echo "$FPR"   # 40 位指纹，记进 §2.5 的表，后面到处要用

# 2. 专用于 CI 的签名子钥，2 年有效期。到期续签即可，**身份不变**。
gpg --batch --passphrase '' --quick-add-key "$FPR" rsa4096 sign 2y

# 3. 导出给 CI 的那一份：只含子钥，无口令。
#    为什么必须无口令：rpmsign / debsigs / appimagetool 各自去调 gpg，
#    没有统一的口令传递口子，有口令时它们要么卡在 pinentry 上直到超时，
#    要么静默产出未签名的包。CI 这份的保护来自 Actions Secret 本身，
#    而且它只是子钥——泄露时用主钥吊销子钥即可，身份不用换。
gpg --export-secret-subkeys --armor "$FPR" > kilikili-ci-subkey-PRIVATE.asc

# 4. 公钥，要进仓库
gpg --armor --export "$FPR" > signing-key.asc

# 5. 给主钥加口令，再导出离线备份与吊销证书（这两条是交互式的）
gpg --passwd "$FPR"
gpg --export-secret-keys --armor "$FPR" > kilikili-master-PRIVATE.asc
gpg --gen-revoke "$FPR" > kilikili-revocation.asc

# 6. 备份文件挪到加密介质（至少两份异地）后，临时钥匙环连同里面的私钥一起删掉
rm -rf "$GNUPGHOME"; unset GNUPGHOME
```

然后：

- 把 `signing-key.asc` 提交到 **`assets/linux/signing-key.asc`**；
- 把指纹写进本文 §2.5 的记录表；
- 把 `kilikili-ci-subkey-PRIVATE.asc` 的**全文**存为 Actions secret
  `LINUX_GPG_PRIVATE_KEY`（`gh secret set LINUX_GPG_PRIVATE_KEY < kilikili-ci-subkey-PRIVATE.asc`）；
- 删掉本机的 `/tmp/ci-subkey.asc` 与导出的私钥文件（备份介质上的除外）。

### 2.3 CI 做了什么

`linux_x64.yml`，按执行顺序：

1. **导入发布签名密钥**（构建之前，密钥有问题就不浪费十几分钟打包）：导入、
   取指纹、**空签一次**确认无口令、与 `assets/linux/signing-key.asc` 比对指纹。
2. **包内签名**：`rpmsign --addsign`、`debsigs --sign=origin`，并导出公钥为
   `KiliKili-linux-signing-key.asc` 一起发布。
3. **生成 `SHA256SUMS`**（必须排在包内签名之后——rpmsign 与 debsigs 改写的是
   包本身，先算的哈希对不上最终发出去的文件）。
4. **分离签名**：四个产物 + `SHA256SUMS` 各出一个 `.asc`。
5. **校验签名**：逐个 `gpg --verify`、`sha256sum --check`、`rpm --checksig`，
   deb 则确认 `_gpgorigin` 确实在包里、且签它的是我们这把钥匙。

   第 5 步有个坑值得记下来：**实际签名的是子钥，包里写的是子钥的 key id，
   不是主钥指纹**。认签名者时要拿「主钥 + 全部子钥」的 key id 全集去比
   （`gpg --list-keys --with-colons` 里 `pub:` / `sub:` 行的第 5 栏），拿
   40 位指纹的后 16 位去比会把正确的签名判成别人签的——而且是在「签名
   明明是对的」的情况下红，查起来格外费时间。

没配 `LINUX_GPG_PRIVATE_KEY` 时 1–6 全部跳过，产物与以前逐字节同构，但日志里
会有一段「本次 Linux 产物没有签名」的提示。

### 2.4 轮换与吊销

换身份是**破坏性**的：老用户配好的校验会从此失败。所以 CI 钉了指纹——secret
换了而 `assets/linux/signing-key.asc` 没换，构建直接红。确实要换时：

1. 用主钥签发新子钥（**身份不变**，这种情况公钥文件也不用动，只换 secret）；
2. 真要换主钥（泄露、算法迁移）：发布一次**过渡版本**，用老钥签一份新公钥
   声明，并在 Release 说明里写明指纹变化；同时更新 `assets/linux/signing-key.asc`
   与 §2.5 的记录表；
3. 主钥泄露时先发吊销证书（`gpg --import kilikili-revocation.asc` 后上传到
   公钥服务器），再走第 2 步。

### 2.5 身份记录

**这张表是「同一身份」的权威记录**，比对指纹时以它为准，不要以某次构建日志
为准——日志里那串是构建自己打印的，拿它核对等于没核对。

| 项 | 值 |
|---|---|
| UID | `KiliKili Release Signing`（**不带邮箱**：UID 随公钥永久公开，而来源校验靠的是指纹，不是邮箱） |
| 主钥指纹 | `E52E1D5C119DC831920CF0031391988F12E7D6E8` |
| 主钥算法 | RSA 4096，仅签名，不过期 |
| CI 子钥 ID | `DA915DBC409585B0`（RSA 4096，仅签名） |
| 子钥到期 | 2028-11-20（到期前用主钥续签即可，**身份不变**） |
| 公钥文件 | `assets/linux/signing-key.asc` |
| 生成日期 | 2026-11-21 |
| 首次实跑验证 | [run 44876645273](https://github.com/kakakakacat/KiliKili/actions/runs/44876645273)：CI 侧签名与校验全绿；本机用仓库里这份公钥跑 `tool/verify-release-signatures.sh` 验下载到的真实产物，通过 |
| 备份位置（不写具体路径/口令） | **待办**：主钥、自动生成的吊销证书与 CI 子钥导出目前只在生成它的那台开发机上，尚未转入加密介质 |
| 首次使用的版本 | *待填：第一个带签名的正式发布* |

子钥签名时包里写的是 `DA915DBC409585B0`，**不是**上面那个 40 位主钥指纹——
这正是 §2.3 第 5 条那个坑的实例。

---

## 3. Windows：代码签名证书

### 3.1 现状与后果

未签名。用户首次运行 `setup.exe` 会看到 SmartScreen 的蓝色拦截框，要点
「更多信息 → 仍要运行」；部分企业环境会直接禁止运行。便携版 zip 里的
`kilikili.exe` 同样未签名。

### 3.2 买之前必须知道的一件事

**2023-06-01 起，CA/B Forum 要求所有代码签名证书（OV 与 EV）的私钥存放在
硬件或云端 HSM 里**，CA 不再签发可以直接下载成 `.pfx` 的 OV 证书。这决定了
接入方式，买错了流程要重写：

- **云签名服务**（Azure Trusted Signing、DigiCert KeyLocker、SSL.com eSigner
  等）：CI 里用 `signtool sign /dlib <供应商的 dlib> /dmdf <配置>`，或供应商
  提供的 Action。按月计费的那几家是目前最便宜的路子。
- **硬件令牌**（USB token）：**不能用于 CI**——没人能把 U 盘插进 GitHub 的
  runner。只能本地签，发布流程要随之改成「本地签完再上传 Release」。
- **`.pfx` 文件**：只有 2023 年规则生效前签发、尚未过期的老证书，或自签名
  证书才会是这个形态。

`win_x64.yml` 现在实现的是 `.pfx` 那条路（`WINDOWS_CERT_PFX_BASE64` +
`WINDOWS_CERT_PASSWORD`）。**选定云签名供应商后，这一步要加一个分支**，
secret 也换成供应商的凭据——签名位置与校验方式不变，改动集中在那一步里。

### 3.3 CI 做了什么

`win_x64.yml` 的「签名 Windows 产物」步骤，在打包 zip 之前：

- 找到 runner 上的 `signtool.exe`（路径带 SDK 版本号，镜像升级时会变）；
- 对便携版的 `kilikili.exe` 和 `setup.exe` 各签一次，**必须带时间戳**
  （`/tr` + `/td sha256`）——否则证书一到期，已经发出去的包全部失效；
- `signtool verify /pa` 当场复核；
- 配了 `WINDOWS_CERT_SHA1` 时比对证书指纹，防止身份被悄悄换掉。

**已知缺口：安装包内部的那一份 exe 签不到。** `fastforge package` 一条命令
同时做 `flutter build` 和 Inno Setup 打包，等它跑完，exe 已经被压进安装包了。
要连那一份一起签，得把它拆成「先 build、签、再 package」，而 package 会重跑
一次 `flutter build`，很可能覆盖掉刚签好的 exe。没有证书在手时这个改法无法
验证，留到真拿到证书、能在 CI 里跑通一次时再动（#121）。于是现在的效果是：
便携版的 exe 已签名，**通过安装包装上去的那一份仍未签名**——安装包本身的
签名在安装那一刻起作用，装完之后的文件不受保护。

### 3.4 身份记录

| 项 | 值 |
|---|---|
| 证书类型（OV / EV / 云签名供应商） | *待填* |
| 主体名（CN） | *待填* |
| SHA-1 指纹 | *待填，同时设为 `WINDOWS_CERT_SHA1`* |
| 到期时间 | *待填* |
| 续期负责人 | *待填* |

---

## 4. macOS：Developer ID + 公证

### 4.1 现状与后果

未签名、未公证。用户双击 dmg 里的 app 会被 Gatekeeper 拦下（「无法打开，
因为无法验证开发者」），要右键「打开」或 `xattr -dr com.apple.quarantine`。

### 4.2 需要什么

Apple Developer Program（99 美元/年）。对外分发必须用 **Developer ID
Application** 证书——`Apple Development` 与 `Apple Distribution` 都不行，前者
只能装在登记过的设备上，后者是 App Store 用的。另外要一份**公证用的 App
Store Connect API key**（`.p8`）：签名和公证是两件事，只签名的包在 macOS
10.15+ 上照样被拦。

Secrets：

| 名字 | 内容 |
|---|---|
| `MACOS_CERT_P12_BASE64` | Developer ID Application 证书 + 私钥导出的 p12，base64 |
| `MACOS_CERT_PASSWORD` | 导出 p12 时设的口令 |
| `MACOS_NOTARY_KEY_BASE64` | App Store Connect API key（`.p8`），base64 |
| `MACOS_NOTARY_KEY_ID` | key id |
| `MACOS_NOTARY_ISSUER_ID` | issuer id |

仓库变量 `MACOS_SIGN_IDENTITY` 可选：不填时 CI 自己从钥匙串里找
`Developer ID Application`。

### 4.3 CI 做了什么

`mac.yml`：

1. 证书导入一个**临时钥匙串**（不碰 `login.keychain`，随 runner 一起消失），
   并设 partition list——不设的话 codesign 会弹 UI 授权框，在无人值守的 runner
   上表现为卡住直到超时；
2. 构建后**由内向外**签：先签 `Contents/Frameworks` 下的 `.framework` /
   `.dylib`，再签 app 本体。不用 `--deep`（Apple 已不推荐，且对 .dylib 的处理
   不可靠）。签名带 `--options runtime`（hardened runtime，公证的硬性要求）
   和 `--timestamp`，entitlements 用仓库里的 `macos/Runner/Release.entitlements`；
3. dmg 本身也签——Gatekeeper 校验的是用户双击的那个文件；
4. `notarytool submit --wait` 公证，失败时**把公证日志打出来**（被拒的真正
   原因只在那份日志里，通常是某个嵌套二进制没签、没开 hardened runtime 或
   缺时间戳）；
5. `stapler staple` 装订，让用户首次打开不需要联网；再用
   `spctl -a -t open --context context:primary-signature` 按 Gatekeeper 实际
   放行 dmg 的那条策略复核一次。

只配证书、不配公证凭据时，第 4、5 步跳过并打出提示——包是签过的，但首次
打开仍会被拦。

### 4.4 身份记录

| 项 | 值 |
|---|---|
| Team ID | *待填* |
| 证书主体（Developer ID Application: …） | *待填* |
| 证书到期 | *待填* |
| 公证 key id / issuer id | *待填（只记 id，不记密钥内容）* |

---

## 5. iOS：仓库侧没有发布身份，这是结论不是遗漏

分发方式是侧载（AltStore / Sideloadly / TrollStore 等），**最终签名由使用者
自己的 Apple ID 在本机完成**，签名身份因此属于使用者，不属于仓库。

就算买了开发者账号，仓库这边能签出来的也只有：

- **ad-hoc**：要逐台登记 UDID，显然不适用于公开分发；
- **企业证书（Enterprise）**：Apple 限定用于组织内部分发，本项目的用法不符合
  条款，而且被吊销时所有已安装的包会一起失效；
- **App Store**：第三方 B 站客户端上不了架。

所以 `ios.yml` 产出的是**故意未签名**的 IPA。里面那行
`codesign --force --sign -` 是 ad-hoc 占位签名，只为让侧载工具重签时不出错，
不是发布签名。

**后果要在发布说明里写清楚**：iOS 用户的安装有效期取决于他自己用的账号
（免费 Apple ID 签出来的 7 天一过期），这不是本项目能改变的事。

---

## 6. Android：验证身份与发布身份

历史上的 CI APK 使用 runner 临时 debug 签名，无法作为升级起点。#146 把身份
分成两套：`build.yml` 留空 `tag` 与 `ci.yml` 构建的是 `.dev` 应用，用固定的
**验证密钥**；`build.yml` 带 `tag` 才构建无后缀的正式应用，必须用独立的
**发布密钥**。验证密钥的 APK 不用于对外分发，也不能升级正式应用。

`tool/android-signing.py` 在构建前验证 keystore 的证书 SHA-256 是否等于 Actions
variable，构建后用 `apksigner verify --print-certs` 核对每个 APK，并用 `aapt`
确认应用 ID。任一 secret、
variable 缺失或不匹配时构建失败；发布步骤因此不会上传错误身份的 APK。
本地未配置正式密钥时，普通 debug 开发仍可使用 debug 签名；缺正式签名的
release 构建在 `preReleaseBuild` 阶段失败，不在 Gradle 配置阶段阻断 debug。
仓库不保存私钥、口令或 JKS。仓库所有者须在自己的机器上分别生成两把密钥，
离线加密备份，再设置下表；**正式密钥绝不能复用验证密钥**。

| 用途 | Actions secrets | Actions variable |
|---|---|---|
| 验证包 `.dev` | `ANDROID_CI_KEYSTORE_BASE64`、`ANDROID_CI_STORE_PASSWORD`、`ANDROID_CI_KEY_ALIAS`、`ANDROID_CI_KEY_PASSWORD` | `ANDROID_CI_CERT_SHA256` |
| 正式包 | `SIGN_KEYSTORE_BASE64`、`KEYSTORE_PASSWORD`、`KEY_ALIAS`、`KEY_PASSWORD` | `ANDROID_RELEASE_CERT_SHA256` |

所有者操作步骤：

1. 在本机用 `keytool -genkeypair -keystore <独立路径>.jks -alias <别名>
   -keyalg RSA -keysize 3072 -validity 10000` 分别生成验证与发布密钥；密码由
   `keytool` 交互读取，不写进命令行或仓库。用 `keytool -list -v -keystore ...`
   记下 `SHA256` 证书指纹。
2. 把各自 JKS 的 base64 全文存入对应 secret，密码与别名也存入对应 secret；
   把**公开**指纹存入对应 variable。GitHub Actions 密钥不能读回，必须先给
   两把 JKS 和密码做至少两份加密离线备份。
3. 在验证分支上连续手动运行两次 `build.yml`，仅启用 Android、`tag` 留空；
   下载同一架构 APK，在同一设备上先装第一包再覆盖装第二包，确认数据保留。
   随后用带 `tag` 的**候选发布**验证正式包指纹及下一版覆盖升级；触发带
   `tag` 的工作流会创建 GitHub Release，须按发布流程执行。

| 身份记录 | 验证包 | 正式包 |
|---|---|---|
| 证书 SHA-256 | `9f4982e33fd0640f4b189bc0b98711e37defbb19493b2e0b0290c545f4763e44` | `a8b1ac2fda2f60ec01d29a8e4acbbff46e48a0b58fc50adc08ec12638b619e97` |
| 首次使用版本 / run | `2.2.1-10567a7fd+5914`；[run 45318556187](https://github.com/kakakakacat/KiliKili/actions/runs/45318556187)、[run 45318560355](https://github.com/kakakakacat/KiliKili/actions/runs/45318560355)，DN_ENG41 覆盖安装通过 | *首次正式发布后填入* |
| 备份状态 | 本机加密包已做解密回读校验，待两份离线转存 | 本机加密包已做解密回读校验，待两份离线转存 |

---

## 7. 使用者怎么验

### Linux

```sh
# 从 Release 页下载包、对应的 .asc、SHA256SUMS、SHA256SUMS.asc
tool/verify-release-signatures.sh ~/Downloads/kilikili
```

脚本把公钥导进一个临时钥匙环（不碰使用者的 `~/.gnupg`），验 `SHA256SUMS.asc`、
核对哈希、再逐个验分离签名。它**刻意不验包内签名**：rpm 与 deb 各要一套本机
工具与配置，而它们证明的来源与这条链完全相同。

包管理器侧的校验（可选）：

```sh
sudo rpm --import KiliKili-linux-signing-key.asc && rpm --checksig -v KiliKili_linux_*.rpm
# deb 的包内签名要 debsig-verify + /etc/debsig/policies 下的策略文件，
# 默认不装、apt 也不会用它；只想确认来源的话，验 .asc 就够了。
```

**第一次拿到公钥时要核对指纹**（§2.5 的记录表），之后每次校验都只是在确认
「还是同一个人签的」。只看脚本输出的指纹等于没核对——那串是它自己打印的。

### Windows / macOS

```powershell
signtool verify /pa /v KiliKili_windows_*_setup.exe
Get-AuthenticodeSignature .\KiliKili_windows_*_setup.exe | Format-List
```

```sh
spctl -a -t open --context context:primary-signature -vv KiliKili_macos_*.dmg
codesign --verify --strict --verbose=2 /Applications/KiliKili.app
xcrun stapler validate KiliKili_macos_*.dmg
```

---

## 8. 仓库所有者的待办清单

按平台独立，哪一项做完哪一项生效：

- [x] **Linux**：密钥已生成、公钥已钉在 `assets/linux/signing-key.asc`、
      `LINUX_GPG_PRIVATE_KEY` 已配、实跑验证通过（§2.5）。
- [ ] **Linux 的备份**：主钥、吊销证书与 CI 子钥导出仍在生成它的那台开发机上，
      要转入加密介质并补上 §2.5 的「备份位置」。主钥丢了只能换身份，
      而换身份等于所有老用户的校验一齐失败。
- [ ] **Windows**：先定供应商形态（§3.2），再决定是走 `.pfx` 分支还是加
      云签名分支；拿到后设 `WINDOWS_CERT_PFX_BASE64` /
      `WINDOWS_CERT_PASSWORD` / `WINDOWS_CERT_SHA1`，填 §3.4。
- [ ] **macOS**：开 Apple Developer Program → 建 Developer ID Application
      证书与公证 API key → 设 §4.2 的五个 secret → 填 §4.4。
- [ ] **Android**：按 §6 建立两套身份与加密备份，设置 secrets / vars，
      连续两次验证包覆盖安装，再核对首次正式发布的证书与升级路径（#146）。
- [ ] 每个平台首次签名发布后，按 [发布验证](../qa/RELEASE-VALIDATION.md)
      在真实系统上验一次安装与**原地升级**（同身份覆盖安装不应要求先卸载）。
