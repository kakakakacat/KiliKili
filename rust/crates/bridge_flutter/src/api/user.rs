//! 「我的」那一族的 Dart 导出（S7 / issue #57）。
//!
//! 基线对应的 `UserHttp` 十四个函数已随本次迁移删除，按 ADR-011 决策四
//! 不留降级开关。
//!
//! ## 登录用户信息还负责本地缓存的序列化
//!
//! 基线把 `UserInfoData` 整个对象用 Hive 适配器存盘。迁移之后导出结构是
//! 不可变的 typed 值，Hive 适配器无从生成；按仓库所有者的决定，缓存改存
//! **一段 JSON**，由 [`serialize_user_info`] / [`parse_user_info_cache`]
//! 这一对负责——JSON 的形状因此只有 Rust 知道，Dart 侧拿到的始终是
//! `UserInfo`，不会出现两边各自拼键名的情况。
//!
//! 改名/换头像之后要更新缓存，用 [`user_info_with_profile`]：构造一个只改
//! 两个字段的新值。让 Dart 去填齐八个字段才能「改一个」是个会漏字段的写法。
//!
//! ## 账号
//!
//! 这一族在 `ApiType.apiTypeSet` 里都没有登记，与基线拦截器一样走 `main`。
//! 硬币数那条挂在 `account.bilibili.com`，由 Dart 传不同的 `base_url`。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_api::user;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;

use flutter_rust_bridge::frb;

use crate::api::init;
use crate::api::transport::TransportSettings;
use crate::api::write::WriteResult;

/// 登录用户信息。
///
/// 只有八个字段：基线那 25 个里，其余 17 个在整个 `lib/` 没有任何消费者
/// （理由见 `bili_domain::user::UserInfo`）。
#[derive(Debug, Clone)]
pub struct UserInfo {
    /// 是否已登录。这条接口未登录时也回 `code 0`，「账号失效」靠它判断，
    /// 不是错误码。
    pub is_login: Option<bool>,
    pub mid: Option<i64>,
    pub uname: Option<String>,
    pub face: Option<String>,
    /// 硬币数，服务端给的就是小数。
    pub money: Option<f64>,
    pub level_info: Option<LevelInfo>,
    /// `> 0` 为大会员。
    pub vip_status: Option<i64>,
    /// `2` 是年度大会员。
    pub vip_type: Option<i64>,
    /// `1` 为硬核会员。
    pub is_senior_member: Option<i64>,
}

/// 等级与经验。`next_exp` 在满级时已换成 `current_exp`（见领域层）。
#[derive(Debug, Clone)]
pub struct LevelInfo {
    pub current_level: Option<i64>,
    pub current_min: Option<i64>,
    pub current_exp: Option<i64>,
    pub next_exp: Option<i64>,
}

/// 「我的」页三个计数。
#[derive(Debug, Clone)]
pub struct UserStat {
    pub following: Option<i64>,
    pub follower: Option<i64>,
    pub dynamic_count: Option<i64>,
}

/// 硬币/经验流水里的一条。三个字段都是服务端排好版的展示串。
#[derive(Debug, Clone)]
pub struct CoinLogItem {
    pub time: String,
    pub delta: String,
    pub reason: String,
}

/// 登录日志里的一条。
#[derive(Debug, Clone)]
pub struct LoginLogItem {
    pub ip: String,
    pub time_at: String,
    pub geo: String,
}

/// up 主实名信息。
#[derive(Debug, Clone)]
pub struct UserRealName {
    pub name: Option<String>,
    pub reject_page: Option<RejectPage>,
}

/// 实名未通过的说明页。
#[derive(Debug, Clone)]
pub struct RejectPage {
    pub title: Option<String>,
    pub text: Option<String>,
}

/// 空间隐私设置的一项。标题由界面按 `key` 查表，不在这里。
#[derive(Debug, Clone)]
pub struct PrivacySetting {
    pub key: String,
    pub value: Option<i64>,
    /// 为真时 `value == 0` 才算「开」。读错它就是把隐私设置显示反。
    pub reversed: bool,
    /// 界面上的第几组（1/2/3）。
    pub group: i64,
}

/// 订阅的收藏夹/合集，一页。
#[derive(Debug, Clone)]
pub struct SubList {
    pub count: Option<i64>,
    pub has_more: Option<bool>,
    pub list: Vec<SubItem>,
}

/// 订阅里的一项。`kind` 对应协议的 `type`：11 收藏夹，21 合集。
#[derive(Debug, Clone)]
pub struct SubItem {
    pub id: Option<i64>,
    pub fid: Option<i64>,
    pub mid: Option<i64>,
    pub attr: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub upper_mid: Option<i64>,
    pub upper_name: Option<String>,
    pub upper_face: Option<String>,
    pub cover_type: Option<i64>,
    pub intro: Option<String>,
    pub ctime: Option<i64>,
    pub mtime: Option<i64>,
    pub state: Option<i64>,
    pub fav_state: Option<i64>,
    pub media_count: Option<i64>,
    pub view_count: Option<i64>,
    pub kind: Option<i64>,
    pub play_count: Option<i64>,
    pub collect_count: Option<i64>,
}

/// 视频的一个标签。
#[derive(Debug, Clone)]
pub struct VideoTag {
    pub tag_id: Option<i64>,
    pub tag_name: Option<String>,
    /// `bgm` / `topic` / 普通标签。
    pub tag_type: Option<String>,
    pub music_id: Option<String>,
}

macro_rules! result_type {
    ($(#[$meta:meta])* $name:ident, $data:ty) => {
        $(#[$meta])*
        #[derive(Debug, Clone)]
        pub struct $name {
            pub data: Option<$data>,
            pub error_kind: Option<String>,
            pub error_detail: Option<String>,
            pub error_code: Option<i64>,
        }
    };
}

result_type!(
    /// 登录用户信息的结果。
    UserInfoResult,
    UserInfo
);
result_type!(
    /// 三个计数的结果。
    UserStatResult,
    UserStat
);
result_type!(
    /// 硬币数的结果。
    ///
    /// 请求成功但服务端没给 `money` 时 `data` 为空且 `error_kind` 也为空
    /// ——与请求失败是两回事，界面显示「-」而不是报错。
    CoinResult,
    f64
);
result_type!(
    /// 流水的结果。两份流水共用。
    CoinLogResult,
    Vec<CoinLogItem>
);
result_type!(
    /// 登录日志的结果。
    LoginLogResult,
    Vec<LoginLogItem>
);
result_type!(
    /// 实名信息的结果。
    UserRealNameResult,
    UserRealName
);
result_type!(
    /// 隐私设置的结果。
    PrivacyResult,
    Vec<PrivacySetting>
);
result_type!(
    /// 订阅列表的结果。
    SubListResult,
    SubList
);
result_type!(
    /// 视频标签的结果。
    VideoTagsResult,
    Vec<VideoTag>
);

/// 一次请求所需的公共参数。
#[derive(Debug, Clone)]
pub struct UserContext {
    /// 多数接口是 `api.bilibili.com`；硬币数那条传 `account.bilibili.com`。
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    /// CSRF token。**只有写操作要传**（ADR-013 决策四）。
    pub csrf: Option<String>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// Unix 秒，由 Dart 注入。只有 APP 签名的实名那条用得上。
    pub ts: i64,
}

impl UserContext {
    fn split(self) -> (RequestContext, ReqwestTransport, i64) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            csrf: self.csrf.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.ts)
    }
}

// ------------------------------------------------------------------ 只读

/// 取登录用户信息。
pub async fn fetch_user_info(context: UserContext) -> UserInfoResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_user_info(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(info) => UserInfoResult {
            data: Some(user_info(info)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => UserInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取「我的」页三个计数。
pub async fn fetch_user_stat(context: UserContext) -> UserStatResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_user_stat(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(s) => UserStatResult {
            data: Some(UserStat {
                following: s.following,
                follower: s.follower,
                dynamic_count: s.dynamic_count,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => UserStatResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取硬币数。成功但服务端没给时 `data` 与 `error_kind` 同为空。
pub async fn fetch_coin(context: UserContext) -> CoinResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_coin(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(money) => CoinResult {
            data: money,
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => CoinResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取硬币流水。
pub async fn fetch_coin_log(context: UserContext) -> CoinLogResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_coin_log(&transport, &ctx).await })
        .await;
    coin_log_result(flatten(joined))
}

/// 取经验流水。与硬币流水同形，只换路径。
pub async fn fetch_exp_log(context: UserContext) -> CoinLogResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_exp_log(&transport, &ctx).await })
        .await;
    coin_log_result(flatten(joined))
}

/// 取登录日志。
pub async fn fetch_login_log(context: UserContext) -> LoginLogResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_login_log(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(list) => LoginLogResult {
            data: Some(
                list.into_iter()
                    .map(|i| LoginLogItem {
                        ip: i.ip,
                        time_at: i.time_at,
                        geo: i.geo,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LoginLogResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取 up 主实名信息。APP 签名；`appkey`/`appsec` 由 Dart 传入。
pub async fn fetch_user_real_name(
    context: UserContext,
    access_key: String,
    up_mid: i64,
    appkey: String,
    appsec: String,
) -> UserRealNameResult {
    let (ctx, transport, ts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            user::fetch_user_real_name(&transport, &ctx, &access_key, up_mid, &appkey, &appsec, ts)
                .await
        })
        .await;

    match flatten(joined) {
        Ok(r) => UserRealNameResult {
            data: Some(UserRealName {
                name: r.name,
                reject_page: r.reject_page.map(|p| RejectPage {
                    title: p.title,
                    text: p.text,
                }),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => UserRealNameResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取空间隐私设置。返回十五项，顺序与分组即界面上的顺序。
pub async fn fetch_space_setting(context: UserContext, mid: i64) -> PrivacyResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_space_setting(&transport, &ctx, mid).await })
        .await;

    match flatten(joined) {
        Ok(list) => PrivacyResult {
            data: Some(
                list.into_iter()
                    .map(|s| PrivacySetting {
                        key: s.key,
                        value: s.value,
                        reversed: s.reversed,
                        group: s.group,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PrivacyResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取订阅的收藏夹/合集一页。
pub async fn fetch_sub_list(
    context: UserContext,
    up_mid: i64,
    page: i64,
    page_size: i64,
) -> SubListResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_sub_list(&transport, &ctx, up_mid, page, page_size).await })
        .await;

    match flatten(joined) {
        Ok(l) => SubListResult {
            data: Some(SubList {
                count: l.count,
                has_more: l.has_more,
                list: l
                    .list
                    .into_iter()
                    .map(|i| SubItem {
                        id: i.id,
                        fid: i.fid,
                        mid: i.mid,
                        attr: i.attr,
                        title: i.title,
                        cover: i.cover,
                        upper_mid: i.upper_mid,
                        upper_name: i.upper_name,
                        upper_face: i.upper_face,
                        cover_type: i.cover_type,
                        intro: i.intro,
                        ctime: i.ctime,
                        mtime: i.mtime,
                        state: i.state,
                        fav_state: i.fav_state,
                        media_count: i.media_count,
                        view_count: i.view_count,
                        kind: i.kind,
                        play_count: i.play_count,
                        collect_count: i.collect_count,
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SubListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取视频标签。没有标签的稿件返回空列表，不是失败。
pub async fn fetch_video_tags(
    context: UserContext,
    bvid: String,
    cid: Option<i64>,
) -> VideoTagsResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::fetch_video_tags(&transport, &ctx, &bvid, cid).await })
        .await;

    match flatten(joined) {
        Ok(list) => VideoTagsResult {
            data: Some(
                list.into_iter()
                    .map(|t| VideoTag {
                        tag_id: t.tag_id,
                        tag_name: t.tag_name,
                        tag_type: t.tag_type,
                        music_id: t.music_id,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => VideoTagsResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// -------------------------------------------------------------------- 写

/// 领大会员经验。按天只记一次，重发落到同一状态。
pub async fn vip_exp_add(context: UserContext, mid: i64) -> WriteResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::vip_exp_add(&transport, &ctx, mid).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

/// 批量改空间隐私设置。整批是置为语义。
pub async fn space_setting_mod(context: UserContext, values: Vec<(String, String)>) -> WriteResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::space_setting_mod(&transport, &ctx, values).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

/// 举报一条动态。**失败不自动重发**：重发会多提交一次举报。
pub async fn dynamic_report(
    context: UserContext,
    mid: i64,
    dyn_id: String,
    reason_type: i64,
    reason_desc: Option<String>,
) -> WriteResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            user::dynamic_report(
                &transport,
                &ctx,
                mid,
                &dyn_id,
                reason_type,
                reason_desc.as_deref(),
            )
            .await
        })
        .await;
    WriteResult::from_result(flatten(joined))
}

/// 预约/取消预约一个空间活动。
pub async fn space_reserve(context: UserContext, sid: i64, reserve: bool) -> WriteResult {
    let (ctx, transport, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { user::space_reserve(&transport, &ctx, sid, reserve).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

// ------------------------------------------------------- 本地缓存的序列化

/// 把登录用户信息序列化成一段 JSON，供 Dart 存进 Hive。
///
/// JSON 的形状由 `bili_api::user` 定义，Dart 侧只负责把这段串原样存取。
/// 失败返回 `None`——缓存写不成不是错误。
#[must_use]
#[frb(sync)]
pub fn serialize_user_info(info: UserInfo) -> Option<String> {
    bili_api::user::serialize_user_info_cache(&domain_user_info(info))
}

/// 把存盘的 JSON 解回登录用户信息。
///
/// 解不出来返回 `None`，调用方当作「没有缓存」。旧版本留下的是 Hive 的
/// 二进制帧而不是 JSON，走到这里同样是 `None`。
#[must_use]
#[frb(sync)]
pub fn parse_user_info_cache(json: String) -> Option<UserInfo> {
    bili_api::user::parse_user_info_cache(&json).map(user_info)
}

/// 改名/换头像之后的缓存更新：只改两个字段，其余原样。
///
/// 不让 Dart 自己 `UserInfo(...)` 重建：那要填齐八个字段，加字段时一定
/// 漏，而漏掉的字段会悄悄从缓存里消失。
#[must_use]
#[frb(sync)]
pub fn user_info_with_profile(
    info: UserInfo,
    uname: Option<String>,
    face: Option<String>,
) -> UserInfo {
    UserInfo {
        uname: uname.or(info.uname),
        face: face.or(info.face),
        ..info
    }
}

/// 导出结构 → 领域实体。只在缓存序列化这一条路径上用到。
fn domain_user_info(i: UserInfo) -> bili_domain::user::UserInfo {
    bili_domain::user::UserInfo {
        is_login: i.is_login,
        mid: i.mid,
        uname: i.uname,
        face: i.face,
        money: i.money,
        level_info: i.level_info.map(|l| bili_domain::user::LevelInfo {
            current_level: l.current_level,
            current_min: l.current_min,
            current_exp: l.current_exp,
            next_exp: l.next_exp,
        }),
        vip_status: i.vip_status,
        vip_type: i.vip_type,
        is_senior_member: i.is_senior_member,
    }
}

fn user_info(i: bili_domain::user::UserInfo) -> UserInfo {
    UserInfo {
        is_login: i.is_login,
        mid: i.mid,
        uname: i.uname,
        face: i.face,
        money: i.money,
        level_info: i.level_info.map(|l| LevelInfo {
            current_level: l.current_level,
            current_min: l.current_min,
            current_exp: l.current_exp,
            next_exp: l.next_exp,
        }),
        vip_status: i.vip_status,
        vip_type: i.vip_type,
        is_senior_member: i.is_senior_member,
    }
}

fn coin_log_result(r: Result<Vec<bili_domain::user::CoinLogItem>, DomainError>) -> CoinLogResult {
    match r {
        Ok(list) => CoinLogResult {
            data: Some(
                list.into_iter()
                    .map(|i| CoinLogItem {
                        time: i.time,
                        delta: i.delta,
                        reason: i.reason,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => CoinLogResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 把 `JoinHandle` 的两层结果压平。任务 panic 不传给 Dart。
fn flatten<T>(
    joined: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    match joined {
        Ok(inner) => inner,
        Err(e) => Err(DomainError::new(
            bili_domain::error::ErrorKind::Network,
            format!("取数任务未完成: {e}"),
        )),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        LevelInfo, UserInfo, parse_user_info_cache, serialize_user_info, user_info_with_profile,
    };

    fn sample() -> UserInfo {
        UserInfo {
            is_login: Some(true),
            mid: Some(1),
            uname: Some("甲".to_owned()),
            face: Some("https://example.invalid/a.jpg".to_owned()),
            money: Some(12.5),
            level_info: Some(LevelInfo {
                current_level: Some(5),
                current_min: Some(10800),
                current_exp: Some(11000),
                next_exp: Some(28800),
            }),
            vip_status: Some(1),
            vip_type: Some(2),
            is_senior_member: Some(1),
        }
    }

    #[test]
    fn cache_round_trips() {
        let json = serialize_user_info(sample()).expect("可序列化");
        let back = parse_user_info_cache(json).expect("可解回");
        assert_eq!(back.mid, Some(1));
        assert_eq!(back.uname.as_deref(), Some("甲"));
        assert_eq!(back.money, Some(12.5));
        assert_eq!(back.level_info.expect("有等级").next_exp, Some(28800));
        assert_eq!(back.is_senior_member, Some(1));
    }

    #[test]
    fn a_non_json_cache_is_treated_as_absent() {
        // 旧版本存进去的是 Hive 的二进制帧，不是 JSON。
        assert!(parse_user_info_cache("\u{1}\u{2}not json".to_owned()).is_none());
    }

    #[test]
    fn profile_update_touches_only_the_two_fields() {
        let updated = user_info_with_profile(sample(), Some("乙".to_owned()), None);
        assert_eq!(updated.uname.as_deref(), Some("乙"));
        assert_eq!(
            updated.face.as_deref(),
            Some("https://example.invalid/a.jpg"),
            "没传新头像就保持原样"
        );
        assert_eq!(updated.money, Some(12.5), "其余字段一个都不能丢");
        assert_eq!(updated.vip_type, Some(2));
    }
}
