//! 上传业务实体（ADR-015 / issue #193）。

use serde::Serialize;

/// 一张传好的图。
///
/// **只对应 `upload_bfs` 那条**。收藏夹封面那条（`upload_image`）的响应里
/// 只有一个 `location`，调用点也只用那一个串，所以它直接返回 `String`，
/// 不套这个结构——合成一个带四个 `Option` 的实体，等于让调用点自己猜
/// 哪几个在这条路径上有值。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UploadedImage {
    pub url: String,
    /// 宽高与大小只影响动态里图片的排版，缺了不致命——所以是 `Option`，
    /// 而 `url` 不是。
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// 字节数，**是浮点不是整数**。
    ///
    /// 基线 `(json['img_size'] as num?)?.toDouble()` 收的就是 double，而且
    /// 这个值会**原样发回发布接口**（动态的图片描述里带着它）。截成整数
    /// 等于悄悄改了一个要发给服务端的字段，`204.8` 会变成 `204`——这种
    /// 改动不报错，也不会在界面上显出来。
    pub size: Option<f64>,
}
