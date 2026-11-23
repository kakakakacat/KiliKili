//! 图片上传（ADR-015 / issue #193）。
//!
//! `lib/http/msg.dart` 剩下的两条。它们**不属于消息域**，是历史上顺手放进
//! 那个文件的，所以没跟着 S12（#63）一起迁；真正拦住它们的是一项传输层
//! 当时还没有的能力：multipart 与上传取消。
//!
//! | 接口 | 路径 | 文件字段 | 调用点 |
//! | --- | --- | --- | --- |
//! | [`upload_image_request`] | `/x/upload/web/image` | `file` | 收藏夹封面 |
//! | [`upload_bfs_request`] | `/x/dynamic/feed/draw/upload_bfs` | `file_up` | 动态配图、投票选项配图、私信图片 |
//!
//! **两条的文件字段名不一样**，给错的表现是服务端回 `code 0` 却没有 url
//! ——不报错，界面上只是「传完了但图没了」。
//!
//! ## 幂等定级：两条都是 `Unsafe`
//!
//! 按 ADR-013 决策三的三问：
//!
//! 1. 真的发请求吗？是。
//! 2. 服务端语义是「置为」还是「加一次」？**加一次**——重复上传同一张图会
//!    在服务端多占一份存储。
//! 3. 重发前查得到上一次生没生效吗？**查不到**。每次返回的 url 都不同，
//!    调用方无从判定。
//!
//! → `Unsafe`，于是传输层一律不重试（ADR-013 决策一）。这同时把 #193 担心
//! 的「取消过的请求被重试补发」在类型上消掉了，不需要为取消单写一条规则。
//!
//! ## 响应形状的一处不对称
//!
//! 两条接口的响应**不一样**，不要合并成一个实体：
//!
//! - `upload_image` 回 `{"location": "..."}`，调用点只取这一个串；
//! - `upload_bfs` 回 `{"image_url","image_width","image_height","img_size"}`，
//!   调用点要把宽高和大小一起塞进动态的图片描述里。
//!
//! 合成一个带四个 `Option` 的结构，等于让调用点自己猜哪几个在这条路径上
//! 有值。

use bili_domain::error::DomainError;
use bili_domain::upload::UploadedImage;
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{CsrfIn, PreparedWrite, UploadFile, WriteBody};

pub mod path {
    /// 通用图片上传（收藏夹封面）。
    pub const UPLOAD_IMAGE: &str = "/x/upload/web/image";
    /// 动态/私信配图上传。
    pub const UPLOAD_BFS: &str = "/x/dynamic/feed/draw/upload_bfs";
}

/// 收藏夹封面上传。
///
/// `bucket` 与 `dir` 由调用点给（当前只有 `medialist` / `cover` 一组），
/// 不在这里写死——它们是业务参数，不是协议常量。
#[must_use]
pub fn upload_image_request(file_path: &str, bucket: &str, dir: &str) -> PreparedWrite {
    PreparedWrite {
        path: path::UPLOAD_IMAGE,
        query: Vec::new(),
        body: WriteBody::Multipart {
            // 顺序与基线一致：bucket、file、dir、csrf。服务端不关心，但
            // 请求快照钉住了它，换序只会让 diff 变成噪音。
            fields: vec![
                ("bucket".to_owned(), bucket.to_owned()),
                ("dir".to_owned(), dir.to_owned()),
            ],
            file: UploadFile {
                field: "file",
                path: file_path.to_owned(),
            },
        },
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Unsafe,
    }
}

/// 动态/私信配图上传。
///
/// `category` 与 `biz` 可省：基线用 `?:` 把 null 的那一项整条从表单里拿掉，
/// **不是**传空串。三个调用点给的 `biz` 各不相同（`new_dyn` / `vote` /
/// `im`），服务端按它决定图片进哪个桶。
#[must_use]
pub fn upload_bfs_request(
    file_path: &str,
    category: Option<&str>,
    biz: Option<&str>,
) -> PreparedWrite {
    let mut fields = Vec::new();
    if let Some(category) = category {
        fields.push(("category".to_owned(), category.to_owned()));
    }
    if let Some(biz) = biz {
        fields.push(("biz".to_owned(), biz.to_owned()));
    }
    PreparedWrite {
        path: path::UPLOAD_BFS,
        query: Vec::new(),
        body: WriteBody::Multipart {
            fields,
            file: UploadFile {
                field: "file_up",
                path: file_path.to_owned(),
            },
        },
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Unsafe,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UploadImageDto {
    location: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UploadBfsDto {
    image_url: Option<String>,
    /// 宽高与大小都是展示/排版用的数字，按 #166 的判据放宽。
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    image_width: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    image_height: Option<i64>,
    /// **浮点**，不要收成整数：基线收的是 double，而且这个值会原样发回
    /// 发布接口。见 [`UploadedImage::size`]。
    img_size: Option<f64>,
}

/// 解析收藏夹封面上传的响应。
///
/// # Errors
/// 信封失败沿用统一分类；`location` 缺失判协议错误——上传「成功」却没有
/// 地址，调用点拿它当封面，静默落空会变成一张空白封面。
pub fn parse_upload_image(body: &str) -> Result<String, DomainError> {
    let dto: UploadImageDto = crate::envelope::unwrap_envelope(body)?;
    dto.location.ok_or_else(|| {
        DomainError::new(
            bili_domain::error::ErrorKind::Protocol,
            "上传成功但响应里没有 location",
        )
    })
}

/// 解析动态配图上传的响应。
///
/// # Errors
/// 同 [`parse_upload_image`]，缺 `image_url` 判协议错误。宽高允许缺失：
/// 它们只影响排版，而地址缺了这张图就不存在。
pub fn parse_upload_bfs(body: &str) -> Result<UploadedImage, DomainError> {
    let dto: UploadBfsDto = crate::envelope::unwrap_envelope(body)?;
    let url = dto.image_url.ok_or_else(|| {
        DomainError::new(
            bili_domain::error::ErrorKind::Protocol,
            "上传成功但响应里没有 image_url",
        )
    })?;
    Ok(UploadedImage {
        url,
        width: dto.image_width,
        height: dto.image_height,
        size: dto.img_size,
    })
}

/// 传收藏夹封面。
///
/// # Errors
/// 文件打不开或超过本地上限、传输失败、信封失败。被取消时是
/// [`bili_domain::error::ErrorKind::Cancelled`]。
pub async fn send_upload_image<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    file_path: &str,
    bucket: &str,
    dir: &str,
) -> Result<String, DomainError> {
    let req = upload_image_request(file_path, bucket, dir);
    let body = crate::client::send_write(transport, ctx, &req).await?;
    parse_upload_image(&body)
}

/// 传动态/私信配图。
///
/// # Errors
/// 同 [`send_upload_image`]。
pub async fn send_upload_bfs<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    file_path: &str,
    category: Option<&str>,
    biz: Option<&str>,
) -> Result<UploadedImage, DomainError> {
    let req = upload_bfs_request(file_path, category, biz);
    let body = crate::client::send_write(transport, ctx, &req).await?;
    parse_upload_bfs(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        parse_upload_bfs, parse_upload_image, path, upload_bfs_request, upload_image_request,
    };
    use crate::http::Idempotency;
    use crate::request::{UploadFile, WriteBody};
    use bili_domain::error::ErrorKind;

    fn file_of(body: &WriteBody) -> &UploadFile {
        match body {
            WriteBody::Multipart { file, .. } => file,
            _ => panic!("上传必须是 multipart 体"),
        }
    }

    /// 两条接口的文件字段名**不一样**。给错不报错，只是图没了——所以钉住。
    #[test]
    fn the_two_endpoints_use_different_file_fields() {
        assert_eq!(
            file_of(&upload_image_request("/t.jpg", "b", "d").body).field,
            "file"
        );
        assert_eq!(
            file_of(&upload_bfs_request("/t.jpg", None, None).body).field,
            "file_up"
        );
    }

    /// 上传一律 `Unsafe`：重发会多占一份存储，而且查不到上一次成没成。
    /// 这条同时决定了它永远不会被重试补发（ADR-013 决策一）。
    #[test]
    fn uploads_are_always_unsafe() {
        assert_eq!(
            upload_image_request("/t.jpg", "b", "d").idempotency,
            Idempotency::Unsafe
        );
        assert_eq!(
            upload_bfs_request("/t.jpg", None, None).idempotency,
            Idempotency::Unsafe
        );
    }

    /// 可省参数「整条不发」，不是传空串。
    #[test]
    fn bfs_omits_absent_params_instead_of_sending_blanks() {
        let bare = upload_bfs_request("/t.jpg", None, None);
        assert_eq!(bare.form_string(), "file_up=<file>");
        assert_eq!(bare.path, path::UPLOAD_BFS);

        let full = upload_bfs_request("/t.jpg", Some("daily"), Some("new_dyn"));
        assert_eq!(
            full.form_string(),
            "category=daily&biz=new_dyn&file_up=<file>"
        );
    }

    /// 请求快照里**不出现文件路径**：它带着作者机器的目录结构。
    #[test]
    fn the_snapshot_never_carries_the_file_path() {
        let req = upload_image_request("/home/someone/相册/IMG_0001.jpg", "medialist", "cover");
        let snapshot = req.form_string();
        assert!(!snapshot.contains("IMG_0001"), "快照不该带真实路径");
        assert!(!snapshot.contains("someone"));
        assert_eq!(snapshot, "bucket=medialist&dir=cover&file=<file>");
    }

    #[test]
    fn upload_image_takes_location_and_bfs_takes_the_four_fields() {
        let url = parse_upload_image(r#"{"code":0,"data":{"location":"https://x/a.jpg"}}"#)
            .expect("应解析成功");
        assert_eq!(url, "https://x/a.jpg");

        let img = parse_upload_bfs(
            r#"{"code":0,"data":{"image_url":"https://x/b.jpg","image_width":"1920","image_height":1080,"img_size":204.8}}"#,
        )
        .expect("应解析成功");
        assert_eq!(img.url, "https://x/b.jpg");
        assert_eq!(img.width, Some(1920), "字符串形态的尺寸照收");
        assert_eq!(img.height, Some(1080));
        assert_eq!(img.size, Some(204.8), "字节数是浮点，不截断");
    }

    /// 「成功但没有地址」要判失败：静默落空会变成一张空白封面，而那时
    /// 用户已经以为传好了。
    #[test]
    fn a_success_without_a_url_is_a_failure() {
        let err = parse_upload_image(r#"{"code":0,"data":{}}"#).expect_err("没有 location");
        assert_eq!(err.kind(), ErrorKind::Protocol);
        let err =
            parse_upload_bfs(r#"{"code":0,"data":{"image_width":1}}"#).expect_err("没有 image_url");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }
}
