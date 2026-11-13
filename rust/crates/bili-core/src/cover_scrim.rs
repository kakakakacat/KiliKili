//! 封面底部文案遮罩的取色与对比度收敛。
//!
//! ## 为什么在 Rust
//!
//! 这里全是阈值判断与收敛算法：明暗极性的选择、不透明度的求解、WCAG 对比
//! 度的校验。换掉宿主框架之后它们一条都不该重写（AGENTS.md 第 2 节）。宿
//! 主只负责两件平台相关的事——把封面解码成一张小图，以及把算出来的颜色画
//! 成渐变。
//!
//! ## 输入为什么是一张缩略图
//!
//! 列表里会同时出现几十张封面，按原图逐像素统计必然掉帧。宿主传进来的是
//! 一张十几像素见方的缩略图（见 `lib/common/widgets/video_card/`），统计
//! 量从百万级降到几百，且缩放本身已经做了一次低通，正好压掉单像素高光对
//! 分位数的干扰。
//!
//! ## 只看下半部分
//!
//! 遮罩只盖住封面底部，用整张图的平均色会被上半部分（常常是完全不同的主
//! 体）带偏。统计区间取底部 [`REGION_FROM`] 以下。

/// 一张封面算出来的底部遮罩配色。颜色一律是 `0xAARRGGBB`。
///
/// [`CoverScrim::scrim_argb`] 用在文案所在的半透明实色区，也用作其上方
/// 渐变段的终点。文字只能压在实色区；渐变中段的实际背景与这里计算对比度
/// 时用的背景不同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverScrim {
    /// 文案那一条的遮罩色，含 alpha。
    pub scrim_argb: u32,
    /// 标题色，不透明。
    pub title_argb: u32,
    /// 辅助信息（分区、日期…）色，不透明。
    pub subtitle_argb: u32,
    /// UP 名那块小药丸的底色，含 alpha，画在**遮罩之上**。
    ///
    /// 调用方给的品牌色按可读性需要调过不透明度：压得太实会让药丸上的字
    /// 掉出对比度，所以这里返回的 alpha 可能比想要的低，甚至为 0（等于不
    /// 画底色，退回纯遮罩——那一档的对比度是有保证的）。
    pub tint_argb: u32,
    /// 药丸上的文字色，不透明。
    pub tint_label_argb: u32,
}

/// 统计区间的上边界，占封面高度的比例。
const REGION_FROM: f32 = 0.55;

/// 标题要达到的 WCAG 对比度。4.5 是正文级别的 AA 线。
const MIN_TITLE_CONTRAST: f32 = 4.5;

/// 辅助信息要达到的对比度。它比标题小一号但也不是装饰，取 AA 的大字线 3.0。
const MIN_SUBTITLE_CONTRAST: f32 = 3.0;

/// UP 名药丸底色的不透明度上限。
///
/// 它是**色调**不是底色：压到不透明就成了一块实心色块，遮罩随封面变色的
/// 效果全被它盖掉，而且品牌色是固定的，在浅色遮罩上会非常跳。
const TINT_ALPHA_MAX: f32 = 0.42;

/// 药丸不透明度的回退步长。一路降到 0 必然达标——那时它就是纯遮罩。
const TINT_ALPHA_STEP: f32 = 0.06;

/// 遮罩不透明度的搜索区间。下限再低就盖不住杂色封面，上限再高遮罩本身会
/// 变成一条硬边，不如直接把遮罩调暗。
const ALPHA_MIN: f32 = 0.70;
const ALPHA_MAX: f32 = 0.95;
const ALPHA_STEP: f32 = 0.05;

/// 主色直方图每个通道分几档。4 档 = 64 个桶：再细会把同一片渐变打散成
/// 许多小桶，谁都不占优；再粗则把明显不同的颜色并成一个。
const DOMINANT_LEVELS: usize = 4;

/// 主色打分时给中性色的底分。
///
/// 得分 = 像素数 × (底分 + 饱和度)。纯按像素数会让背景的灰、黑边、白底
/// 永远赢；完全不看像素数又会让角落里一小块高饱和色当主色。0.35 的效果
/// 是：灰色区域要比彩色区域大一倍左右才赢得过它。
const DOMINANT_NEUTRAL_FLOOR: f32 = 0.35;

/// 判为「浅色封面」的门槛：底部区域**最暗的那一成**都还超过它，才认为整块
/// 底部是浅的、值得翻成浅遮罩 + 深文字。用分位数而不是平均值，是因为白底
/// 上压一行黑字的封面平均值也很高，但那行字正落在遮罩下面。
const LIGHT_COVER_P10: f32 = 0.55;

/// 遮罩的饱和度上限。完全保留封面的饱和度会让遮罩看起来像一块色块而不是
/// 阴影；完全去掉又丢了「随封面变化」。
const SCRIM_SAT_MAX: f32 = 0.45;

/// 明度每次挪动的步长。
///
/// 遮罩的明度**从封面底部的混合色自己的明度出发**，只在对比度不够时才一
/// 步步往需要的那一极挪。早先的写法是直接把明度钉到 0.14，结果不管什么
/// 封面算出来都接近纯黑——色相是对的，但人眼先看到的是明度，看上去就不
/// 像「封面的颜色」。现在深色封面几乎原样保留自己的颜色，只有浅色和中等
/// 明度的封面才会被压暗，而那正是不压就看不清字的情况。
const LIGHTNESS_STEP: f32 = 0.05;

/// 封面缺失、仍在加载、或解码失败时的固定回退。
///
/// 刻意不跟随主题：回退样式必须在拿不到任何封面信息时也成立，而深浅主题
/// 的判断在宿主侧。统一用深遮罩 + 白字，和占位图的底色一致。
///
/// `tint_rgb` 是 UP 名药丸的品牌色，`0xRRGGBB`（忽略 alpha，由这里算）。
#[must_use]
pub fn fallback_scrim(tint_rgb: u32) -> CoverScrim {
    const SCRIM: u32 = 0xD9_10_10_10;
    let composite = blend(
        unpack_rgb(SCRIM),
        ((SCRIM >> 24) & 0xFF) as f32 / 255.0,
        [0.0; 3],
    );
    let (tint_argb, tint_label_argb) = tinted_pill(tint_rgb, [1.0; 3], composite);
    CoverScrim {
        scrim_argb: SCRIM,
        title_argb: 0xFF_FF_FF_FF,
        subtitle_argb: 0xFF_D4_D4_D4,
        tint_argb,
        tint_label_argb,
    }
}

/// 解出 `0xRRGGBB` / `0xAARRGGBB` 的 RGB 三分量。
fn unpack_rgb(argb: u32) -> [f32; 3] {
    [
        ((argb >> 16) & 0xFF) as f32 / 255.0,
        ((argb >> 8) & 0xFF) as f32 / 255.0,
        (argb & 0xFF) as f32 / 255.0,
    ]
}

/// 算 UP 名药丸的底色与文字色。
///
/// 文字沿用标题色（纯白 / 近黑），只调底色的不透明度：让药丸上的字跟同一
/// 行的其它文字一个色，看起来才是一行；而且不透明度一路降到 0 就回到纯
/// 遮罩，那一档的对比度在上面已经保证过，所以这个搜索必然收敛。
fn tinted_pill(tint_rgb: u32, text: [f32; 3], scrim_composite: [f32; 3]) -> (u32, u32) {
    let tint = unpack_rgb(tint_rgb);
    let mut alpha = TINT_ALPHA_MAX;
    loop {
        let composite = blend(tint, alpha, scrim_composite);
        if contrast(text, composite) >= MIN_SUBTITLE_CONTRAST || alpha <= 0.0 {
            return (pack(tint, alpha), pack(text, 1.0));
        }
        alpha = (alpha - TINT_ALPHA_STEP).max(0.0);
    }
}

/// 按封面缩略图算出底部遮罩配色。
///
/// `rgba` 是逐像素的 RGBA8，长度必须是 `width * height * 4`；alpha 被忽略
/// （封面没有透明通道，占位图也是不透明的）。尺寸对不上或为空时返回
/// [`fallback_scrim`]——这是正常路径的一部分（图还没解码完），不是错误。
///
/// `tint_rgb` 是 UP 名药丸的品牌色，`0xRRGGBB`。它由宿主传进来而不是写死
/// 在这里：一个色值字面量属于宿主（AGENTS.md 第 2 节），**怎么让它在任意
/// 封面上都还读得清**才是这里的事。
#[must_use]
pub fn cover_scrim(rgba: &[u8], width: u32, height: u32, tint_rgb: u32) -> CoverScrim {
    let Some(samples) = sample_bottom(rgba, width, height) else {
        return fallback_scrim(tint_rgb);
    };
    scrim_for(&samples, tint_rgb)
}

/// 底部区域的采样结果。
struct BottomSamples {
    /// 区域主色，sRGB 0..1。遮罩的色相 / 饱和度 / 起始明度都来自它。
    ///
    /// 用主色而不是平均色：平均色把互补的颜色混在一起，彩色封面算出来往往
    /// 是一摊灰，看上去跟封面没关系。主色取的是「这块区域里占得最多的那种
    /// 颜色」，才是肉眼认为的「封面的颜色」。
    dominant: [f32; 3],
    /// 相对亮度的 10% 分位。白字可读性的最坏情况看它的反面，浅遮罩看它。
    p10: f32,
    /// 相对亮度的 90% 分位。深遮罩压白字时的最坏情况。
    ///
    /// 用分位数而不是极值：封面上一个镜面高光像素会把极值顶到 1.0，逼着
    /// 遮罩一路加到上限，而那一个像素根本盖不住任何一个字。
    p90: f32,
}

fn sample_bottom(rgba: &[u8], width: u32, height: u32) -> Option<BottomSamples> {
    if width == 0 || height == 0 {
        return None;
    }
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if rgba.len() != expected {
        return None;
    }
    // 至少留一行：极端小的缩略图（高 1px）也要能出结果。
    let from = ((height as f32 * REGION_FROM) as u32).min(height - 1);
    let row_start = (from as usize) * (width as usize) * 4;
    let region = &rgba[row_start..];

    let count = region.len() / 4;
    let mut lums: Vec<f32> = Vec::with_capacity(count);
    // 每个桶记 [r 和, g 和, b 和, 像素数]，最后取桶内均值当这个桶的颜色。
    const BUCKETS: usize = DOMINANT_LEVELS * DOMINANT_LEVELS * DOMINANT_LEVELS;
    let mut hist = [[0.0_f32; 4]; BUCKETS];
    for px in region.chunks_exact(4) {
        let rgb = [
            f32::from(px[0]) / 255.0,
            f32::from(px[1]) / 255.0,
            f32::from(px[2]) / 255.0,
        ];
        let idx = bucket_index(rgb);
        hist[idx][0] += rgb[0];
        hist[idx][1] += rgb[1];
        hist[idx][2] += rgb[2];
        hist[idx][3] += 1.0;
        lums.push(relative_luminance(rgb));
    }
    lums.sort_by(f32::total_cmp);
    Some(BottomSamples {
        dominant: dominant_color(&hist),
        p10: percentile(&lums, 0.10),
        p90: percentile(&lums, 0.90),
    })
}

fn bucket_index(rgb: [f32; 3]) -> usize {
    let q = |c: f32| ((c * DOMINANT_LEVELS as f32) as usize).min(DOMINANT_LEVELS - 1);
    (q(rgb[0]) * DOMINANT_LEVELS + q(rgb[1])) * DOMINANT_LEVELS + q(rgb[2])
}

/// 选出得分最高的桶，返回桶内像素的均值色。
///
/// 返回桶内均值而不是桶心：桶是 1/4 宽的立方体，取桶心会把颜色统一吸到
/// 几个固定值上，相邻封面的遮罩会出现肉眼可见的跳变。
fn dominant_color(hist: &[[f32; 4]]) -> [f32; 3] {
    let mut best = [0.0_f32; 3];
    let mut best_score = -1.0_f32;
    for bucket in hist {
        let n = bucket[3];
        if n <= 0.0 {
            continue;
        }
        let color = [bucket[0] / n, bucket[1] / n, bucket[2] / n];
        let (_, sat, _) = rgb_to_hsl(color);
        let score = n * (DOMINANT_NEUTRAL_FLOOR + sat);
        if score > best_score {
            best_score = score;
            best = color;
        }
    }
    best
}

fn percentile(sorted: &[f32], q: f32) -> f32 {
    debug_assert!(!sorted.is_empty());
    let idx = ((sorted.len() - 1) as f32 * q).round() as usize;
    sorted[idx]
}

fn scrim_for(s: &BottomSamples, tint_rgb: u32) -> CoverScrim {
    let light = s.p10 > LIGHT_COVER_P10;
    // 文字色先定下来，遮罩再往「能托住这个文字色」的方向收敛。反过来（先
    // 定遮罩再挑文字）在中等明度的封面上会反复横跳。
    let text: [f32; 3] = if light {
        [0.06, 0.06, 0.06]
    } else {
        [1.0, 1.0, 1.0]
    };
    // 最坏情况的背景：白字怕亮，所以看 p90；黑字怕暗，所以看 p10。遮罩是
    // 半透的，字最终压在「遮罩 + 封面」的合成色上。
    let backdrop_lum = if light { s.p10 } else { s.p90 };
    let backdrop = [backdrop_lum, backdrop_lum, backdrop_lum];

    let (h, sat, dominant_l) = rgb_to_hsl(s.dominant);
    let sat = sat.min(SCRIM_SAT_MAX);
    // 够不够暗/亮由对比度说了算，所以从主色自己的明度起步，不达标才挪。
    let pole = if light { 1.0 } else { 0.0 };
    let mut l = dominant_l.clamp(0.0, 1.0);

    let mut chosen = None;
    'search: loop {
        let scrim = hsl_to_rgb(h, sat, l);
        let mut alpha = ALPHA_MIN;
        while alpha <= ALPHA_MAX + f32::EPSILON {
            let composite = blend(scrim, alpha, backdrop);
            if contrast(text, composite) >= MIN_TITLE_CONTRAST {
                chosen = Some((scrim, alpha, composite));
                break 'search;
            }
            alpha += ALPHA_STEP;
        }
        if (l - pole).abs() <= f32::EPSILON {
            break;
        }
        l = if light {
            (l + LIGHTNESS_STEP).min(pole)
        } else {
            (l - LIGHTNESS_STEP).max(pole)
        };
    }
    // 走到极点仍不达标只可能来自浮点边界；取该极点 + 最大不透明度兜底。
    let (scrim, alpha, composite) = chosen.unwrap_or_else(|| {
        let scrim = hsl_to_rgb(h, sat, pole);
        (scrim, ALPHA_MAX, blend(scrim, ALPHA_MAX, backdrop))
    });

    let (tint_argb, tint_label_argb) = tinted_pill(tint_rgb, text, composite);
    CoverScrim {
        scrim_argb: pack(scrim, alpha),
        title_argb: pack(text, 1.0),
        subtitle_argb: pack(subtitle_color(text, composite), 1.0),
        tint_argb,
        tint_label_argb,
    }
}

/// 辅助信息比标题弱一档：往合成背景色里退，但不越过 [`MIN_SUBTITLE_CONTRAST`]。
fn subtitle_color(text: [f32; 3], composite: [f32; 3]) -> [f32; 3] {
    // 0.82 是观感上「明显次一级但不像禁用态」的位置；真正的底线由下面的
    // 回退守住，所以这个数调整起来不会把可读性调没。
    let faded = blend(text, 0.82, composite);
    if contrast(faded, composite) >= MIN_SUBTITLE_CONTRAST {
        faded
    } else {
        text
    }
}

/// 在 sRGB 分量空间里做 alpha 合成——Flutter / Slint 的画布默认就是在这个
/// 空间混色，换成线性光算出来的对比度和屏幕上看到的会对不上。
fn blend(fg: [f32; 3], alpha: f32, bg: [f32; 3]) -> [f32; 3] {
    [
        fg[0] * alpha + bg[0] * (1.0 - alpha),
        fg[1] * alpha + bg[1] * (1.0 - alpha),
        fg[2] * alpha + bg[2] * (1.0 - alpha),
    ]
}

fn relative_luminance(rgb: [f32; 3]) -> f32 {
    fn lin(c: f32) -> f32 {
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2])
}

fn contrast(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn rgb_to_hsl(rgb: [f32; 3]) -> (f32, f32, f32) {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d <= f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == rgb[0] {
        ((rgb[1] - rgb[2]) / d).rem_euclid(6.0)
    } else if max == rgb[1] {
        (rgb[2] - rgb[0]) / d + 2.0
    } else {
        (rgb[0] - rgb[1]) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s <= f32::EPSILON {
        return [l, l, l];
    }
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [r + m, g + m, b + m]
}

fn pack(rgb: [f32; 3], alpha: f32) -> u32 {
    fn ch(v: f32) -> u32 {
        (v.clamp(0.0, 1.0) * 255.0).round() as u32
    }
    (ch(alpha) << 24) | (ch(rgb[0]) << 16) | (ch(rgb[1]) << 8) | ch(rgb[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// B 站粉，宿主实际传的就是它（`Style.biliPink`）。
    const BILI_PINK: u32 = 0x00_FB_72_99;

    /// 测试里一律用品牌色跑，省得每个用例都写一遍最后一个参数。
    fn cover_scrim_t(rgba: &[u8], w: u32, h: u32) -> CoverScrim {
        cover_scrim(rgba, w, h, BILI_PINK)
    }

    /// 造一张纯色缩略图。
    fn solid(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        (0..w * h)
            .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
            .collect()
    }

    fn unpack(argb: u32) -> ([f32; 3], f32) {
        (
            [
                ((argb >> 16) & 0xFF) as f32 / 255.0,
                ((argb >> 8) & 0xFF) as f32 / 255.0,
                (argb & 0xFF) as f32 / 255.0,
            ],
            ((argb >> 24) & 0xFF) as f32 / 255.0,
        )
    }

    /// 标题压在「遮罩合成到封面底部最坏像素」上之后的实际对比度。
    fn title_contrast(scrim: CoverScrim, worst_backdrop: f32) -> f32 {
        let (scrim_rgb, alpha) = unpack(scrim.scrim_argb);
        let (title, _) = unpack(scrim.title_argb);
        let bg = [worst_backdrop; 3];
        contrast(title, blend(scrim_rgb, alpha, bg))
    }

    #[test]
    fn 尺寸对不上时回退() {
        assert_eq!(cover_scrim_t(&[0, 0, 0], 4, 4), fallback_scrim(BILI_PINK));
        assert_eq!(cover_scrim_t(&[], 0, 0), fallback_scrim(BILI_PINK));
    }

    #[test]
    fn 深色封面给白字() {
        let s = cover_scrim_t(&solid(16, 16, [20, 24, 40]), 16, 16);
        assert_eq!(s.title_argb, 0xFF_FF_FF_FF);
    }

    #[test]
    fn 浅色封面翻成浅遮罩加深字() {
        let s = cover_scrim_t(&solid(16, 16, [245, 243, 238]), 16, 16);
        let (title, _) = unpack(s.title_argb);
        let (scrim, _) = unpack(s.scrim_argb);
        assert!(relative_luminance(title) < 0.1, "浅封面应给深色文字");
        assert!(relative_luminance(scrim) > 0.6, "浅封面应给浅色遮罩");
    }

    #[test]
    fn 遮罩保留封面的色相() {
        // 饱和的红色封面，遮罩应该偏红而不是中性灰。
        let s = cover_scrim_t(&solid(16, 16, [160, 20, 20]), 16, 16);
        let (scrim, _) = unpack(s.scrim_argb);
        assert!(scrim[0] > scrim[1] && scrim[0] > scrim[2]);
    }

    #[test]
    fn 各种纯色封面都满足标题对比度() {
        for r in [0_u8, 64, 128, 200, 255] {
            for g in [0_u8, 64, 128, 200, 255] {
                for b in [0_u8, 64, 128, 200, 255] {
                    let px = solid(8, 8, [r, g, b]);
                    let s = cover_scrim_t(&px, 8, 8);
                    let lum = relative_luminance([
                        f32::from(r) / 255.0,
                        f32::from(g) / 255.0,
                        f32::from(b) / 255.0,
                    ]);
                    let c = title_contrast(s, lum);
                    assert!(c >= MIN_TITLE_CONTRAST - 0.01, "({r},{g},{b}) 对比度 {c}");
                }
            }
        }
    }

    #[test]
    fn 高对比杂色封面仍然可读() {
        // 黑白棋盘：平均是中灰，但 p90 接近纯白、p10 接近纯黑。
        let (w, h) = (16_u32, 16_u32);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let v = if (x + y) % 2 == 0 { 255 } else { 0 };
                px.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let s = cover_scrim_t(&px, w, h);
        assert!(title_contrast(s, 1.0) >= MIN_TITLE_CONTRAST - 0.01);
    }

    #[test]
    fn 辅助信息不弱于底线() {
        for v in [0_u8, 90, 180, 255] {
            let s = cover_scrim_t(&solid(8, 8, [v, v, v]), 8, 8);
            let (scrim_rgb, alpha) = unpack(s.scrim_argb);
            let lum = relative_luminance([f32::from(v) / 255.0; 3]);
            let composite = blend(scrim_rgb, alpha, [lum; 3]);
            let (sub, _) = unpack(s.subtitle_argb);
            assert!(contrast(sub, composite) >= MIN_SUBTITLE_CONTRAST - 0.01);
        }
    }

    /// 一半 A 色一半 B 色的缩略图。
    fn halves(w: u32, h: u32, left: [u8; 3], right: [u8; 3]) -> Vec<u8> {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..h {
            for x in 0..w {
                let c = if x < w / 2 { left } else { right };
                px.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        px
    }

    #[test]
    fn 深色封面的遮罩不被压成纯黑() {
        // 这是改版的要点：遮罩要看得出是「封面的那个颜色」，不是一块黑。
        // 深蓝封面本来就够暗，白字压上去已经达标，不该再往下压。
        let s = cover_scrim_t(&solid(16, 16, [38, 54, 102]), 16, 16);
        let (scrim, alpha) = unpack(s.scrim_argb);
        let (_, _, l) = rgb_to_hsl(scrim);
        assert!(l > 0.15, "明度被压没了：{l}");
        assert!(scrim[2] > scrim[0], "蓝色封面的遮罩应偏蓝");
        assert!(alpha < 1.0, "遮罩必须是半透的");
    }

    #[test]
    fn 主色不是平均色() {
        // 左半纯白、右半纯蓝：平均色是淡蓝灰，主色应该是那片蓝。
        // 取平均的旧写法在这里会算出一个几乎看不出颜色的遮罩。
        let px = halves(16, 16, [255, 255, 255], [30, 70, 200]);
        let s = cover_scrim_t(&px, 16, 16);
        let (scrim, _) = unpack(s.scrim_argb);
        let (h, sat, _) = rgb_to_hsl(scrim);
        assert!(sat > 0.3, "主色的饱和度被平均掉了：{sat}");
        assert!((200.0..=250.0).contains(&h), "色相不是蓝：{h}");
    }

    #[test]
    fn 大片中性色压得过小块高饱和() {
        // 几乎全是深灰、角落一小撮亮红：主色该是那片灰，不是红。
        // 否则一张灰调封面会因为一个红色 logo 整条遮罩变红。
        let (w, h) = (16_u32, 16_u32);
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let c = if x >= w - 2 && y >= h - 2 {
                    [230, 20, 20]
                } else {
                    [58, 60, 64]
                };
                px.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        let s = cover_scrim_t(&px, w, h);
        let (scrim, _) = unpack(s.scrim_argb);
        let (_, sat, _) = rgb_to_hsl(scrim);
        assert!(sat < 0.2, "被角落那一小块红带跑了：{sat}");
    }

    /// UP 名压在「品牌色药丸合成到遮罩上」之后的实际对比度。
    fn pill_contrast(s: CoverScrim, worst_backdrop: f32) -> f32 {
        let (scrim_rgb, alpha) = unpack(s.scrim_argb);
        let composite = blend(scrim_rgb, alpha, [worst_backdrop; 3]);
        let (tint, tint_alpha) = unpack(s.tint_argb);
        let (label, _) = unpack(s.tint_label_argb);
        contrast(label, blend(tint, tint_alpha, composite))
    }

    #[test]
    fn 各种纯色封面上的up名药丸都读得清() {
        for r in [0_u8, 64, 128, 200, 255] {
            for g in [0_u8, 64, 128, 200, 255] {
                for b in [0_u8, 64, 128, 200, 255] {
                    let s = cover_scrim_t(&solid(8, 8, [r, g, b]), 8, 8);
                    let lum = relative_luminance([
                        f32::from(r) / 255.0,
                        f32::from(g) / 255.0,
                        f32::from(b) / 255.0,
                    ]);
                    let c = pill_contrast(s, lum);
                    assert!(
                        c >= MIN_SUBTITLE_CONTRAST - 0.01,
                        "({r},{g},{b}) 药丸对比度 {c}"
                    );
                }
            }
        }
    }

    #[test]
    fn 药丸是色调不是实色() {
        // 压到不透明就成了一块固定粉色块，遮罩随封面变色的效果全被盖掉。
        let s = cover_scrim_t(&solid(16, 16, [30, 40, 60]), 16, 16);
        let (tint, alpha) = unpack(s.tint_argb);
        assert!(alpha > 0.0 && alpha <= TINT_ALPHA_MAX, "alpha={alpha}");
        // 传什么色就是什么色，不许在 Rust 这边改色相。
        assert_eq!(s.tint_argb & 0x00_FF_FF_FF, BILI_PINK);
        assert!(tint[0] > tint[1]);
    }

    #[test]
    fn 药丸文字跟标题同色() {
        // 同一行里 UP 名和其它文字不该一个白一个黑。
        for v in [0_u8, 120, 255] {
            let s = cover_scrim_t(&solid(8, 8, [v, v, v]), 8, 8);
            assert_eq!(s.tint_label_argb, s.title_argb, "v={v}");
        }
    }

    #[test]
    fn 只统计底部() {
        // 上半纯白、下半纯黑：应按下半的黑来判极性。
        let (w, h) = (8_u32, 8_u32);
        let mut px = Vec::new();
        for y in 0..h {
            let v = if y < h / 2 { 255 } else { 0 };
            for _ in 0..w {
                px.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let s = cover_scrim_t(&px, w, h);
        assert_eq!(s.title_argb, 0xFF_FF_FF_FF);
    }
}
