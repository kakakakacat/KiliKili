//! 关注分组与关注关系的 Dart 导出（S6 #56 / S7 #57）。
//!
//! 基线对应的 `lib/http/member.dart` 的八个分组相关函数与 `lib/http/user.dart`
//! 的三个关系相关函数已随本次迁移删除，按 ADR-011 决策四不留降级开关。
//! 写操作尤其不能留降级：只读降级只是多发一次请求，写降级意味着可能重复
//! 提交（ADR-013 决策五）。
//!
//! ## 为什么 `followings` / `fans` 不在这里
//!
//! 它们在协议上同属 `/x/relation/*`，但 S15（#66）已经把它们连同
//! [`FollowList`] 一起落在了 [`crate::api::misc`]。把它们挪过来只能换到
//! 「代码按协议分类更整齐」，代价是整个 `frb_generated` 重排、`rust_misc.dart`
//! 的导出面变化，以及上游同步时多一个冲突点——AGENTS.md §9 明确不做这种
//! 顺手重构。本模块因此**复用** misc 的 [`FollowList`]，不另造一个同形结构。
//!
//! ## 账号
//!
//! 这一族在 `ApiType.apiTypeSet` 里都没有登记，与基线拦截器一样走 `main`。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::relation;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::misc::{FollowListResult, follow_list_result};
use crate::api::transport::TransportSettings;
use crate::api::write::WriteResult;

/// 一个关注分组。
#[derive(Debug, Clone)]
pub struct FollowTag {
    pub tagid: Option<i64>,
    pub name: Option<String>,
    /// 组内人数。
    pub count: Option<i64>,
    /// 服务端给的说明文案。
    pub tip: Option<String>,
}

/// 当前账号与某个人的关系。
#[derive(Debug, Clone)]
pub struct Relation {
    pub attribute: Option<i64>,
    pub mtime: Option<i64>,
    /// 这个人在哪几个分组里。**`None` 与空列表不是一回事**：没有分组是
    /// `None`，在默认分组是 `[0]`，分组弹窗据此决定默认勾选。
    ///
    /// 刻意收成 `i32`：FRB 把 `Vec<i64>` 映射成元素是 `PlatformInt64` 的
    /// `Int64List`，于是 Dart 侧拿到的是一串 `BigInt`，而这个列表要原样喂
    /// 给分组面板的 `List<int>`，每个元素都得再转一次。分组 id 是账号自己
    /// 建的那几十个，不可能溢出 `i32`。领域层仍是 `i64`
    /// （同样的取舍见 `crate::api::video_page::VideoShot::index`）。
    pub tag: Option<Vec<i32>>,
    pub special: Option<i64>,
}

/// 分组清单的结果。
#[derive(Debug, Clone)]
pub struct FollowTagsResult {
    pub data: Option<Vec<FollowTag>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 关系的结果。
#[derive(Debug, Clone)]
pub struct RelationResult {
    pub data: Option<Relation>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 新建分组的结果。成功时 `data` 是服务端分配的 `tagid`。
#[derive(Debug, Clone)]
pub struct CreatedTagResult {
    pub data: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 一次请求所需的公共参数。
///
/// 与 `MiscContext` 同形但各自独立：两个模块的参数集将来会分头演化
/// （本模块迟早要加上 APP 端签名那一组），共用一个结构等于让任一方的
/// 变化波及另一方。
#[derive(Debug, Clone)]
pub struct RelationContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    /// CSRF token。**只有写操作要传**，只读的传 `None`
    /// （ADR-013 决策四）。注意「只读却要 csrf」的那两条
    /// （`followed_upper` / `same/followings`）把 csrf 当成普通查询参数，
    /// 由 Dart 显式传进 `csrf` 形参，走的不是这里。
    pub csrf: Option<String>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// WBI mixin key。只有关注搜索签名，其余传空串即可。
    pub mixin_key: String,
    /// Unix 秒，由 Dart 注入。
    pub wts: i64,
}

impl RelationContext {
    fn split(self) -> (RequestContext, ReqwestTransport, String, i64) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            csrf: self.csrf.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.mixin_key, self.wts)
    }
}

// ------------------------------------------------------------------ 只读

/// 取关注分组清单。
pub async fn fetch_follow_up_tags(context: RelationContext) -> FollowTagsResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { relation::fetch_follow_up_tags(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(tags) => FollowTagsResult {
            data: Some(
                tags.into_iter()
                    .map(|t| FollowTag {
                        tagid: t.tagid,
                        name: t.name,
                        count: t.count,
                        tip: t.tip,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FollowTagsResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取某个分组下的一页人。
///
/// 结果的 `total` 恒为 `None`——这条接口的 `data` 是裸数组，不给总数。
/// 翻页靠「这一页还满不满」判断，与基线一致。
pub async fn fetch_follow_up_group(
    context: RelationContext,
    mid: Option<i64>,
    tagid: Option<i64>,
    page: Option<i64>,
    page_size: i64,
    order_type: String,
) -> FollowListResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            relation::fetch_follow_up_group(
                &transport,
                &ctx,
                mid,
                tagid,
                page,
                page_size,
                &order_type,
            )
            .await
        })
        .await;

    follow_list_result(flatten(joined))
}

/// 在自己的关注里按昵称搜索。WBI 签名。
pub async fn fetch_follow_search(
    context: RelationContext,
    vmid: i64,
    page: i64,
    page_size: i64,
    name: String,
) -> FollowListResult {
    let (ctx, transport, mixin_key, wts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            relation::fetch_follow_search(
                &transport, &ctx, vmid, page, page_size, &name, &mixin_key, wts,
            )
            .await
        })
        .await;

    follow_list_result(flatten(joined))
}

/// 取「他关注的 up 主」一页。`csrf` 由 Dart 的账号体系给出。
pub async fn fetch_followed_up(
    context: RelationContext,
    csrf: String,
    vmid: i64,
    page: i64,
) -> FollowListResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(
            async move { relation::fetch_followed_up(&transport, &ctx, vmid, page, &csrf).await },
        )
        .await;

    follow_list_result(flatten(joined))
}

/// 取「共同关注」一页。`page` 省略时服务端给第一页。
pub async fn fetch_same_following(
    context: RelationContext,
    csrf: String,
    vmid: i64,
    page: Option<i64>,
) -> FollowListResult {
    let (ctx, transport, _, _) = context.split();
    let joined =
        init::runtime()
            .spawn(async move {
                relation::fetch_same_following(&transport, &ctx, vmid, page, &csrf).await
            })
            .await;

    follow_list_result(flatten(joined))
}

/// 取与某个人的关系。
pub async fn fetch_relation(context: RelationContext, fid: i64) -> RelationResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { relation::fetch_relation(&transport, &ctx, fid).await })
        .await;

    match flatten(joined) {
        Ok(r) => RelationResult {
            data: Some(Relation {
                attribute: r.attribute,
                mtime: r.mtime,
                // 见字段文档：分组 id 不可能溢出 i32。真溢出了也只是这一个
                // 分组勾不上，不值得让整条请求失败。
                tag: r.tag.map(|ids| {
                    ids.into_iter()
                        .map(|id| i32::try_from(id).unwrap_or(i32::MAX))
                        .collect()
                }),
                special: r.special,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => RelationResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// -------------------------------------------------------------------- 写

/// 把若干人设置到若干分组。两个参数都是逗号分隔的 id 串。
pub async fn add_users_to_tags(
    context: RelationContext,
    fids: String,
    tagids: String,
) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { relation::add_users(&transport, &ctx, &fids, &tagids).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 新建分组，成功时带回服务端分配的 `tagid`。
pub async fn create_follow_tag(context: RelationContext, tag: String) -> CreatedTagResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { relation::create_follow_tag(&transport, &ctx, &tag).await })
        .await;

    match flatten(joined) {
        Ok(tagid) => CreatedTagResult {
            data: Some(tagid),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => CreatedTagResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 给分组改名。
pub async fn update_follow_tag(context: RelationContext, tagid: i64, name: String) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { relation::update_follow_tag(&transport, &ctx, tagid, &name).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 删除分组。
pub async fn del_follow_tag(context: RelationContext, tagid: i64) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { relation::del_follow_tag(&transport, &ctx, tagid).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 加/取消特别关注。
pub async fn special_action(context: RelationContext, fid: i64, add: bool) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { relation::special_action(&transport, &ctx, fid, add).await })
        .await;

    WriteResult::from_result(flatten(joined))
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
