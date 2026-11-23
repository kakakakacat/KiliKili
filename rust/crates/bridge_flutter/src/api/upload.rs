//! 图片上传的 typed FRB 导出（ADR-015 / issue #193）。
//!
//! ## 取消为什么长这样
//!
//! 形状照搬 ADR-014 给直播长连接定的那一套：`begin` **同步**返回一个单调
//! 递增的 id，联网在共享 runtime 里做；宿主拿着 id 再 `await` 或 `cancel`。
//! 因此**连接还没建立也能取消**。
//!
//! 没用 FRB 的 `DropGuard` / `CancelableFuture`：它们把取消绑在 Dart 侧
//! 对象的生命周期上，而这里的取消是**显式的用户动作**（退出发布页，或者
//! 同批里另一张图失败了要把其余几张一起停掉）。绑生命周期会让「什么时候
//! 真的停了」取决于 GC 时机，那是最难查的一类 bug。更要紧的是：仓库里
//! 已经有一套取消机制在跑，第二套的代价不是多写几行，是以后每个人都要
//! 先问「这里该用哪一套」。
//!
//! **取消过的上传不会被重试补发**，这一条不需要额外规则：上传一律
//! `Idempotency::Unsafe`，ADR-013 决策一规定它一律不重试。
//!
//! 被取消的上传以 `error_kind == "cancelled"` 返回，与传输层既有的取消
//! 分类一致；宿主据此不弹 toast。

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use bili_api::{http::RequestContext, transport_reqwest::ReqwestTransport, upload};
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;
use flutter_rust_bridge::frb;
use tokio::task::JoinHandle;

use crate::api::{init, transport::TransportSettings};

static NEXT_ID: AtomicI64 = AtomicI64::new(1);
static UPLOADS: OnceLock<Mutex<HashMap<i64, JoinHandle<UploadOutcome>>>> = OnceLock::new();
fn uploads() -> &'static Mutex<HashMap<i64, JoinHandle<UploadOutcome>>> {
    UPLOADS.get_or_init(Default::default)
}

/// 两条上传共用的一次产出：要么一个地址（+ 可选的尺寸），要么一个错误。
enum UploadOutcome {
    Image(Result<String, DomainError>),
    Bfs(Result<bili_domain::upload::UploadedImage, DomainError>),
}

#[derive(Debug, Clone)]
pub struct UploadContext {
    pub base_url: String,
    pub cookie: Option<String>,
    /// 两条上传都把 csrf 放在**表单字段**里（ADR-015 决策一）。
    pub csrf: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

impl UploadContext {
    fn split(self) -> (RequestContext, ReqwestTransport) {
        (
            RequestContext {
                base_url: self.base_url,
                cookie: self.cookie.map(Redacted::new),
                csrf: self.csrf.map(Redacted::new),
                headers: self.headers,
                timeout: Duration::from_millis(self.timeout_ms),
            },
            ReqwestTransport::new(self.transport.to_config()),
        )
    }
}

/// 一张传好的图。
#[derive(Debug, Clone)]
pub struct UploadedImageData {
    pub url: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub size: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct UploadImageResult {
    /// 收藏夹封面那条只回一个地址。
    pub data: Option<String>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct UploadBfsResult {
    pub data: Option<UploadedImageData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

fn cancelled_detail() -> (Option<String>, Option<String>) {
    (Some("cancelled".into()), Some("上传已取消".into()))
}

fn failed<T>(error: &DomainError) -> (Option<T>, Option<String>, Option<String>, Option<i64>) {
    (
        None,
        Some(error.kind().to_string()),
        Some(error.detail().into()),
        error.code(),
    )
}

/// 开始传收藏夹封面。**同步返回** `upload_id`，联网在后台跑。
///
/// `file_path` 必须是真实路径，不能是 Android 的 SAF uri——调用点要先把
/// 内容落成文件（ADR-015 决策二；基线 `MultipartFile.fromFile` 同样如此）。
#[frb(sync)]
pub fn upload_image_begin(
    context: UploadContext,
    file_path: String,
    bucket: String,
    dir: String,
) -> i64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (ctx, transport) = context.split();
    let handle = init::runtime().spawn(async move {
        UploadOutcome::Image(
            upload::send_upload_image(&transport, &ctx, &file_path, &bucket, &dir).await,
        )
    });
    uploads()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, handle);
    id
}

/// 开始传动态/私信配图。约定同 [`upload_image_begin`]。
#[frb(sync)]
pub fn upload_bfs_begin(
    context: UploadContext,
    file_path: String,
    category: Option<String>,
    biz: Option<String>,
) -> i64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (ctx, transport) = context.split();
    let handle = init::runtime().spawn(async move {
        UploadOutcome::Bfs(
            upload::send_upload_bfs(
                &transport,
                &ctx,
                &file_path,
                category.as_deref(),
                biz.as_deref(),
            )
            .await,
        )
    });
    uploads()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, handle);
    id
}

/// 取走一个上传的句柄。取走之后它就不在注册表里了，所以 `await` 与
/// `cancel` 天然互斥——先到的那个拿到句柄，后到的拿到 `None`。
fn take(id: i64) -> Option<JoinHandle<UploadOutcome>> {
    uploads()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id)
}

/// 等一个封面上传的结果。
pub async fn upload_image_await(upload_id: i64) -> UploadImageResult {
    let Some(handle) = take(upload_id) else {
        let (kind, detail) = cancelled_detail();
        return UploadImageResult {
            data: None,
            error_kind: kind,
            error_detail: detail,
            error_code: None,
        };
    };
    match handle.await {
        Ok(UploadOutcome::Image(Ok(url))) => UploadImageResult {
            data: Some(url),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Ok(UploadOutcome::Image(Err(error))) => {
            let (data, kind, detail, code) = failed(&error);
            UploadImageResult {
                data,
                error_kind: kind,
                error_detail: detail,
                error_code: code,
            }
        }
        // 另一种产出说明调用方把两条接口的 id 搞混了。
        Ok(UploadOutcome::Bfs(_)) => UploadImageResult {
            data: None,
            error_kind: Some("protocol".into()),
            error_detail: Some("上传句柄与接口不匹配".into()),
            error_code: None,
        },
        // JoinError：被 abort 了（即取消），或任务 panic。
        Err(e) if e.is_cancelled() => {
            let (kind, detail) = cancelled_detail();
            UploadImageResult {
                data: None,
                error_kind: kind,
                error_detail: detail,
                error_code: None,
            }
        }
        Err(_) => UploadImageResult {
            data: None,
            error_kind: Some("protocol".into()),
            error_detail: Some("上传工作任务未完成".into()),
            error_code: None,
        },
    }
}

/// 等一个配图上传的结果。
pub async fn upload_bfs_await(upload_id: i64) -> UploadBfsResult {
    let Some(handle) = take(upload_id) else {
        let (kind, detail) = cancelled_detail();
        return UploadBfsResult {
            data: None,
            error_kind: kind,
            error_detail: detail,
            error_code: None,
        };
    };
    match handle.await {
        Ok(UploadOutcome::Bfs(Ok(image))) => UploadBfsResult {
            data: Some(UploadedImageData {
                url: image.url,
                width: image.width,
                height: image.height,
                size: image.size,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Ok(UploadOutcome::Bfs(Err(error))) => {
            let (data, kind, detail, code) = failed(&error);
            UploadBfsResult {
                data,
                error_kind: kind,
                error_detail: detail,
                error_code: code,
            }
        }
        Ok(UploadOutcome::Image(_)) => UploadBfsResult {
            data: None,
            error_kind: Some("protocol".into()),
            error_detail: Some("上传句柄与接口不匹配".into()),
            error_code: None,
        },
        Err(e) if e.is_cancelled() => {
            let (kind, detail) = cancelled_detail();
            UploadBfsResult {
                data: None,
                error_kind: kind,
                error_detail: detail,
                error_code: None,
            }
        }
        Err(_) => UploadBfsResult {
            data: None,
            error_kind: Some("protocol".into()),
            error_detail: Some("上传工作任务未完成".into()),
            error_code: None,
        },
    }
}

/// 取消一个进行中的上传。
///
/// **重复取消、取消一个已经结束的、取消一个不存在的 id，都是成功**——与
/// ADR-014 的 `close` 一致。调用点常常在 `finally` 里兜底调一次，让它报错
/// 只会逼出一圈 try/catch。
#[frb(sync)]
pub fn upload_cancel(upload_id: i64) {
    if let Some(handle) = take(upload_id) {
        handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> UploadContext {
        UploadContext {
            base_url: "https://api.example.invalid".to_owned(),
            cookie: None,
            csrf: None,
            headers: Vec::new(),
            timeout_ms: 10_000,
            transport: TransportSettings {
                retry_count: 0,
                retry_delay_ms: 0,
                proxy_host: None,
                proxy_port: None,
                accept_invalid_certs: false,
                http1_only: false,
            },
        }
    }

    /// 取消一个没有的 id 不该 panic：调用点会在 `finally` 里兜底调它。
    #[test]
    fn cancelling_an_unknown_id_is_a_no_op() {
        upload_cancel(i64::MAX);
        upload_cancel(0);
    }

    /// 取消之后再 await，拿到的是 `cancelled` 而不是一个网络错误——宿主
    /// 据此不弹 toast。
    #[test]
    fn a_cancelled_upload_reports_cancelled_not_a_network_error() {
        let id = upload_bfs_begin(ctx(), "/no/such/file.jpg".to_owned(), None, None);
        upload_cancel(id);
        let result = init::runtime().block_on(upload_bfs_await(id));
        assert_eq!(result.error_kind.as_deref(), Some("cancelled"));
        assert!(result.data.is_none());
    }

    /// id 是单调递增的，两次 begin 不会撞上同一个槽位。
    #[test]
    fn ids_are_monotonic() {
        let a = upload_image_begin(ctx(), "/a.jpg".to_owned(), "b".to_owned(), "d".to_owned());
        let b = upload_image_begin(ctx(), "/b.jpg".to_owned(), "b".to_owned(), "d".to_owned());
        assert!(b > a);
        upload_cancel(a);
        upload_cancel(b);
    }

    /// 文件不存在时是 `protocol` 加一句能看懂的话，不是一个 IO 错误码。
    /// 这条失败在**发请求之前**就判掉（ADR-015 决策五）。
    #[test]
    fn a_missing_file_fails_before_any_request() {
        let id = upload_image_begin(
            ctx(),
            "/definitely/not/here.jpg".to_owned(),
            "medialist".to_owned(),
            "cover".to_owned(),
        );
        let result = init::runtime().block_on(upload_image_await(id));
        assert_eq!(result.error_kind.as_deref(), Some("protocol"));
        assert!(
            result
                .error_detail
                .as_deref()
                .is_some_and(|d| d.contains("打不开")),
            "文案要说明是文件打不开，而不是一个裸 errno"
        );
    }
}
