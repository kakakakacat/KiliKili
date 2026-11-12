//! 弹幕引擎的 Dart 导出：会话生命周期 + 分段并入 + 按桶取用。
//!
//! ## 为什么是这个形状（issue #42、ADR-010）
//!
//! **不**把整段弹幕交给 Dart。一个热门视频的单个分段有数千条，序列化
//! 加 Dart 侧反序列化比基线直接用 protobuf.dart 解还慢——基线已经为此
//! 在 256 KiB 以上切 isolate。所以：Rust 持有已解码、已过滤、已分桶的
//! 结果，Dart 按播放进度取**一桶**（100ms），每次只过几条。
//!
//! 三个函数的同步性是刻意区分的：
//!
//! - [`danmaku_push_segment`] **异步**。它要解几千条 protobuf 并跑一遍
//!   引擎，放在 Dart 线程上就是一次掉帧；FRB 默认把非 sync 函数放到
//!   Rust 侧工作线程，等于白拿了基线那个 isolate 的效果。
//! - [`danmaku_bucket`] **同步**（`#[frb(sync)]`）。渲染侧每 100ms 取一次，
//!   且基线 `getCurrentDanmaku` 是同步返回的；若它返回 Future，调用方
//!   就得改成异步，弹幕会晚一帧出现。一次 map 查询加几条记录的拷贝，
//!   同步是合适的。
//! - [`danmaku_open`] / [`danmaku_close`] 同步：只动一个 map。
//!
//! ## 会话所有权
//!
//! 会话存在本模块的全局表里，键是单调递增、**不复用**的 id。Dart 侧
//! 一个 `PlDanmakuController` 对应一个会话，`dispose` 时关闭。在途的
//! 分段响应若在关闭之后才回来，它持有的 id 已经查不到会话，于是被丢弃
//! ——这正是 ADR-003 的 `generation` 要达到的效果（细节见
//! `bili_danmaku::session` 的模块文档）。
//!
//! 全局表是桥接层的**生命周期管理**，不是业务逻辑：引擎与会话本身在
//! `bili-danmaku`，Phase 2 换成 Slint 后原样复用，被丢掉的只有这个表。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use bili_danmaku::cursor::{CursorState, ProgressStep, step as cursor_step};
use bili_danmaku::engine::{EngineOptions, ProcessedElem};
use bili_danmaku::rule::FilterRules;
use bili_danmaku::session::DanmakuSession;
use bili_domain::session::SessionId;
use flutter_rust_bridge::frb;

/// 引擎参数。对应基线 `PlPlayerController` 上的四个会话内不变量。
#[derive(Debug, Clone)]
pub struct DanmakuEngineOptions {
    /// 未登录时不做 `isSelf` 标记。
    pub is_login: bool,
    /// 当前账号 mid 的 crc32 十六进制串（基线 `midHash`）。
    pub self_mid_hash: String,
    /// 合并同内容弹幕（基线 `Pref.mergeDanmaku`）。
    pub merge: bool,
    /// 关键词屏蔽（子串、区分大小写）。
    pub block_keywords: Vec<String>,
    /// 正则屏蔽。**已去掉两端斜杠**的源串，与基线存进 Hive 的形态一致。
    pub block_regexes: Vec<String>,
    /// UID 屏蔽，比对的是弹幕的 `mid_hash`。
    pub block_uids: Vec<String>,
}

/// 开会话的结果。`session_id` 为 `None` 即失败，调用方走 Dart 原路径。
#[derive(Debug, Clone)]
pub struct DanmakuOpenResult {
    pub session_id: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
}

/// 并入一个分段的结果。
#[derive(Debug, Clone)]
pub struct DanmakuPushResult {
    /// 失败时为 false，调用方应**整条**降级回 Dart：Rust 侧少并入一个
    /// 分段的表现是「这一段没人发弹幕」，没有任何提示。
    pub ok: bool,
    /// 该分段此前已并入，本次无操作。
    pub already_loaded: bool,
    /// 本分段解出的条数（过滤前）。
    pub decoded: u32,
    /// 会话已入桶的总条数（过滤后）。
    pub total: u32,
    /// 服务端的分段状态，1 表示该视频弹幕已关闭。
    pub state: i32,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
}

/// 一条可渲染的弹幕。
///
/// **只带渲染与交互真正要用的字段**（基线 `lib/pages/danmaku/view.dart`
/// 用到的那些）。这是每 100ms 跨一次 FFI 的结构，把 `action`、`extra`、
/// `id_str`、`oid` 这些没有消费方的字段一并拷过去，只是在热路径上多拷
/// 几个字符串。需要它们时再加，并在这里写明谁要用。
#[derive(Debug, Clone)]
pub struct DanmakuBucketItem {
    pub id: i64,
    /// 视频内毫秒偏移。渲染不用，保留它是为了 Dart 侧重建出的弹幕对象
    /// 与基线一致（排查问题时按进度对得上号）。
    pub progress: i32,
    pub mode: i32,
    pub fontsize: i32,
    pub color: u32,
    pub mid_hash: String,
    pub content: String,
    /// 屏蔽等级。阈值过滤在渲染时做，不在引擎里做。
    pub weight: i32,
    pub like_count: i64,
    /// `DmColorfulType` 的线上数值，60001 为大会员渐变色。
    pub colorful: i32,
    /// 合并后的重复条数；未参与合并时为 0。
    pub count: i32,
    pub is_self: bool,
}

impl From<&ProcessedElem> for DanmakuBucketItem {
    fn from(p: &ProcessedElem) -> Self {
        Self {
            id: p.elem.id,
            progress: p.elem.progress,
            mode: p.elem.mode,
            fontsize: p.elem.fontsize,
            color: p.elem.color,
            mid_hash: p.elem.mid_hash.clone(),
            content: p.elem.content.clone(),
            weight: p.elem.weight,
            like_count: p.elem.like_count,
            colorful: p.elem.colorful.into(),
            count: p.count,
            is_self: p.is_self,
        }
    }
}

/// 会话表。
///
/// `Mutex` 而非 `RwLock`：竞争只可能发生在「取桶」与「并入分段」之间
/// （两个线程），读写锁的复杂度换不来东西。
///
/// 注意这把锁的代价：取桶是**同步**调用，若恰好撞上一次并入，它会等到
/// 那次并入做完。一个分段几千条，并入是毫秒级，而取桶每 100ms 一次——
/// 也就是说每载入一个分段，最多有一次取桶要多等几毫秒。这仍然远好于
/// 基线（基线在 Dart 线程上解同一份分段，超过 256 KiB 才切 isolate），
/// 但它是真机验收要盯的点之一（ADR-010 的「待验证」）。若实测不可接受，
/// 下一步是把分桶结果换成可并发读的结构，而不是把取桶改成异步——异步
/// 会让弹幕晚一帧。
#[frb(ignore)]
fn sessions() -> MutexGuard<'static, HashMap<u64, DanmakuSession>> {
    static SESSIONS: OnceLock<Mutex<HashMap<u64, DanmakuSession>>> = OnceLock::new();
    SESSIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        // 中毒意味着某次持锁时 panic 了。表本身仍然是一致的（panic 只
        // 可能来自分配），继续用它比让整条弹幕链路从此不可用更好。
        .unwrap_or_else(PoisonError::into_inner)
}

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// 新建一个弹幕会话。规则编不过时返回错误，调用方整条降级回 Dart。
#[frb(sync)]
#[must_use]
pub fn danmaku_open(options: DanmakuEngineOptions) -> DanmakuOpenResult {
    let rules = match FilterRules::compile(
        options.block_keywords,
        &options.block_regexes,
        options.block_uids,
    ) {
        Ok(rules) => rules,
        Err(e) => {
            return DanmakuOpenResult {
                session_id: None,
                error_kind: Some(e.kind().to_string()),
                error_detail: Some(e.detail().to_owned()),
            };
        }
    };

    let id = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    let engine = EngineOptions {
        is_login: options.is_login,
        self_mid_hash: options.self_mid_hash,
        merge: options.merge,
        rules,
    };
    sessions().insert(id, DanmakuSession::new(SessionId(id), engine));

    tracing::debug!(session = id, "danmaku session opened");
    DanmakuOpenResult {
        // u64 → i64：Dart 的 int 是 64 位有符号。会话 id 从 1 开始递增，
        // 溢出需要连开 2^63 个会话。
        session_id: i64::try_from(id).ok(),
        error_kind: None,
        error_detail: None,
    }
}

/// 解码并并入一个分段。
///
/// `bytes` 是**解帧并解压之后**的 protobuf 字节，不是 HTTP 响应体——
/// gRPC 的 5 字节前缀与 gzip 仍由 Dart 的 `GrpcReq.decompressProtobuf`
/// 处理（理由同 `bili_api::danmaku` 的模块文档）。
///
/// 非 `sync`：解码几千条 protobuf 要在 Rust 工作线程上做。
#[must_use]
pub fn danmaku_push_segment(
    session_id: i64,
    segment_index: i64,
    bytes: Vec<u8>,
) -> DanmakuPushResult {
    fn failure(kind: &str, detail: String) -> DanmakuPushResult {
        DanmakuPushResult {
            ok: false,
            already_loaded: false,
            decoded: 0,
            total: 0,
            state: 0,
            error_kind: Some(kind.to_owned()),
            error_detail: Some(detail),
        }
    }

    let segment = match bili_api::danmaku::parse_segment(&bytes) {
        Ok(segment) => segment,
        Err(e) => return failure(&e.kind().to_string(), e.detail().to_owned()),
    };

    let Ok(id) = u64::try_from(session_id) else {
        return failure("cancelled", format!("会话 id 非法: {session_id}"));
    };
    let mut guard = sessions();
    let Some(session) = guard.get_mut(&id) else {
        // 会话已关闭：切 P / 切视频 / 页面销毁之后才回来的响应。按
        // ADR-003，过期结果必须被丢弃，而不是报错给用户。
        return failure("cancelled", format!("会话 {session_id} 已关闭"));
    };

    let outcome = session.ingest_segment(segment_index, segment);
    DanmakuPushResult {
        ok: true,
        already_loaded: outcome.already_loaded,
        decoded: u32::try_from(outcome.decoded).unwrap_or(u32::MAX),
        total: u32::try_from(outcome.total).unwrap_or(u32::MAX),
        state: outcome.state,
        error_kind: None,
        error_detail: None,
    }
}

/// 取某个播放进度所在的 100ms 桶。
///
/// 返回 `None` 有两种可能：会话不存在，或这 100ms 内没有弹幕。两者在
/// 渲染侧是同一种处理（这一帧不加弹幕），不必区分；要区分会话是否存在
/// 的地方是 [`danmaku_push_segment`]，那里会报错。
#[frb(sync)]
#[must_use]
pub fn danmaku_bucket(session_id: i64, progress_ms: i32) -> Option<Vec<DanmakuBucketItem>> {
    let id = u64::try_from(session_id).ok()?;
    let guard = sessions();
    let bucket = guard.get(&id)?.bucket(progress_ms)?;
    Some(bucket.iter().map(DanmakuBucketItem::from).collect())
}

/// 关闭会话并释放它持有的弹幕。id 不再复用。
///
/// 重复关闭是无操作：Dart 侧的 `dispose` 可能被调用多次。
#[frb(sync)]
pub fn danmaku_close(session_id: i64) {
    // 刻意不用 let-chain：`#[frb(sync)]` 展开出的代码按 2021 版解析，
    // let-chain 在那里是语法错误（edition 2024 才允许）。
    let Ok(id) = u64::try_from(session_id) else {
        return;
    };
    if sessions().remove(&id).is_some() {
        tracing::debug!(session = id, "danmaku session closed");
    }
}

/// 当前未关闭的会话数。
///
/// 只为诊断，且确有使用者：Dart 侧在 debug 构建关闭会话后打印它
/// （`RustDanmakuSession.close`），ADR-010 的真机验收也靠它判断切 P /
/// 退出页面后有没有留下会话。泄漏一个会话等于泄漏一个视频的全部弹幕，
/// 而那在界面上没有任何症状。
#[frb(sync)]
#[must_use]
pub fn danmaku_open_session_count() -> u32 {
    u32::try_from(sessions().len()).unwrap_or(u32::MAX)
}

/// [`danmaku_progress_step`] 的结果：这一帧怎么处理 + 调用方要存回去的游标。
///
/// 刻意是扁平结构而不是带值的枚举：本桥接层至今没有导出过枚举，
/// 保持导出面「纯数据结构 + 明确的函数」（AGENTS.md §2）。
#[derive(Debug, Clone, Copy)]
pub struct DanmakuProgressStep {
    /// 这一帧要不要按 [`Self::bucket_ms`] 取桶。false 时调用方什么都不做,
    /// 连分段请求也不要触发——那会在回跳期间白拉视频开头那一段。
    pub emit: bool,
    /// 取桶前要不要先清屏。只有 seek 造成的不连续会置 true。
    pub clear: bool,
    /// 要取的桶（整百毫秒）。`emit` 为 false 时无意义。
    pub bucket_ms: i32,
    /// 游标状态，调用方原样存下来，下一帧传回来。`-1` 表示还没发过桶。
    pub last_bucket_ms: i32,
    /// 游标已看到的 seek 代际，同样原样存回。
    pub seen_epoch: i64,
}

/// 判定一帧播放进度该不该取桶、要不要先清屏（issue #89）。
///
/// 判据全在 [`bili_danmaku::cursor`]，这里只做 `Option<i32>` 与 `-1` 的
/// 互转——FRB 的导出面不带 Option 哨兵更省事，而「还没发过桶」在 Dart
/// 侧本来就要有个初值。
///
/// **纯函数，不碰会话表。** 原因是它必须对两条路径都生效：Rust 弹幕引擎
/// 开着时取 Rust 的桶，引擎降级回 Dart 时取 `_dmSegMap` 的桶，而降级恰恰
/// 发生在会话建不起来的时候。挂在会话上，降级路径就没人看门了。
///
/// 同步（`#[frb(sync)]`）的理由与 [`danmaku_bucket`] 一样：渲染侧每 100ms
/// 调一次，返回 Future 会让弹幕晚一帧。
#[frb(sync)]
#[must_use]
pub fn danmaku_progress_step(
    last_bucket_ms: i32,
    seen_epoch: i64,
    position_ms: i32,
    epoch: i64,
) -> DanmakuProgressStep {
    let state = CursorState {
        last_bucket_ms: if last_bucket_ms < 0 {
            None
        } else {
            Some(last_bucket_ms)
        },
        seen_epoch,
    };
    let outcome = cursor_step(state, position_ms, epoch);
    let (emit, clear, bucket_ms) = match outcome.step {
        ProgressStep::Skip => (false, false, 0),
        ProgressStep::Emit { bucket_ms } => (true, false, bucket_ms),
        ProgressStep::Restart { bucket_ms } => (true, true, bucket_ms),
    };
    DanmakuProgressStep {
        emit,
        clear,
        bucket_ms,
        last_bucket_ms: outcome.state.last_bucket_ms.unwrap_or(-1),
        seen_epoch: outcome.state.seen_epoch,
    }
}
