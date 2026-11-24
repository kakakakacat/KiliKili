# Player Port Flutter 适配（issue #4）

用户已确认按独立切片推进。契约不重写；沿用 `bili-domain::player` 和
`bili-core::PlayerSession`。本次完成 Flutter 适配与可自动化验证，真机五条
验收矩阵缺项继续挂 #4，不凭代码测试关单。

## 方案与边界

采用 Rust 会话 + Flutter media-kit 平台适配器。纯 Dart 复制代际算法会违反
宿主可替换原则；先完成 #5 的所有通用异步机制会扩大切片，因此均不采用。
`bili-core::media_kit_port` 包含后端确认状态机、媒体资源编码、错误分类；
桥接层只做 DTO 转换和会话表。Dart 只调用 media-kit，读 libmpv 事件并映射
为语言中立的事实批次，再消费 Rust 批准的快照。

media-kit 当前公开流没有 FILE_LOADED / PLAYBACK_RESTART，seek Future 也只代表
命令回复。适配器创建同一个 mpv 核心的独立 client，非阻塞排空自己的事件队列，
不抢 media-kit 的队列，不修改 pub cache，不依赖新版本。周期 40ms 是采样频率，
不累计或外推播放时间；一次批次最多产生一个位置快照。属性用异步 get_property 回复，保留请求发起时的代际；
不在 UI 线程同步读取核心属性。

## 时序与取消

每次 Open / Seek 在 Rust 预留新代际，立即作废旧快照；Dart 命令串行执行。
进入真正后端调用前先排空队列，再通知 Rust 开始对应命令。Open 必须先见
START_FILE 再见 FILE_LOADED；Seek 必须先见 SEEK 再见 PLAYBACK_RESTART，
才晋升事件代际。命令回复不能冒充完成。后续命令若已作废当前命令，其确认仍可
释放串行队列，但事实不能更新 UI。关闭即从 Rust 会话表删除，取消定时器并销毁
观察 client；逻辑关闭后不得更新 UI 或派发新命令。media-kit 内部已在途的调用
不可取消，物理 dispose 必须等待它退出，避免 stop 与 open 的内部 continuation 并行。

观察属性覆盖真实 position、duration、pause、buffering、speed、eof、轨道和
buffer。命令与事件使用同一 session/generation。失败与队列溢出显式报告，不
把无法判定的旧数据盖成新代际。后端确认有超时；超时/事件溢出后永久停用本适配器，必须重建播放器才能再开播，
不能在同一后端直接放行下一次 seek，以免把 A 的迟到确认认作 B 的确认。
关闭发生在 open 内部等待时，以及 A 超时→B→A 迟到确认均须有回归。
属性缓存在 START_FILE 清空；只有 FILE_LOADED 后才发新资源快照。
轨道选择通过 SelectTracks；AttachSurface 只登记 VideoController 已就绪的
不透明标识，DetachSurface/SetLifecycle 通知端口，平台纹理生命周期仍由插件处理。

## 产品接入

`PlPlayerController` 的 open / refresh / seek / play / pause / speed 经过适配器；
主状态监听改为消费 Rust 批准的快照。保留音量、着色器、画面大小、PiP 等平台 API。
删除被替代的 Dart EDL 拼接、错误分类和局部 `_seekEpoch` 算法；暂停按钮、
自动跳过与片段预览统一订阅可跨后端重建的权威快照流，弹幕使用 Rust
代际。异步初始化和切片后的回调按播放请求令牌隔离。平台表面仍由 VideoController
管理；Flutter 已有生命周期行为继续执行，并把生命周期意图通知端口。

## 验证与验收

Rust 回归：未确认的旧快照、快速连发 seek、切资源、关闭、跨会话、暂停/缓冲不
推进时钟、速度事实、错误分类、Unicode/分轨资源编码、批次合并。
真实 FRB 回归：typed 命令/事件、代际过滤、关闭后计数归零。
Dart 适配器回归：模拟传输乱序/关闭，与真实 libmpv 本地生成媒体的 open、暂停、
seek、倍速、切资源测试分开。能在本机运行的真实测试留运行记录；移动端 PiP、
前后台、网络缓冲、真实弹幕与 baseline wall time/RSS 对照仍按 ADR 五项逐格记录，
不得把桌面无窗口自动化写成真机全量验收。同步更新 ADR-003、验证能力、手工验证
路径、分叉面文档。不修改协议请求签名，不添加 protocol-changelog。
