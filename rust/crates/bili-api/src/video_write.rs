//! 视频页写操作的请求构造与解析（S4 / issue #54）。
//!
//! 对应基线 `VideoHttp` 里除发评论以外的全部写操作
//! （`lib/http/video.dart`，本片迁完已删）：点赞、点踩、投币、两种一键
//! 三连、关注/拉黑、删评论、追番的加删改、四条上报，以及首页「不感兴趣」
//! 的反馈与取消。
//!
//! 幂等定级逐条写在各自的构造函数上，依据取 ADR-013 决策三那张表——那
//! 张表本来就是拿 `video.dart` 当样例做的，这里是把它落成代码。
//!
//! ## 三个主机，三套鉴权
//!
//! 这一片横跨三个 base_url，而**鉴权方式跟着主机走**，不跟着「是不是写
//! 操作」走：
//!
//! | 主机 | 接口 | 鉴权 |
//! | --- | --- | --- |
//! | `api.bilibili.com` | 三连、关注、删评论、追番、三条上报 | Cookie + 表单里的 `csrf` |
//! | `app.bilibili.com` | 点赞、点踩、投币、不感兴趣 | **无 Cookie**，`access_key` + APP 签名 |
//! | `api.live.bilibili.com` | `roomEntryAction` | Cookie + **查询串**里的 `csrf`，体是 JSON |
//!
//! 第二行是基线拦截器的行为（`account_mgr.dart` 的 `isApp` 分支）：APP
//! 端接口不带 Cookie，而是把 `access_key` 塞进参数再整体 APP 签名。给它
//! 补一个 csrf 不会报错，只会让请求面与基线对不上。
//!
//! 第三行是 dio 默认值的后果：`roomEntryAction` 的调用点没写
//! `contentType`，而 `BaseOptions` 里也没设，于是走了 dio 的默认
//! `application/json`。参数分布也因此分成两处。
//!
//! ## 几个反直觉的取值，都在构造函数里钉死了
//!
//! - 点赞 `like=0` 是**点赞**，`1` 是取消（APP 端接口与 web 端相反）；
//! - 点踩同理；
//! - 三连的响应是**三件事各自的回执**，部分成功是正常结果，不是错误。
//!
//! 这类映射写反了不会报错，表现是「点一下没反应」或者「点赞变成了取消」
//! （ADR-013 补充五）。所以它们一律收在这里并有测试盯着，不散到调用点。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::video_write::{PgcTriple, UgcTriple};
use serde::Deserialize;
use serde_json::json;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{CsrfIn, PreparedRequest, PreparedWrite, WriteBody};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
///
/// 这里只放**路径**，主机由调用方放进 [`RequestContext::base_url`]：
/// 基线的 `Api.likeVideo` 等常量是拼好主机的完整 URL，因为它要同时当
/// 账号查表的键用（`ApiType.apiTypeSet`）；Rust 侧没有那张表，拆成两段
/// 反而不会把主机写进签名。
pub mod path {
    // --- api.bilibili.com ---
    pub const UGC_TRIPLE: &str = "/x/web-interface/archive/like/triple";
    pub const PGC_TRIPLE: &str = "/pgc/season/episode/like/triple";
    pub const RELATION_MOD: &str = "/x/relation/modify";
    pub const REPLY_DEL: &str = "/x/v2/reply/del";
    pub const HEART_BEAT: &str = "/x/click-interface/web/heartbeat";
    pub const HISTORY_REPORT: &str = "/x/v2/history/report";
    pub const MEDIA_LIST_HISTORY: &str = "/x/v1/medialist/history";
    pub const PGC_ADD: &str = "/pgc/web/follow/add";
    pub const PGC_DEL: &str = "/pgc/web/follow/del";
    pub const PGC_UPDATE: &str = "/pgc/web/follow/status/update";

    // --- app.bilibili.com ---
    pub const LIKE: &str = "/x/v2/view/like";
    pub const DISLIKE: &str = "/x/v2/view/dislike";
    pub const COIN: &str = "/x/v2/view/coin/add";
    pub const FEED_DISLIKE: &str = "/x/feed/dislike";
    pub const FEED_DISLIKE_CANCEL: &str = "/x/feed/dislike/cancel";

    // --- api.live.bilibili.com ---
    pub const ROOM_ENTRY_ACTION: &str = "/xlive/web-room/v1/index/roomEntryAction";
}

/// 三连与关注请求里的固定常量。值照抄基线，不求甚解——它们是风控用的
/// 来源标识，换一个值会被拒。
mod fixed {
    pub const STATISTICS: &str = r#"{"appId":100,"platform":5}"#;
    pub const UGC_TRIPLE_SPMID: &str = "333.788.0.0";
    pub const UGC_TRIPLE_SOURCE: &str = "web_normal";
    pub const RELATION_SPMID: &str = "333.1387";
    pub const RELATION_GAIA_SOURCE: &str = "web_main";
    pub const RELATION_DEVICE_REQ_JSON: &str =
        r#"{"platform":"web","device":"pc","spmid":"333.1387"}"#;
    /// 「不感兴趣」那两条接口上基线写死的客户端标识。
    pub const FEED_BUILD: &str = "1";
    pub const FEED_MOBI_APP: &str = "android";
}

/// APP 端接口的鉴权材料。
///
/// 对应基线拦截器 `isApp` 分支里用到的那几个值。它们的真相源在 Dart
/// 的账号体系，按请求传入、用完即弃（RFC-0001 §2.2）——Rust 不保存。
///
/// `access_key` 未登录时为 `None`，此时整个参数不出现，与基线的
/// `if (!account.accessKey.isNullOrEmpty)` 一致。
#[derive(Debug, Clone)]
pub struct AppAuth {
    pub appkey: String,
    pub appsec: String,
    pub access_key: Option<String>,
    /// 签名用的 Unix 秒。由调用方注入，使构造保持纯函数、可被快照固定。
    pub ts: i64,
}

impl AppAuth {
    /// 把业务参数补成 APP 端接口真正发出去的那一组。
    ///
    /// 顺序：先放 `access_key`（基线是往同一个 map 里塞），再整体签名。
    /// `app_sign::sign` 返回的是按 key 升序 + 末尾 `sign` 的列表，与基线
    /// 发出去的 map 内容一致——表单字段的顺序服务端不看，签名覆盖的那串
    /// 才看，而那串两边都是排序后的。
    fn sign(&self, mut params: Vec<(String, String)>) -> Vec<(String, String)> {
        if let Some(key) = &self.access_key {
            params.push(("access_key".to_owned(), key.clone()));
        }
        bili_protocol::app_sign::sign(&params, &self.appkey, &self.appsec, self.ts)
    }
}

/// 把 BV 号换成写请求要的 `aid`。
///
/// 这一片有四条接口的参数是 `aid` 而调用点只有 `bvid`（基线在 Dart 里用
/// `IdUtils.bv2av` 转）。换算本身是协议（[`bili_protocol::bvid`]），放在
/// 这一层是因为**转错的后果是写操作落到另一个稿件上**——点赞点到别人
/// 头上。让它在这里失败，比发一个指向错误稿件的请求要好。
///
/// # Errors
/// 不是合法 BV 号 → [`ErrorKind::Protocol`]。
pub fn aid_from_bvid(bvid: &str) -> Result<i64, DomainError> {
    bili_protocol::bvid::bv_to_av(bvid).ok_or_else(|| {
        DomainError::new(
            ErrorKind::Protocol,
            format!("不是合法的 BV 号，写请求不会指向正确的稿件: {bvid}"),
        )
    })
}

// ----------------------------------------------------------- APP 端写操作

/// 点赞 / 取消赞。`like` 是**目标状态**：true = 点赞。
///
/// ## `like` 的取值和直觉是反的
///
/// 基线写的是 `'like': type ? '0' : '1'`，而传进去的 `type` 是
/// `!hasLike.value`，也就是**目标**状态。于是：目标为「赞」发 `0`，
/// 目标为「取消」发 `1`。
///
/// 这是 APP 端接口（`app.bilibili.com/x/v2/view/like`）的约定，与 web 端
/// `/x/web-interface/archive/like` 的 `1=赞 2=取消` 不是一回事。写反了
/// 不报错，表现是点赞变成取消（ADR-013 补充五）。
///
/// 定级 [`Idempotency::Idempotent`]：参数是目标状态，置为语义，重发落到
/// 同一个状态。
#[must_use]
pub fn like_request(aid: i64, like: bool, auth: &AppAuth) -> PreparedWrite {
    app_form(
        path::LIKE,
        vec![
            ("aid".to_owned(), aid.to_string()),
            ("like".to_owned(), if like { "0" } else { "1" }.to_owned()),
        ],
        auth,
        Idempotency::Idempotent,
    )
}

/// 点踩 / 取消踩。`dislike` 是**目标状态**，取值映射与 [`like_request`] 同。
#[must_use]
pub fn dislike_request(aid: i64, dislike: bool, auth: &AppAuth) -> PreparedWrite {
    app_form(
        path::DISLIKE,
        vec![
            ("aid".to_owned(), aid.to_string()),
            (
                "dislike".to_owned(),
                if dislike { "0" } else { "1" }.to_owned(),
            ),
        ],
        auth,
        Idempotency::Idempotent,
    )
}

/// 投币。`multiply` 是币数（上限 2），`select_like` 为 1 时附带点赞。
///
/// 定级 [`Idempotency::Verifiable`]：重发会**多扣硬币**。协议上
/// `/x/web-interface/archive/coins` 查得到本稿已投几枚，但本仓库没接
/// （`hasCoinVideo` 在基线 `api.dart` 里是注释掉的），所以这一档在界面上
/// 的落地形态就是「失败了让用户自己再按一次」（ADR-013 补充六）。
#[must_use]
pub fn coin_request(aid: i64, multiply: i64, select_like: i64, auth: &AppAuth) -> PreparedWrite {
    app_form(
        path::COIN,
        vec![
            ("aid".to_owned(), aid.to_string()),
            ("multiply".to_owned(), multiply.to_string()),
            ("select_like".to_owned(), select_like.to_string()),
        ],
        auth,
        Idempotency::Verifiable,
    )
}

/// 「不感兴趣」反馈与取消共用的参数。
///
/// `reason_id` 与 `feedback_id` 在基线里是**二选一**（调用点有
/// `assert((reasonId != null) ^ (feedbackId != null))`）。这里把它做成
/// 枚举，于是「两个都给」或「一个都不给」在类型上就不存在——基线那条
/// assert 在 release 构建里是不执行的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DislikeReason {
    Reason(i64),
    Feedback(i64),
}

impl DislikeReason {
    fn to_pair(self) -> (String, String) {
        match self {
            Self::Reason(id) => ("reason_id".to_owned(), id.to_string()),
            Self::Feedback(id) => ("feedback_id".to_owned(), id.to_string()),
        }
    }
}

/// 首页「不感兴趣」的反馈（`cancel` 为 true 时是取消）。
///
/// **基线用的是 GET**，尽管它改的是服务端状态。照搬——换成 POST 等于
/// 顺手改协议，而这一片的任务是迁移，不是修接口。因此它返回的是
/// [`PreparedRequest`] 而不是 [`PreparedWrite`]，也就没有幂等定级可填：
/// 重试规则由只读那条路径决定（`RetryPolicy::Read`）。
///
/// 这件事值得记一笔而不是默默照做：下一个人看到「写操作走 GET」多半会
/// 以为是漏改。它不是。
#[must_use]
pub fn feed_dislike_request(
    goto: &str,
    id: i64,
    reason: DislikeReason,
    cancel: bool,
    auth: &AppAuth,
) -> PreparedRequest {
    let params = vec![
        ("goto".to_owned(), goto.to_owned()),
        ("id".to_owned(), id.to_string()),
        reason.to_pair(),
        ("build".to_owned(), fixed::FEED_BUILD.to_owned()),
        ("mobi_app".to_owned(), fixed::FEED_MOBI_APP.to_owned()),
    ];
    PreparedRequest {
        path: if cancel {
            path::FEED_DISLIKE_CANCEL
        } else {
            path::FEED_DISLIKE
        },
        query: auth.sign(params),
    }
}

/// APP 端表单写请求的共同形状：签名 + 不带 csrf。
fn app_form(
    path: &'static str,
    params: Vec<(String, String)>,
    auth: &AppAuth,
    idempotency: Idempotency,
) -> PreparedWrite {
    PreparedWrite {
        path,
        query: Vec::new(),
        body: WriteBody::Form(auth.sign(params)),
        // APP 端接口没有 csrf，鉴权靠 access_key。
        csrf: CsrfIn::None,
        idempotency,
    }
}

// ----------------------------------------------------------- web 端写操作

/// UGC 一键三连。
///
/// 定级 [`Idempotency::Verifiable`]：含投币，跟着投币走（ADR-013）。
///
/// 那一串 `eab_x` / `ramval` / `ga` / `spmid` / `statistics` 是基线写死的
/// 固定值，照抄。
#[must_use]
pub fn ugc_triple_request(aid: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::UGC_TRIPLE,
        vec![
            ("aid".to_owned(), aid.to_string()),
            ("eab_x".to_owned(), "2".to_owned()),
            ("ramval".to_owned(), "0".to_owned()),
            ("source".to_owned(), fixed::UGC_TRIPLE_SOURCE.to_owned()),
            ("ga".to_owned(), "1".to_owned()),
            ("spmid".to_owned(), fixed::UGC_TRIPLE_SPMID.to_owned()),
            ("statistics".to_owned(), fixed::STATISTICS.to_owned()),
        ],
        Idempotency::Verifiable,
    )
}

/// PGC 一键三连。与 UGC 不是同一条接口，参数也只有 `ep_id` 一个。
#[must_use]
pub fn pgc_triple_request(ep_id: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::PGC_TRIPLE,
        vec![("ep_id".to_owned(), ep_id.to_string())],
        Idempotency::Verifiable,
    )
}

/// 操作用户关系（关注 1 / 取关 2 / 拉黑 5 / 取消拉黑 6 / 移除粉丝 7）。
///
/// 定级 [`Idempotency::Idempotent`]：`act` 是**目标关系**而非增量
/// （ADR-013 决策三的样例表），重发落到同一个关系。
///
/// `fp` 是基线 `extend_content` 里的那个浏览器 UA（`BrowserUa.pc`）。
/// 它是一段平台相关的字符串字面量，真相源留在宿主、按请求传入——与
/// Cookie 同理。
///
/// 参数分布在两处：`statistics` 与 `x-bili-device-req-json` 在查询串上，
/// 其余在表单里。基线如此。
#[must_use]
pub fn relation_mod_request(mid: i64, act: i64, re_src: i64, fp: &str) -> PreparedWrite {
    let extend_content = json!({
        "entity": "user",
        "entity_id": mid,
        "fp": fp,
    })
    .to_string();

    PreparedWrite {
        path: path::RELATION_MOD,
        query: vec![
            ("statistics".to_owned(), fixed::STATISTICS.to_owned()),
            (
                "x-bili-device-req-json".to_owned(),
                fixed::RELATION_DEVICE_REQ_JSON.to_owned(),
            ),
        ],
        body: WriteBody::Form(vec![
            ("fid".to_owned(), mid.to_string()),
            ("act".to_owned(), act.to_string()),
            ("re_src".to_owned(), re_src.to_string()),
            (
                "gaia_source".to_owned(),
                fixed::RELATION_GAIA_SOURCE.to_owned(),
            ),
            ("spmid".to_owned(), fixed::RELATION_SPMID.to_owned()),
            ("extend_content".to_owned(), extend_content),
        ]),
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Idempotent,
    }
}

/// 删除一条评论。
///
/// 定级 [`Idempotency::Idempotent`]：删第二次返回「不存在」，最终状态
/// 一致（ADR-013 决策三的样例表）。
#[must_use]
pub fn reply_del_request(reply_type: i64, oid: i64, rpid: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::REPLY_DEL,
        vec![
            ("type".to_owned(), reply_type.to_string()),
            ("oid".to_owned(), oid.to_string()),
            ("rpid".to_owned(), rpid.to_string()),
        ],
        Idempotency::Idempotent,
    )
}

/// 追番 / 追剧：加入。响应里有一句用来弹 toast 的文案。
#[must_use]
pub fn pgc_add_request(season_id: i64) -> PreparedWrite {
    season_form(path::PGC_ADD, &season_id.to_string(), None)
}

/// 追番 / 追剧：取消。
#[must_use]
pub fn pgc_del_request(season_id: i64) -> PreparedWrite {
    season_form(path::PGC_DEL, &season_id.to_string(), None)
}

/// 追番状态改为「想看 1 / 在看 2 / 看过 3」。
///
/// ## `season_ids` 为什么是字符串而不是数字
///
/// 这条接口收的是**逗号分隔的一串** id：追番列表页的批量操作把勾选的
/// 若干季一次改掉（基线 `FavPgcController.onUpdateList` 里的
/// `.map((item) => item.seasonId).join(',')`）。基线的参数类型是
/// `String` 正是为此，不是写漏了。
///
/// 改成 `i64` 会让批量操作在编译期就过不去——那是好事；但它同时会让
/// **单条**调用看起来更自然，于是下一个人很容易"顺手修正"，把批量那条
/// 路悄悄改成只改第一季。留成字符串，并把理由写在这里。
#[must_use]
pub fn pgc_update_request(season_ids: &str, status: i64) -> PreparedWrite {
    season_form(path::PGC_UPDATE, season_ids, Some(status))
}

/// 追番三条共用的形状。三条都是置为语义，定级 `Idempotent`。
fn season_form(path: &'static str, season_ids: &str, status: Option<i64>) -> PreparedWrite {
    let mut form = vec![("season_id".to_owned(), season_ids.to_owned())];
    if let Some(status) = status {
        form.push(("status".to_owned(), status.to_string()));
    }
    PreparedWrite::form_only(path, form, Idempotency::Idempotent)
}

// ------------------------------------------------------------- 上报类写操作

/// 历史记录上报。
///
/// 定级 [`Idempotency::Idempotent`]，但**调用方应当传 `retry_count: 0`**：
/// 上报是高频自动触发的，重试的收益接近零（下一次上报马上就来），代价
/// 却是账号级的风控计数（ADR-013 决策三「上报类的例外」）。
#[must_use]
pub fn history_report_request(aid: i64, kind: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::HISTORY_REPORT,
        vec![
            ("aid".to_owned(), aid.to_string()),
            ("type".to_owned(), kind.to_string()),
        ],
        Idempotency::Idempotent,
    )
}

/// 播放进度心跳的业务参数。
///
/// 字段多且互斥关系绕，用一个结构体而不是八个位置参数——位置参数里
/// 把 `epid` 和 `sid` 传反了编译器不会说话。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeartBeatParams {
    /// 课程（pugv）用 `aid`，其余用 `bvid`。基线就是这么分的，原样搬。
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub cid: i64,
    pub epid: Option<i64>,
    pub season_id: Option<i64>,
    pub sub_type: Option<i64>,
    /// `VideoType` 的数值编号（UGC 3 / PGC 4 / PUGV 10）。
    pub video_type: i64,
    /// 已播秒数。
    pub played_time: i64,
    /// 是否课程。决定上面那条 `aid` / `bvid` 的二选一。
    pub is_pugv: bool,
}

/// 播放进度心跳。
///
/// 定级与重试同 [`history_report_request`]。
///
/// **未登录时不该调用**：基线的拦截器直接把这条请求 reject 掉
/// （`account_mgr.dart` 里 `!account.isLogin && path == Api.heartBeat`），
/// 这个判断留在宿主——它要的是账号状态，而账号体系在 Dart 那边。
#[must_use]
pub fn heart_beat_request(params: &HeartBeatParams) -> PreparedWrite {
    let mut form = Vec::with_capacity(8);
    // 基线：`if (isPugv) 'aid': ?aid else 'bvid': ?bvid`——`?` 是「值为
    // null 就整个键不出现」。两个分支各自都可能一个键都不加。
    if params.is_pugv {
        if let Some(aid) = params.aid {
            form.push(("aid".to_owned(), aid.to_string()));
        }
    } else if let Some(bvid) = &params.bvid {
        form.push(("bvid".to_owned(), bvid.clone()));
    }
    form.push(("cid".to_owned(), params.cid.to_string()));
    if let Some(epid) = params.epid {
        form.push(("epid".to_owned(), epid.to_string()));
    }
    if let Some(sid) = params.season_id {
        form.push(("sid".to_owned(), sid.to_string()));
    }
    form.push(("type".to_owned(), params.video_type.to_string()));
    if let Some(sub_type) = params.sub_type {
        form.push(("sub_type".to_owned(), sub_type.to_string()));
    }
    form.push(("played_time".to_owned(), params.played_time.to_string()));

    PreparedWrite::form_only(path::HEART_BEAT, form, Idempotency::Idempotent)
}

/// 播放列表（medialist）的历史上报。
#[must_use]
pub fn media_list_history_request(desc: i64, oid: i64, upper_mid: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::MEDIA_LIST_HISTORY,
        vec![
            ("desc".to_owned(), desc.to_string()),
            ("oid".to_owned(), oid.to_string()),
            ("upper_mid".to_owned(), upper_mid.to_string()),
        ],
        Idempotency::Idempotent,
    )
}

/// 进入直播间的上报。
///
/// 这条与这一片其余接口都不一样，两处：**体是 JSON**（调用点没写
/// `contentType`，吃的是 dio 的默认值），**csrf 在查询串上**。见模块
/// 文档那张表。
#[must_use]
pub fn room_entry_action_request(room_id: i64) -> PreparedWrite {
    PreparedWrite {
        path: path::ROOM_ENTRY_ACTION,
        query: Vec::new(),
        // room_id 是数字不是字符串：基线传的是 `Object`，dio 按 JSON
        // 序列化，服务端看到的就是数字。
        body: WriteBody::Json(json!({ "room_id": room_id, "platform": "pc" })),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Idempotent,
    }
}

// ------------------------------------------------------------------- 解析

/// `num` 型字段。基线的模型声明成 `num?`（Dart 里 int 或 double 都收），
/// 实测服务端给的是整数。
///
/// 用 [`serde_json::Value`] 收下再转，而不是直接声明 `Option<i64>`：后者
/// 撞上一个 `2.0` 会让**整个响应**解析失败，于是一次成功的三连在界面上
/// 显示为失败。写操作上这种误判的代价比少一个字段高得多。
fn as_int(v: Option<serde_json::Value>) -> Option<i64> {
    match v? {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| {
            #[allow(clippy::cast_possible_truncation)]
            n.as_f64().map(|f| f as i64)
        }),
        _ => None,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UgcTripleDto {
    like: Option<bool>,
    coin: Option<bool>,
    fav: Option<bool>,
    multiply: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PgcTripleDto {
    coin: Option<serde_json::Value>,
    coin_number: Option<serde_json::Value>,
    favorite: Option<serde_json::Value>,
    fmid: Option<serde_json::Value>,
    follow: Option<serde_json::Value>,
    like: Option<serde_json::Value>,
    relation: Option<bool>,
}

/// 只有一句 toast 文案的响应体。追番三条用它。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ToastDto {
    toast: Option<String>,
}

/// 解析 UGC 三连的回执。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_ugc_triple(body: &str) -> Result<UgcTriple, DomainError> {
    let dto: UgcTripleDto = crate::envelope::unwrap_envelope(body)?;
    Ok(UgcTriple {
        like: dto.like,
        coin: dto.coin,
        fav: dto.fav,
        multiply: as_int(dto.multiply),
    })
}

/// 解析 PGC 三连的回执。
///
/// # Errors
/// 见 [`parse_ugc_triple`]。
pub fn parse_pgc_triple(body: &str) -> Result<PgcTriple, DomainError> {
    let dto: PgcTripleDto = crate::envelope::unwrap_envelope(body)?;
    Ok(PgcTriple {
        coin: as_int(dto.coin),
        coin_number: as_int(dto.coin_number),
        favorite: as_int(dto.favorite),
        fmid: as_int(dto.fmid),
        follow: as_int(dto.follow),
        like: as_int(dto.like),
        relation: dto.relation,
    })
}

/// 解析点赞的回执——一句 toast 文案，载荷在 `data` 里。
///
/// 缺了文案不算失败：点赞本身是成功的，界面上少一句提示而已。返回
/// `None`，由调用方决定兜底文案（那是 i18n 的事，留在宿主）。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_like_toast(body: &str) -> Result<Option<String>, DomainError> {
    let dto: ToastDto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto.toast)
}

/// 解析追番三条的回执。载荷在 **`result`** 而不是 `data` 里。
///
/// # Errors
/// 见 [`parse_like_toast`]。
pub fn parse_season_toast(body: &str) -> Result<Option<String>, DomainError> {
    let dto: ToastDto = crate::envelope::unwrap_result_envelope(body)?;
    Ok(dto.toast)
}

/// 点踩接口的响应可能**根本不是 JSON**。
///
/// 基线写的是 `if (res.data is! String && res.data['code'] == 0)`，失败时
/// 把那个字符串原样弹给用户。也就是说它见过服务端回一段纯文本——多半是
/// 网关层的拒绝页。
///
/// 这里保持同样的可见性：非 JSON 响应归 [`ErrorKind::Remote`]（服务端
/// 明确拒绝，不是我们的解析过时），detail 带上那段文本的前若干字节供
/// 诊断。不归 `Protocol` 是因为 `Protocol` 的含义是「协议变了，该改代码
/// 了」，而这里多半只是一次被拦。
///
/// # Errors
/// 非 JSON → `Remote`；`code != 0` → 按码分类。
pub fn check_dislike_response(body: &str) -> Result<(), DomainError> {
    if serde_json::from_str::<serde_json::Value>(body).is_err() {
        // 截断：这段文本会进错误对象，而错误对象会进日志。整页 HTML 没有
        // 诊断价值，前两百字节足够认出是什么。
        let head: String = body.chars().take(200).collect();
        return Err(DomainError::new(
            ErrorKind::Remote,
            format!("点踩接口返回了非 JSON 响应: {head}"),
        ));
    }
    crate::envelope::check_envelope(body)
}

// ------------------------------------------------------------------- 发送

/// 发一条没有载荷的写请求。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn send_write<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedWrite,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, prepared).await?;
    crate::envelope::check_envelope(&body)
}

/// 发一条写请求并解析它的载荷。
///
/// # Errors
/// 见 [`send_write`]，外加 `parse` 自己的失败。
pub async fn send_parsed<T, R, F>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedWrite,
    parse: F,
) -> Result<R, DomainError>
where
    T: HttpTransport,
    F: FnOnce(&str) -> Result<R, DomainError>,
{
    let body = crate::client::send_write(transport, ctx, prepared).await?;
    parse(&body)
}

/// 发「不感兴趣」那条 GET。
///
/// # Errors
/// 见 [`send_write`]。
pub async fn send_feed_dislike<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedRequest,
) -> Result<(), DomainError> {
    let body = crate::client::send(transport, ctx, prepared).await?;
    crate::envelope::check_envelope(&body)
}

/// 发点踩那条，用它自己的响应判定。
///
/// # Errors
/// 见 [`check_dislike_response`]。
pub async fn send_dislike<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedWrite,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, prepared).await?;
    check_dislike_response(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::request::CsrfIn;

    /// 占位 appkey/appsec，与 `bili_protocol::app_sign` 的测试同源，
    /// 不是用户凭据。
    fn auth() -> AppAuth {
        AppAuth {
            appkey: "dfca71928277209b".to_owned(),
            appsec: "b5475a8825547a4fc26c7d518eaaa02e".to_owned(),
            access_key: Some("access-key-placeholder".to_owned()),
            ts: 1_700_000_000,
        }
    }

    fn keys(w: &PreparedWrite) -> Vec<&str> {
        w.form_fields()
            .expect("这条接口是表单体")
            .iter()
            .map(|(k, _)| k.as_str())
            .collect()
    }

    fn value<'a>(form: &'a [(String, String)], key: &str) -> &'a str {
        form.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| panic!("缺字段 {key}"))
    }

    /// 钉死那个反直觉的映射：0 = 点赞，1 = 取消。
    ///
    /// 依据是基线的调用点——`likeVideo(type: !hasLike.value)` 传的是
    /// **目标**状态，而请求里写的是 `type ? '0' : '1'`。写反了不会报错，
    /// 表现是点赞变成取消。
    #[test]
    fn like_state_mapping_is_pinned() {
        let on = like_request(2, true, &auth());
        let form = on.form_fields().expect("表单体");
        assert_eq!(value(form, "like"), "0", "目标是点赞，发 0");
        assert_eq!(value(form, "aid"), "2");

        let off = dislike_request(2, false, &auth());
        assert_eq!(
            value(off.form_fields().expect("表单体"), "dislike"),
            "1",
            "目标是取消踩，发 1"
        );
    }

    /// APP 端接口三件事：带 access_key、整体签名、**没有 csrf**。
    #[test]
    fn app_writes_are_signed_and_carry_no_csrf() {
        let w = coin_request(2, 2, 1, &auth());
        assert_eq!(w.csrf, CsrfIn::None, "APP 端接口鉴权靠 access_key");
        let form = w.form_fields().expect("表单体");
        assert_eq!(value(form, "access_key"), "access-key-placeholder");
        assert_eq!(value(form, "appkey"), "dfca71928277209b");
        assert_eq!(value(form, "ts"), "1700000000");
        assert!(form.iter().any(|(k, _)| k == "sign"), "签名必须随请求发出");
        assert!(form.iter().all(|(k, _)| k != "csrf"));
        assert_eq!(w.idempotency, Idempotency::Verifiable, "投币会多扣硬币");
    }

    #[test]
    fn an_anonymous_app_write_omits_access_key_entirely() {
        // 基线：`if (!account.accessKey.isNullOrEmpty)`。补一个空串会让
        // 签名覆盖一个服务端不认的值。
        let anon = AppAuth {
            access_key: None,
            ..auth()
        };
        let w = like_request(2, true, &anon);
        assert!(
            w.form_fields()
                .expect("表单体")
                .iter()
                .all(|(k, _)| k != "access_key"),
            "未登录不该出现 access_key 字段"
        );
    }

    #[test]
    fn feed_dislike_stays_a_get_and_picks_the_right_path() {
        let a = feed_dislike_request("av", 2, DislikeReason::Reason(7), false, &auth());
        assert_eq!(a.path, path::FEED_DISLIKE);
        let b = feed_dislike_request("av", 2, DislikeReason::Feedback(9), true, &auth());
        assert_eq!(b.path, path::FEED_DISLIKE_CANCEL);

        let has = |r: &PreparedRequest, k: &str| r.query.iter().any(|(key, _)| key == k);
        assert!(has(&a, "reason_id") && !has(&a, "feedback_id"), "二选一");
        assert!(has(&b, "feedback_id") && !has(&b, "reason_id"), "二选一");
        assert!(has(&a, "sign"), "APP 端 GET 同样要签名");
    }

    #[test]
    fn relation_mod_splits_parameters_between_query_and_form() {
        let w = relation_mod_request(123, 5, 11, "test-ua");
        let qkeys: Vec<&str> = w.query.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(qkeys, ["statistics", "x-bili-device-req-json"]);
        assert_eq!(
            keys(&w),
            [
                "fid",
                "act",
                "re_src",
                "gaia_source",
                "spmid",
                "extend_content"
            ]
        );
        let form = w.form_fields().expect("表单体");
        assert_eq!(
            value(form, "extend_content"),
            r#"{"entity":"user","entity_id":123,"fp":"test-ua"}"#,
            "extend_content 的字段顺序与取值都被服务端看着"
        );
        assert_eq!(w.csrf, CsrfIn::Body);
    }

    #[test]
    fn room_entry_action_is_json_with_csrf_on_the_query() {
        let w = room_entry_action_request(123);
        assert_eq!(w.csrf, CsrfIn::Query, "这条的 csrf 在查询串上");
        assert_eq!(w.form_fields(), None, "这条是 JSON 体，不是表单");
        assert_eq!(w.form_string(), r#"{"platform":"pc","room_id":123}"#);
    }

    /// 心跳的两个分支：课程发 aid，其余发 bvid；空值整个键不出现。
    #[test]
    fn heart_beat_picks_aid_for_pugv_and_bvid_otherwise() {
        let ugc = heart_beat_request(&HeartBeatParams {
            bvid: Some("BV1xx411c7mD".to_owned()),
            cid: 42,
            video_type: 3,
            played_time: 100,
            ..HeartBeatParams::default()
        });
        assert_eq!(keys(&ugc), ["bvid", "cid", "type", "played_time"]);

        let pugv = heart_beat_request(&HeartBeatParams {
            aid: Some(2),
            bvid: Some("BV1xx411c7mD".to_owned()),
            cid: 42,
            epid: Some(7),
            season_id: Some(8),
            sub_type: Some(1),
            video_type: 10,
            played_time: 100,
            is_pugv: true,
        });
        assert_eq!(
            keys(&pugv),
            [
                "aid",
                "cid",
                "epid",
                "sid",
                "type",
                "sub_type",
                "played_time"
            ],
            "课程走 aid，bvid 即便给了也不发"
        );
    }

    #[test]
    fn heart_beat_without_an_identifier_still_builds() {
        // 基线的 `?aid` 在 aid 为 null 时整个键不出现，于是请求里只有
        // cid。照搬——这条由服务端拒绝，不在客户端判。
        let w = heart_beat_request(&HeartBeatParams {
            cid: 42,
            video_type: 3,
            played_time: 0,
            ..HeartBeatParams::default()
        });
        assert_eq!(keys(&w), ["cid", "type", "played_time"]);
    }

    #[test]
    fn season_update_adds_status_and_the_others_do_not() {
        assert_eq!(keys(&pgc_add_request(123)), ["season_id"]);
        assert_eq!(keys(&pgc_del_request(123)), ["season_id"]);
        assert_eq!(keys(&pgc_update_request("123", 2)), ["season_id", "status"]);
        assert_eq!(
            pgc_update_request("1,2,3", 2).form_string(),
            "season_id=1,2,3&status=2",
            "批量改追番状态是一串逗号分隔的 id，不是一个数字"
        );
    }

    #[test]
    fn every_write_in_this_slice_is_graded() {
        // ADR-013 决策二要的是「漏填编译不过」，这条只是把本片的定级一次
        // 性列出来，改级别时有个地方会红。
        for (w, expected) in [
            (ugc_triple_request(2), Idempotency::Verifiable),
            (pgc_triple_request(2), Idempotency::Verifiable),
            (coin_request(2, 1, 0, &auth()), Idempotency::Verifiable),
            (like_request(2, true, &auth()), Idempotency::Idempotent),
            (dislike_request(2, true, &auth()), Idempotency::Idempotent),
            (
                relation_mod_request(1, 1, 11, "ua"),
                Idempotency::Idempotent,
            ),
            (reply_del_request(1, 2, 3), Idempotency::Idempotent),
            (pgc_add_request(1), Idempotency::Idempotent),
            (history_report_request(2, 3), Idempotency::Idempotent),
            (media_list_history_request(1, 2, 3), Idempotency::Idempotent),
            (room_entry_action_request(1), Idempotency::Idempotent),
        ] {
            assert_eq!(w.idempotency, expected, "{} 的定级变了", w.path);
        }
    }

    #[test]
    fn ugc_triple_reports_each_action_separately() {
        // 部分成功是正常结果：三连不是原子操作，是三件事的回执。
        let body =
            r#"{"code":0,"message":"0","data":{"like":true,"coin":false,"fav":true,"multiply":2}}"#;
        let t = parse_ugc_triple(body).expect("应解析成功");
        assert_eq!(t.like, Some(true));
        assert_eq!(t.coin, Some(false), "投币没成不代表整次调用失败");
        assert_eq!(t.multiply, Some(2));
    }

    #[test]
    fn a_missing_field_is_none_not_an_error() {
        let t = parse_ugc_triple(r#"{"code":0,"message":"0","data":{}}"#).expect("不是错误");
        assert_eq!(t, UgcTriple::default());
    }

    #[test]
    fn pgc_triple_tolerates_a_float_where_an_int_is_expected() {
        // 基线的模型声明成 `num?`，float 是合法输入。直接用 i64 收会让
        // 整次三连显示为失败——而它其实成功了。
        let body =
            r#"{"code":0,"message":"0","data":{"coin":2.0,"like":1,"favorite":1,"relation":true}}"#;
        let t = parse_pgc_triple(body).expect("应解析成功");
        assert_eq!(t.coin, Some(2));
        assert_eq!(t.like, Some(1));
        assert_eq!(t.relation, Some(true));
    }

    #[test]
    fn like_toast_comes_from_data_and_season_toast_from_result() {
        assert_eq!(
            parse_like_toast(r#"{"code":0,"message":"0","data":{"toast":"已赞"}}"#)
                .expect("应解析成功"),
            Some("已赞".to_owned())
        );
        assert_eq!(
            parse_season_toast(r#"{"code":0,"message":"0","result":{"toast":"追番成功"}}"#)
                .expect("应解析成功"),
            Some("追番成功".to_owned())
        );
    }

    #[test]
    fn a_missing_toast_is_not_a_failure() {
        // 点赞成功了，只是没给文案。兜底文案是 i18n 的事，留在宿主。
        assert_eq!(
            parse_like_toast(r#"{"code":0,"message":"0","data":{}}"#).expect("不是错误"),
            None
        );
    }

    #[test]
    fn a_malformed_bvid_fails_before_the_request_is_built() {
        // 转错一位的后果是点赞点到别人的稿件上，所以这里宁可失败。
        let err = aid_from_bvid("not-a-bvid").expect_err("非法 BV 号必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
        assert_eq!(aid_from_bvid("BV1xx411c7mD").expect("合法 BV 号"), 2);
    }

    #[test]
    fn a_non_json_dislike_response_is_remote_not_protocol() {
        // 基线见过这种响应（`res.data is! String` 那条判断），多半是网关
        // 层的拒绝页。归 Protocol 会读成「协议变了该改代码」，不对。
        let err = check_dislike_response("<html>blocked</html>").expect_err("非 JSON 必须失败");
        assert_eq!(err.kind(), ErrorKind::Remote);
        assert!(err.detail().contains("blocked"), "原文要能看见: {err:?}");
    }

    #[test]
    fn a_normal_dislike_response_still_goes_through_the_envelope() {
        check_dislike_response(r#"{"code":0,"message":"0"}"#).expect("应当成功");
        let err =
            check_dislike_response(r#"{"code":-101,"message":"账号未登录"}"#).expect_err("应失败");
        assert_eq!(err.code(), Some(-101));
    }
}
