//! 封面底部文案遮罩配色的 Dart 导出（issue #108）。
//!
//! 纯计算、没有 I/O，所以是 `#[frb(sync)]`：调用点在列表项的图片解码回调
//! 里，输入是一张十几像素见方的缩略图（几百个像素），算一次是微秒级。改
//! 成 Future 只会让每张封面多排一次事件循环，反而更容易在滚动时掉帧。
//!
//! 算法在 `bili_core::cover_scrim`，本模块只做类型转换（RFC-0001 §3：桥
//! 接层不承载业务逻辑）。

use bili_core::cover_scrim;
use flutter_rust_bridge::frb;

/// 一张封面算出来的底部遮罩配色。颜色是 `0xAARRGGBB`，Dart 侧直接
/// `Color(value)`。
///
/// [`CoverScrimStyle::scrim_argb`] 是**一整块纯色半透明**，不是渐变的一端：
/// 宿主拿它铺满文案那一条即可。
#[derive(Debug, Clone, Copy)]
pub struct CoverScrimStyle {
    /// 文案那一条的遮罩色，含 alpha。取自封面底部区域的主色，按对比度需要
    /// 调过明暗。
    pub scrim_argb: u32,
    pub title_argb: u32,
    pub subtitle_argb: u32,
}

impl From<cover_scrim::CoverScrim> for CoverScrimStyle {
    fn from(v: cover_scrim::CoverScrim) -> Self {
        Self {
            scrim_argb: v.scrim_argb,
            title_argb: v.title_argb,
            subtitle_argb: v.subtitle_argb,
        }
    }
}

/// 按封面缩略图算遮罩配色。
///
/// `rgba` 长度必须是 `width * height * 4`。对不上时返回
/// [`fallback_cover_scrim`] 的同一组值——封面还没解码完是正常路径，不是
/// 错误，没有必要让调用方分支。
#[frb(sync)]
#[must_use]
pub fn compute_cover_scrim(rgba: Vec<u8>, width: u32, height: u32) -> CoverScrimStyle {
    cover_scrim::cover_scrim(&rgba, width, height).into()
}

/// 封面缺失 / 加载中 / 解码失败时的固定回退配色。
#[frb(sync)]
#[must_use]
pub fn fallback_cover_scrim() -> CoverScrimStyle {
    cover_scrim::fallback_scrim().into()
}
