//! `GlassMaterial`：玻璃材质的全部可调参数，以及五个预设。
//!
//! 2026-11-25 起这份真的跑起来了：`gl_probe` 把它整张表打成 uniform 交给
//! `shaders/glass_overlay.frag`，`wgpu_probe` 用 `GlassUniforms` 跑
//! `shaders/glass.wgsl`。它也是这个 spike 里唯一可以当参考的东西：纯数据，
//! 值逐个对着 `shaders/liquid_glass.frag` 与
//! `lib/common/widgets/floating_navigation_bar.dart` 抄的，抄错了能一眼
//! 看出来。
//!
//! # 它为什么在 spike 里而不在 `rust/crates/` 下
//!
//! 按 AGENTS.md §2，材质参数表、形状计算、质量选择与降级规则都该下沉到
//! Rust 的产品代码里——换掉宿主框架时这些不该重写。这张表最终会是那样。
//!
//! 但 ADR-016 还是 Proposed（仓库所有者未确认），这时候在 `rust/crates/`
//! 下立一个 crate，等于把一个未确认的架构决策写进产品代码。**所以它先待
//! 在这里，随 #224（组件库）搬家**，搬的时候删掉本节。
//! 这条例外按 AGENTS.md「例外要写下来」的要求记在这儿。
//!
//! # 单位与颜色约定
//!
//! - 所有长度单位一律是**逻辑像素**，不是物理像素。shader 入口处按
//!   `scale` 换算，理由见 `shaders/glass.wgsl` 顶部。
//! - 所有 `0.0..=1.0` 的强度值都是线性插值系数，不是百分比。
//! - 角度一律是**弧度**，左下原点、逆时针为正（和 GL 的帧缓冲同向）。
//! - 颜色分量按**非预乘**理解；GL 覆盖这条路径上合成用的是
//!   `SRC_ALPHA / ONE_MINUS_SRC_ALPHA`，也就是非预乘。

/// 玻璃材质的全部可调参数。
///
/// 字段取自 #130 列的统一参数表，Phase 2 之后**全部有实现**了。`thickness`
/// 那一项 #130 只给了名字没给语义，这里赋予的含义写在字段文档里，
/// 待 ADR-016 确认。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassMaterial {
    /// 背景模糊半径（高斯标准差），逻辑像素。0 = 不模糊。
    ///
    /// 10.0 这个初值不是拍的：Flutter 侧在后端不支持自定义 shader 时整体
    /// 退到 `ImageFilter.blur(sigma 10)`，那条回退路径的观感就是 Low 档
    /// 要对齐的目标（floating_navigation_bar.dart 的 `supportsShader`
    /// 分支）。
    pub blur_radius: f32,
    /// 多尺度模糊的级数，`1..=4`。1 = 单级（Phase 1 的行为）。
    ///
    /// 每多一级，模糊纹理边长再减半、σ 约翻倍：σ=10 时四级的等效 σ 依次是
    /// **10 / 18.9 / 37.0 / 73.7** 逻辑像素（按方差相加算，推导见
    /// `gl_probe::Overlay::blur`）。
    ///
    /// 它不只是「省钱」——边缘要清楚、体部要糊这件事**单级做不到**，见
    /// `glass_overlay.frag` 的 `backdrop()`。
    pub blur_levels: u32,
    /// 材质整体不透明度，`0.0..=1.0`。乘在 SDF 覆盖率上。
    pub opacity: f32,
    /// 沿法线偏移背景 UV 的像素数，逻辑像素。GLSL 里写死的 7.0。
    pub refraction_strength: f32,
    /// Fresnel 边缘反射的指数。0 = 关掉。
    pub fresnel_power: f32,
    /// 高光向白色 mix 的强度，`0.0..=1.0`。GLSL 里写死的 0.18。
    pub specular_strength: f32,
    /// 高光随 `N·L` 衰减的指数，越大越集中成一个亮点。
    ///
    /// **0 有特殊含义**：整段退回 Flutter 的「贴边一圈恒定 glint」，与光源
    /// 方向无关。`FLUTTER_BASELINE` 靠这个退化保持同式，不要改成 1.0。
    pub specular_sharpness: f32,
    /// 顶边常驻高光强度，`0.0..=1.0`。0 = 关。
    ///
    /// 和 `specular_strength` 分开，是因为它**与光源方向无关**：真玻璃的
    /// 上沿总比下沿亮一道（天光），光源转到底下时它也还在。并进方向项的话
    /// 光源一转到下方顶边就全黑，不对。
    pub top_highlight: f32,
    /// 光源方向的初始角，弧度。0 = 正右，逆时针为正。
    pub light_angle: f32,
    /// 光源角速度，弧度/秒。0 = 静止。
    ///
    /// **动画状态由 Rust 推**（`gl_probe` 用 `Instant::elapsed`），宿主只
    /// 管画——AGENTS.md §2：换宿主时这段不该重写。
    pub light_speed: f32,
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
    /// 玻璃厚度，逻辑像素。
    ///
    /// **#130 的参数表只给了名字，没给语义。** Phase 2 给它的含义是
    /// 「多尺度模糊从细级过渡到粗级的深度」：越厚，重模糊那一档越往边缘
    /// 逼近，薄玻璃则整面都偏清楚。
    ///
    /// 选这个含义的理由是它正好把「厚度」和唯一一个需要**按深度插值**的
    /// 效果接上了，不必再新立一个参数；0 表示整面都用最粗那级。
    /// **这是 Phase 2 的取舍，不是 #130 的原意**，待 ADR-016 确认。
    pub thickness: f32,
    /// 圆角半径，逻辑像素。胶囊形状时等于高度的一半，圆形时等于半径。
    ///
    /// 圆角矩形 / 圆 / 胶囊在 `glass_sdf.glsl` 里是**同一个 SDF**，只有
    /// 这一项不同，不是三种形状三段代码。
    pub corner_radius: f32,
    /// 两个形状融合的半径，逻辑像素。0 = 不融合（硬并集）。
    ///
    /// 对应 #130 P1 的「靠近时的玻璃融合」。它和 ADR-016 决策二那条
    /// 「相距小于 3σ 会互相采到背板」是同一件事的两面：那条是副作用，
    /// 这条是把它用成效果。
    pub merge_k: f32,

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
    /// **这是整张表的锚点**：Demo 要能和首页导航栏逐帧比对，比对的参照物
    /// 就是这一组。其余预设都是在它上面调出来的，调之前先让这一组对上。
    ///
    /// 来源：`shaders/liquid_glass.frag`（15 / 7 / 0.18 / 4）与
    /// `lib/common/widgets/floating_navigation_bar.dart`（导航高度 80、
    /// 圆角 = 高度 / 2）。
    ///
    /// Phase 2 新加的那几项在这一档里**必须全是 0**：动态高光、顶边高光、
    /// 融合，Flutter 那边一个都没有。
    pub const FLUTTER_BASELINE: Self = Self {
        blur_radius: 10.0,
        blur_levels: 1,
        opacity: 1.0,
        refraction_strength: 7.0,
        fresnel_power: 0.0,
        specular_strength: 0.18,
        // 0 = 退回「贴边一圈恒定 glint」，见字段文档。Flutter 版就是这样。
        specular_sharpness: 0.0,
        top_highlight: 0.0,
        light_angle: 0.0,
        light_speed: 0.0,
        edge_highlight: 15.0,
        specular_width: 4.0,
        thickness: 0.0,
        corner_radius: 40.0,
        merge_k: 0.0,
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

    /// #130 Phase 2 要的全套：在 `P0_TARGET` 上补**多尺度模糊**、
    /// **动态高光**与**融合**。
    ///
    /// 和 P0 一样单独立一档，理由同上：三档并排才分得清画面上的差别各自
    /// 来自哪一项。
    pub const PHASE2_TARGET: Self = Self {
        // 三级：等效 σ 10 / 18.9 / 37.0。四级在 80px 高的胶囊上粗到没有
        // 意义（等效 σ 73.7 已经超过胶囊本身），所以默认三级而不是顶满。
        blur_levels: 3,
        // 过渡深度 = 胶囊半高。再深就过不完，再浅则体部整片都是粗级、
        // 多尺度白做。
        thickness: 40.0,
        // 高光：方向性 + 顶边常驻。
        //
        // 8 是看着调的集中度：`pow(N·L, 8)` 掉到一半对应法线偏离光源约 28°，
        // 也就是一段约 56° 的弧。**一开始填的是 32**，那是 10° 的弧，在
        // 80px 高的胶囊上只有几个像素，四个光源角度之间量出来的亮度差只有
        // 1~2 个灰阶——参数对了形状、错了量级。
        specular_sharpness: 8.0,
        top_highlight: 0.10,
        // 每 6 秒转一圈。动起来才看得出「动态」这件事——静止的方向性高光
        // 和一道固定亮边在单张截图里分不开。
        light_speed: std::f32::consts::TAU / 6.0,
        // 融合半径取 24：比两个形状靠近时的常见间距大一档，颈部才长得出来。
        merge_k: 24.0,
        ..Self::P0_TARGET
    };

    /// 按质量档整体降级。
    ///
    /// **整条表面一起降，不逐效果开关**（ADR-016 决策五）：降级后的观感
    /// 是设计过的一档，而不是若干开关的任意组合。
    ///
    /// `Low` 在这里只是把自绘层的参数清干净；真正的 Low 是**整个不画自绘
    /// 层、只留原生 `backdrop-blur`**，那个判断在宿主侧（见
    /// [`GlassQuality::draws_overlay`]），因为它决定的是画不画，不是怎么画。
    pub fn at_quality(self, quality: GlassQuality) -> Self {
        match quality {
            GlassQuality::High => self,
            GlassQuality::Balanced => Self {
                // **模糊整个交给原生 `backdrop-blur`**，自绘这层只管折射与
                // 高光——也就是 ADR-016 决策四那个组合。宿主要负责在它底下
                // 摆一条只开模糊的原生矩形，见 [`GlassQuality::needs_native_blur`]。
                //
                // 这一档原本写的是「自绘单级模糊 + 折射减半」，实测之后改掉了：
                // 自绘单级 +2.69 ms，而 High（三级）才 +2.88 ms——中间档和
                // 顶档一样贵，等于没有中间档；换成这个组合是 **+0.83 ms**。
                //
                // 相应地**折射不减半**。#130 的质量表把 Balanced 写成「简化
                // 折射」，但折射正是自绘这层唯一不可替代的东西（原生给不了），
                // 砍它等于砍掉这一档存在的理由；真正贵的是模糊，而模糊已经
                // 交出去了。减的是高光：退回静态 glint。
                blur_radius: 0.0,
                blur_levels: 1,
                thickness: 0.0,
                // 简化高光：退回静态 glint，不再按光源方向算。
                specular_sharpness: 0.0,
                top_highlight: 0.0,
                light_speed: 0.0,
                // 融合要两个形状同时在一张背板上，Balanced 档不做。
                merge_k: 0.0,
                ..self
            },
            GlassQuality::Low => Self {
                blur_levels: 1,
                thickness: 0.0,
                refraction_strength: 0.0,
                fresnel_power: 0.0,
                fresnel_strength: 0.0,
                specular_sharpness: 0.0,
                top_highlight: 0.0,
                light_speed: 0.0,
                merge_k: 0.0,
                ..self
            },
        }
    }
}

/// #130 列的五个预设。
///
/// **除 `Regular` 之外的四个都是猜的**，没有任何实测背书：#130 只给了
/// 名字，没给参数。它们现在的作用是占住形状，让「参数不散落硬编码」这条
/// 约束从第一天就成立，**不是**可以直接用的设计值。组件库那一轮（#224）
/// 按观感重新定，定完把这句话删掉。
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
                // 磨砂要的是大 σ，而大 σ 正是多尺度的用处：单级把 σ 拉到
                // 这个量级要多采一倍样本，叠一级粗的只要 1/5 的代价。
                blur_radius: 12.0,
                blur_levels: 3,
                thickness: 32.0,
                opacity: 0.92,
                refraction_strength: 5.0,
                ..base
            },
            Self::Floating => GlassMaterial {
                blur_radius: 16.0,
                blur_levels: 2,
                thickness: 40.0,
                refraction_strength: 9.0,
                specular_strength: 0.24,
                specular_sharpness: 32.0,
                top_highlight: 0.12,
                edge_highlight: 18.0,
                ..base
            },
            Self::Interactive => GlassMaterial {
                specular_strength: 0.26,
                specular_sharpness: 48.0,
                top_highlight: 0.08,
                edge_highlight: 18.0,
                // 可交互控件会彼此靠近（工具栏里一排按钮），融合默认开。
                merge_k: 20.0,
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
/// 三档的参数由 [`GlassMaterial::at_quality`] 给，画不画自绘层由
/// [`Self::draws_overlay`] 给。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassQuality {
    /// 多级模糊、完整折射、动态高光。
    High,
    /// 单级模糊、简化折射、静态高光。
    Balanced,
    /// 只留原生 `backdrop-blur`，自绘层整个不画。对应 Flutter 侧那条回退
    /// 路径（`ImageFilter.blur(10)`）。
    Low,
}

impl GlassQuality {
    /// 这一档要不要画自绘层。
    ///
    /// 单独一个判据而不是靠「参数全为 0」：参数全 0 时 shader 照样跑满
    /// 一遍采样与混合，省不掉那 0.27 ms；Low 档要省的正是这一笔。
    pub fn draws_overlay(self) -> bool {
        !matches!(self, Self::Low)
    }

    /// 这一档要不要宿主在玻璃底下摆一条**只开模糊**的原生矩形。
    ///
    /// High 自己画模糊（而且是多尺度的，原生给不了），底下再来一层就是双份；
    /// Balanced 把模糊整个交给原生；Low 只剩原生那一层。
    ///
    /// 为什么这件事在这里而不在宿主：它是**质量档的一部分**，不是布局偏好。
    /// 宿主自己决定的话，换个宿主就要重新推一遍「这一档到底谁画模糊」，
    /// 而那正是 AGENTS.md §2 要避免的。
    ///
    /// 注意原生那条必须是 `blur-only`（透明、无描边、无阴影）：带着渐变与
    /// 描边的话，自绘的折射会把那道 1px 白描边拖出第二道——实测如此，判据
    /// 见 `docs/m0/spikes/2026-11-25-glass-phase2-notes.md` §1。
    pub fn needs_native_blur(self) -> bool {
        matches!(self, Self::Balanced | Self::Low)
    }

    /// 从 `GLASS_GL_QUALITY` 之类的字符串解析。认不出来时返回 `None`，
    /// 由调用方决定是报错还是用默认——**不要在这里静默回落到 High**，
    /// 那样拼错一个字母就会得到一份标签与实际不符的记录（第一轮
    /// `GLASS_COUNT=10` 只画 6 条就是这个形状）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "high" => Some(Self::High),
            "balanced" => Some(Self::Balanced),
            "low" => Some(Self::Low),
            _ => None,
        }
    }
}

/// 传给 `shaders/glass.wgsl` 的 uniform。
///
/// 字段顺序与类型必须和 WGSL 里的 `GlassUniforms` 完全一致，**改一边就要
/// 改另一边**。这里刻意不加 `#[repr(C)]` 以外的东西：std140/std430 的对齐
/// 规则会让 `vec2<f32>` 后面跟 `f32` 这种排布出意外，第一次编译通过之后
/// 要用实际写进去的字节核对一遍，不能只看结构体定义。
///
/// **它只服务 `wgpu_probe`。** GL 覆盖那条路径不走这个结构体——glow 的
/// `uniform_*` 是逐个设的，没有布局问题，而且 Phase 2 新加的那几项
/// （多尺度、光源、融合）在 WGSL 那份里根本没有对应 uniform。两份哪天要
/// 合并，以 GL 那份的参数集为准：跑通的是它。
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `FLUTTER_BASELINE` 是「像不像 Flutter」唯一的参照物，Phase 2 新加的
    /// 那几项能力在它里面必须全是关的。
    ///
    /// 用测试钉住而不是靠注释：这条一旦被破坏，前三轮的对照记录全部作废，
    /// 而**画面上看不出来**——多出来的那点 Fresnel 或顶边高光只会让人觉得
    /// 「Slint 这边好像更亮一点」，不会让人想到参照物被动过。
    #[test]
    fn baseline_keeps_only_what_flutter_has() {
        let m = GlassMaterial::FLUTTER_BASELINE;
        assert_eq!(m.fresnel_power, 0.0, "Flutter 版没有 Fresnel");
        assert_eq!(m.fresnel_strength, 0.0, "Flutter 版没有 Fresnel");
        assert_eq!(
            m.specular_sharpness, 0.0,
            "必须退化成贴边一圈的静态 glint，否则就不是同式了"
        );
        assert_eq!(m.top_highlight, 0.0, "Flutter 版没有顶边高光");
        assert_eq!(m.light_speed, 0.0, "Flutter 版的高光不动");
        assert_eq!(m.merge_k, 0.0, "Flutter 版没有融合");
        assert_eq!(m.blur_levels, 1, "多尺度模糊不属于 baseline");
        assert_eq!(m.thickness, 0.0, "过渡深度只对多尺度有意义");
    }

    /// 降档只能**减**，不能加（ADR-016 决策五）。
    ///
    /// 这条看着像废话，但 `at_quality` 用的是结构体更新语法，写错一个字段名
    /// 就会把某一项悄悄调高，而三档的截图不并排看是分不出来的。
    #[test]
    fn quality_only_takes_away() {
        let high = GlassMaterial::PHASE2_TARGET;
        let balanced = high.at_quality(GlassQuality::Balanced);
        let low = high.at_quality(GlassQuality::Low);

        assert!(balanced.blur_levels <= high.blur_levels);
        // Balanced 把模糊交给原生，所以自绘这边的 σ 必须是 0——否则会和
        // 底下那条原生矩形叠成双份模糊。
        assert_eq!(balanced.blur_radius, 0.0);
        assert!(GlassQuality::Balanced.needs_native_blur());
        assert!(!GlassQuality::High.needs_native_blur());
        assert!(balanced.refraction_strength <= high.refraction_strength);
        assert!(balanced.specular_sharpness <= high.specular_sharpness);
        assert!(balanced.merge_k <= high.merge_k);

        assert!(low.refraction_strength <= balanced.refraction_strength);
        assert!(low.fresnel_strength <= balanced.fresnel_strength);
        assert!(low.blur_levels <= balanced.blur_levels);

        // High 档不该被 at_quality 动到——它就是材质表本身。
        assert_eq!(high.at_quality(GlassQuality::High), high);

        // 画不画自绘层是另一个判据，不靠「参数全为 0」推断。
        assert!(GlassQuality::High.draws_overlay());
        assert!(GlassQuality::Balanced.draws_overlay());
        assert!(!GlassQuality::Low.draws_overlay());
    }

    /// 档位名拼错时必须返回 `None`，由调用方吵出来。
    #[test]
    fn quality_parse_rejects_typos() {
        assert_eq!(
            GlassQuality::parse("balanced"),
            Some(GlassQuality::Balanced)
        );
        assert_eq!(GlassQuality::parse("Balanced"), None);
        assert_eq!(GlassQuality::parse("medium"), None);
    }
}
