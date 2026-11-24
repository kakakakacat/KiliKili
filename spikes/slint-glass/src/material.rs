//! `GlassMaterial`：玻璃材质的全部可调参数，以及五个预设。
//!
//! **这份没有编译过。** 但它是这个 spike 里唯一可以在编译之前当参考的
//! 东西：纯数据，值逐个对着 `shaders/liquid_glass.frag` 与
//! `lib/common/widgets/floating_navigation_bar.dart` 抄的，抄错了能一眼
//! 看出来。
//!
//! # 它为什么在 spike 里而不在 `rust/crates/` 下
//!
//! 按 AGENTS.md §2，材质参数表、形状计算、质量选择与降级规则都该下沉到
//! Rust 的产品代码里——换掉宿主框架时这些不该重写。这张表最终会是那样。
//!
//! 但 #130 的关口 2 还没过（玻璃背后的背景纹理取不取得到都不知道），这时
//! 候在 `rust/crates/` 下立一个 crate，等于把一个未验证的架构决策写进
//! 产品代码。**所以它先待在这里，随 Phase 1 的 ADR 一起搬家。**
//! 这条例外按 AGENTS.md「例外要写下来」的要求记在这儿，随 #130 的 Phase 1
//! 删除。
//!
//! # 单位与颜色约定
//!
//! - 所有长度单位一律是**逻辑像素**，不是物理像素。WGSL 入口处按
//!   `scale` 换算，理由见 `shaders/glass.wgsl` 顶部。
//! - 所有 `0.0..=1.0` 的强度值都是线性插值系数，不是百分比。
//! - 颜色分量按**非预乘**理解；合成阶段是否预乘要等关口 2 的路径定下来
//!   才能写死（候选 C 下场景纹理的 alpha 未必是 1）。

/// 玻璃材质的全部可调参数。
///
/// 字段取自 #130 列的统一参数表。**其中三个这一轮没有实现**，保留字段是
/// 为了让预设表在 Phase 2 补上效果时不用改形状——没实现的字段在
/// `shaders/glass.wgsl` 里根本没有对应的 uniform，不要误以为调了有用：
///
/// - `blur_radius`：多尺度模糊是 P0 里 Flutter 侧也没做的部分。
/// - `fresnel_power`：同上，Flutter 版只有单点 glint，没有 Fresnel。
/// - `thickness`：边缘厚度与形变排在 P1。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassMaterial {
    /// 背景模糊半径，逻辑像素。**未实现**，见结构体文档。
    ///
    /// 10.0 这个初值不是拍的：Flutter 侧在后端不支持自定义 shader 时整体
    /// 退到 `ImageFilter.blur(sigma 10)`，那条回退路径的观感就是 Low 档
    /// 要对齐的目标（floating_navigation_bar.dart 的 `supportsShader`
    /// 分支）。
    pub blur_radius: f32,
    /// 材质整体不透明度，`0.0..=1.0`。
    pub opacity: f32,
    /// 沿法线偏移背景 UV 的像素数，逻辑像素。GLSL 里写死的 7.0。
    pub refraction_strength: f32,
    /// Fresnel 边缘反射的指数。**未实现**，见结构体文档。
    pub fresnel_power: f32,
    /// 高光向白色 mix 的强度，`0.0..=1.0`。GLSL 里写死的 0.18。
    pub specular_strength: f32,
    /// 边缘折射带的宽度，逻辑像素。GLSL 里写死的 15.0。
    ///
    /// 命名跟 #130 的参数表走（那边叫 `edge_highlight`），但要注意它控制的
    /// 是**折射带宽**而不是高光——高光带宽是下面的 `specular_width`。
    /// 两者在 GLSL 里是两个独立的 smoothstep（15 与 4），#130 的参数表把
    /// 它们混成一项了，这里拆开。
    pub edge_highlight: f32,
    /// 高光带的宽度，逻辑像素。GLSL 里写死的 4.0。
    ///
    /// #130 的参数表里没有这一项，是从 GLSL 里拆出来的，见 `edge_highlight`。
    pub specular_width: f32,
    /// 玻璃厚度，逻辑像素。**未实现**，见结构体文档。
    pub thickness: f32,
    /// 圆角半径，逻辑像素。胶囊形状时等于高度的一半。
    pub corner_radius: f32,
}

impl GlassMaterial {
    /// 现行 Flutter 实现的标定值。
    ///
    /// **这是整张表的锚点**：Phase 1 的 Demo 要能和首页导航栏逐帧比对，
    /// 比对的参照物就是这一组。其余预设都是在它上面调出来的，调之前先让
    /// 这一组对上。
    ///
    /// 来源：`shaders/liquid_glass.frag`（15 / 7 / 0.18 / 4）与
    /// `lib/common/widgets/floating_navigation_bar.dart`（导航高度 80、
    /// 圆角 = 高度 / 2）。
    pub const FLUTTER_BASELINE: Self = Self {
        blur_radius: 10.0,
        opacity: 1.0,
        refraction_strength: 7.0,
        fresnel_power: 0.0,
        specular_strength: 0.18,
        edge_highlight: 15.0,
        specular_width: 4.0,
        thickness: 0.0,
        corner_radius: 40.0,
    };
}

/// #130 列的五个预设。
///
/// **除 `Regular` 之外的四个都是猜的**，没有任何实现或实测背书：#130 只
/// 给了名字，没给参数。它们现在的作用是占住形状，让「参数不散落硬编码」
/// 这条约束从第一天就成立，**不是**可以直接用的设计值。Phase 1 的 Demo
/// 跑出来之后按观感重新定，定完把这句话删掉。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassPreset {
    /// 导航栏用的那一档，等于 `GlassMaterial::FLUTTER_BASELINE`。
    Regular,
    /// 折射更弱、几乎只剩高光。
    Clear,
    /// 模糊更重的磨砂。
    Frosted,
    /// 浮层用，边缘更明显。
    Floating,
    /// 可交互控件用，按压时会在这组值上做动画。
    Interactive,
}

impl GlassPreset {
    /// 取这个预设的参数。`corner_radius` 由调用方按实际尺寸覆盖——胶囊的
    /// 圆角等于高度一半，预设表不知道高度。
    pub fn material(self) -> GlassMaterial {
        let base = GlassMaterial::FLUTTER_BASELINE;
        match self {
            Self::Regular => base,
            Self::Clear => GlassMaterial {
                blur_radius: 4.0,
                refraction_strength: 3.0,
                edge_highlight: 10.0,
                ..base
            },
            Self::Frosted => GlassMaterial {
                blur_radius: 24.0,
                opacity: 0.92,
                refraction_strength: 5.0,
                ..base
            },
            Self::Floating => GlassMaterial {
                blur_radius: 16.0,
                refraction_strength: 9.0,
                specular_strength: 0.24,
                edge_highlight: 18.0,
                ..base
            },
            Self::Interactive => GlassMaterial {
                specular_strength: 0.26,
                edge_highlight: 18.0,
                ..base
            },
        }
    }
}

/// 质量档。
///
/// #130 的质量表要求「能力判定在材质层统一做一次、整条表面一起降级」，而
/// 不是逐效果开关——这条直接沿用 Flutter 侧的现成先例：那边用
/// `ImageFilter.isShaderFilterSupported` 判一次，为假就整体退到
/// `ImageFilter.blur`。
///
/// 这一轮三档的行为**都还没有实现**，枚举先立住，免得降级逻辑日后散落到
/// 各个组件里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassQuality {
    /// 多级模糊、完整折射、动态高光。
    High,
    /// 单级模糊、简化折射。
    Balanced,
    /// 静态预模糊或缓存，关折射，静态高光。对应 Flutter 侧那条回退路径。
    Low,
}

/// 传给 `shaders/glass.wgsl` 的 uniform。
///
/// 字段顺序与类型必须和 WGSL 里的 `GlassUniforms` 完全一致，**改一边就要
/// 改另一边**。这里刻意不加 `#[repr(C)]` 以外的东西：std140/std430 的对齐
/// 规则会让 `vec2<f32>` 后面跟 `f32` 这种排布出意外，第一次编译通过之后
/// 要用实际写进去的字节核对一遍，不能只看结构体定义。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GlassUniforms {
    pub size: [f32; 2],
    pub scale: f32,
    pub corner_radius: f32,
    pub rim_width: f32,
    pub refraction_strength: f32,
    pub specular_strength: f32,
    pub specular_width: f32,
}

impl GlassUniforms {
    /// 按表面尺寸与 HiDPI 比例把材质参数打成 uniform。
    ///
    /// `size` 是**逻辑像素**，`scale` 是物理/逻辑。两者都不能用物理像素填，
    /// 理由见 `shaders/glass.wgsl` 顶部那段。
    pub fn new(material: &GlassMaterial, size: [f32; 2], scale: f32) -> Self {
        Self {
            size,
            scale,
            corner_radius: material.corner_radius,
            rim_width: material.edge_highlight,
            refraction_strength: material.refraction_strength,
            specular_strength: material.specular_strength,
            specular_width: material.specular_width,
        }
    }
}
