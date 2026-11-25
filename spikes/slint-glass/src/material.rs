//! `GlassMaterial`：玻璃材质的全部可调参数，以及五个预设。
//!
//! 2026-11-25 起这份真的跑起来了：`wgpu_probe` 用 `GlassUniforms` 打 uniform、
//! 跑 `shaders/glass.wgsl`。它也是这个 spike 里唯一可以当参考的
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

    // ---- 以下四项是 2026-11-25 跟着 GL 覆盖探针一起补的 ----
    //
    // #130 正文的参数表里没有它们，但 Flutter 版的玻璃**不只是那 28 行
    // shader**：外面还套着一层 DecoratedBox（渐变 + 白描边），两层合起来
    // 才是首页那条导航栏的观感。只移植 shader 那一半的话，画出来是一片
    // 几乎看不见的折射边——对照时会误判成「自绘这条不行」。
    //
    // 既然 AGENTS.md §2 要求材质参数是单一来源，这两层的参数就都得在这
    // 张表里，而不是一半在表里、一半散在宿主的控件代码里。
    /// 表面渐变的三个停靠点 alpha（topLeft → 中点 → bottomRight）。
    ///
    /// 来源：`floating_navigation_bar.dart` 的 `scheme.surface` 0.24 / 0.10 /
    /// 0.18。Demo 里用白色近似 `surface`，接进应用时换主题色。
    pub surface_alpha: [f32; 3],
    /// 白色描边的 alpha。来源：同上，0.36。
    pub border_alpha: f32,
    /// 描边宽度，逻辑像素。Flutter 的 `Border.all` 默认 1。
    pub border_width: f32,
    /// Fresnel 边缘反射的强度，`0.0..=1.0`。
    ///
    /// 和 `fresnel_power` 配对：`fresnel_power` 是指数（边缘集中度），
    /// 这个是叠加上去的白色强度。**Flutter 版没有这个效果**，所以
    /// `FLUTTER_BASELINE` 里它是 0；它属于 #130 P0 表里 Flutter 没做、
    /// 要在 Slint 这边新做的那部分，见 `P0_TARGET`。
    pub fresnel_strength: f32,
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
        surface_alpha: [0.24, 0.10, 0.18],
        border_alpha: 0.36,
        border_width: 1.0,
        // Flutter 版没有 Fresnel。这里是 0 不是「还没调」，是**那边确实
        // 没有这个效果**——对照时它必须是 0，否则比的就不是同一个东西。
        fresnel_strength: 0.0,
    };

    /// #130 P0 表要的全套：在 `FLUTTER_BASELINE` 上补**模糊**与 **Fresnel**。
    ///
    /// 为什么要单独立一个常量，而不是直接把 baseline 调了：这两个是
    /// **Flutter 侧没有**的能力（P0 表里那五项，Flutter 只做到了「局部折射 +
    /// Specular 高光」两项）。混进 baseline 的话，「和 Flutter 像不像」就
    /// 再也没有参照物了。GL 探针默认跑这一档，`GLASS_GL_MATERIAL=baseline`
    /// 切回对照档。
    ///
    /// 注意 `blur_radius` 在这里**第一次真的生效**：Flutter 那边它只是
    /// 不支持 shader 时的回退值，两条路二选一（`supportsShader` 分支）；
    /// 自绘这条是「先模糊、再在模糊结果上折射」，两件事同时在。
    pub const P0_TARGET: Self = Self {
        // 和原生 backdrop-blur 的 sigma 取同一个值，三条玻璃才可比。
        blur_radius: 10.0,
        // 2.0 是 Fresnel 的常见起点（越大越集中在边缘）。**这个值是看着
        // 调的，不是算出来的**，#130 的 ADR 要是定了别的口径就以那边为准。
        fresnel_power: 2.0,
        fresnel_strength: 0.35,
        ..Self::FLUTTER_BASELINE
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

    /// 按 WGSL 里 `GlassUniforms` 的字段顺序摊成 8 个 f32。
    ///
    /// 为什么不直接把结构体转成字节：`vec2<f32>` 后面跟一串 `f32`，
    /// std140/std430 的对齐规则和 Rust 的 `#[repr(C)]` 这次恰好一致
    /// （全是 4 字节标量，vec2 对齐 8，总长 32 刚好对齐），**但这是巧合，
    /// 不是保证**。摊成数组之后顺序就在眼前，改字段时一眼能对上 WGSL。
    ///
    /// 这 8 个值对应 shaders/glass.wgsl 的：
    /// size.x, size.y, scale, corner_radius, rim_width,
    /// refraction_strength, specular_strength, specular_width
    pub fn as_f32_array(&self) -> [f32; 8] {
        [
            self.size[0],
            self.size[1],
            self.scale,
            self.corner_radius,
            self.rim_width,
            self.refraction_strength,
            self.specular_strength,
            self.specular_width,
        ]
    }
}
