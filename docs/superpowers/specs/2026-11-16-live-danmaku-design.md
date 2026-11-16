# S11b 直播弹幕长连接迁移（#62）

用户已批准：Rust 接管 token、连接、鉴权、30 秒心跳、分包、zlib/brotli、typed 消息解析与过滤；Dart 每约 100ms 拉取一批事件。会话通过 session_id / generation 隔离，关闭后取消任务且不再输出事件。按服务端顺序尝试候选地址，沿用原有断线即关闭行为，不额外自动重连。

领域实体与解析不依赖宿主。直播间消费 DANMU_MSG、SUPER_CHAT_MESSAGE、WATCHED_CHANGE、ONLINE_RANK_COUNT、ROOM_CHANGE；历史预取与 SuperChat REST 一并迁移以删除共用旧模型。可变展示状态在宿主的包装中保存，FRB 结构保持不可变。屏蔽规则在 Rust 处理，Flutter 保留布局、交互、平台播放器调用和具体文案。

队列、解压体积、嵌套深度均设置上限，避免恶意或拥挤房间无限占用内存；溢出显式上报计数。失败使用 error_kind/error_detail/error_code 返回，不以异常跨桥。取消覆盖连接中、鉴权中、接收中和在途 token/预取结果。凭据不写日志。

验证：协议编码和畸形帧、压缩与批处理、取消竞态、本地 WebSocket 桩、真实脱敏 fixture 与一次性 Dart parity、FRB 生成、完整 Rust 三项检查及 Dart 分析和适用离线测试。真机回归统一留 Phase 2；不合并或关闭 issue，待所有者许可。
