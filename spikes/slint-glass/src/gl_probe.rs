//! 关口 2 **本体**：拿到 Slint 本帧的真实背板，并在它上面跑自己的 shader。
//!
//! # 为什么是 GL，而不是接着走 wgpu
//!
//! `wgpu_probe` 已经用运行时证据判掉了 wgpu 那条：`GraphicsAPI::WGPU30` 只有
//! `instance / device / queue`，没有本帧的渲染目标，拿不到 `TextureView` 就
//! 发不出 render pass。评估文档据此把「AfterRendering 覆盖」整条判为出局。
//!
//! **那一步推广错了。** 出局的只是 wgpu 这个变体。GL 是另一条：
//!
//! - `i-slint-core-1.19.1/api.rs:268` 的 `NativeOpenGL { get_proc_address }`
//!   ——GL 有隐式的当前帧缓冲，不需要谁把渲染目标递过来；
//! - femtovg 的 GL 后端（`opengl.rs:315`）和 Skia 的 GL 后端
//!   （`opengl_surface.rs:85`）给 notifier 的**都是这个变体**，而
//!   **Android 上走的正是后者**（`backend-android-activity-06` → Skia ganesh-gl）；
//! - 时机也对：femtovg 在 `lib.rs:316` 先 `flush_to_output` + `submit_commands`，
//!   **然后**才在 328 行发 `AfterRendering`，`present_surface` 在其后；Skia 在
//!   `lib.rs:992` 的 `ctx.flush(None)` 收尾，notifier 在 804 行其后。
//!   也就是说这个回调跑的时候，**本帧已经画进帧缓冲、还没上屏**。
//! - 上游自己就预期有人在这里画东西：`femtovg/opengl.rs:212` 的注释写着
//!   「`screenshot()` 读的是当前绑定的帧缓冲，而 `AfterRendering` notifier
//!   可能在本帧最后一次 flush 之后重新绑定它」。
//!
//! 于是「当前帧干净背景 + 自定义 shader + Android 可用」三件事第一次有可能
//! 同时成立——评估文档 §3 那张表说「没有任何一条同时满足」，是在只看 wgpu
//! 的前提下得出的。
//!
//! # 它和 `wgpu_probe` 的分工
//!
//! | | 背板 | 证明了什么 |
//! |---|---|---|
//! | `wgpu_probe` | **合成的**（自己画的条带） | 管线通、WGSL 过 naga、纹理导得进 Slint |
//! | 这个 | **本帧真实帧缓冲** | 关口 2 本体 |
//!
//! 两个探针**不要同时开**：它们会抢同一块屏幕位置，而且 wgpu 那条路下
//! notifier 拿到的根本不是 `NativeOpenGL`。
//!
//! # 自采（self-sampling）
//!
//! 背板是整帧拷下来的，**玻璃自己也在那一帧里**。默认的 `slot` 摆法让出最
//! 底下一格（原生胶囊整体上移一格），矩形里只有背景，采到的就是干净背板。
//!
//! 不会「一帧比一帧白」——Slint 每帧从场景图重画，自绘这层画在它之后，
//! 下一帧的 Slint 内容里并不含上一帧的自绘结果。所以压在原生胶囊上是
//! **可以**的，采到的是「本帧的原生玻璃」，不是累积。Phase 2 的 `over-full`
//! / `over-blur` 两档正是故意这么摆的（ADR-016 决策四）。
//!
//! 真正要防的是另一件事：原生那层画了什么，自绘这层就会把什么一起折射。
//! 实测 `over-full`（原生画渐变 + 1px 白描边）会被折射拖出**第二道描边**，
//! 而 `over-blur`（原生只画模糊、材质层归自绘）没有这个问题。判据与剖面
//! 见 `docs/m0/spikes/2026-11-25-glass-phase2-notes.md` §2。

use glow::HasContext;
use slint::ComponentHandle;

use crate::material::{GlassMaterial, GlassQuality};

/// Android 上没有 stdout，`println!` 进不了 logcat。
macro_rules! plog {
    ($($arg:tt)*) => {{
        #[cfg(target_os = "android")]
        log::info!($($arg)*);
        #[cfg(not(target_os = "android"))]
        println!($($arg)*);
    }};
}

/// 一次性的 GL 资源。`AfterRendering` 每帧都会被调用，建程序和纹理是一次性的事。
struct Overlay {
    gl: glow::Context,
    program: glow::Program,
    /// 可分离高斯模糊的那一趟，横竖共用，方向靠 uniform 给。
    blur_program: glow::Program,
    vao: glow::VertexArray,
    backdrop: glow::Texture,
    /// 背板纹理当前的尺寸，物理像素（**含边距**）。尺寸变了要重新分配。
    tex_size: (i32, i32),
    /// 覆盖矩形左下角在背板纹理里的位置，以及这张纹理的整体尺寸，物理像素。
    /// shader 要用它们把折射后的采样点换算成 uv，见 glass_overlay.frag。
    backdrop_origin: (f32, f32),
    backdrop_size: (f32, f32),
    /// 模糊金字塔与它们共用的 FBO。每级两张纹理：`[0]` 是横向那趟的中间
    /// 结果，`[1]` 是该级的输出。**不能只留两张 ping-pong**：shader 要同时
    /// 采第 0 级和最后一级（边缘细、体部粗），两张轮换的话第 0 级在算后面
    /// 几级时就被覆盖了。
    blur_fbo: glow::Framebuffer,
    blur_tex: [[glow::Texture; 2]; MAX_BLUR_LEVELS],
    /// 每级当前的尺寸，物理像素。
    ///
    /// Cell 而不是裸字段：`draw()` 里已经借着 `&self.gl`，再要个 `&mut self`
    /// 去改尺寸就借用冲突了。模糊这一步除了这组尺寸没有别的可变状态，
    /// 用 Cell 把它单独包起来，比把整条 draw 拆开更不容易写错。
    blur_sizes: std::cell::Cell<[(i32, i32); MAX_BLUR_LEVELS]>,
    /// 这一轮用哪组材质参数。**不在这里写常数**：那张表在 material.rs，
    /// 它是宿主与 shader 共同的单一来源（AGENTS.md §2）。
    material: GlassMaterial,
    material_name: String,
    /// 光源动画的起点。
    ///
    /// 动画状态由 Rust 推，不由宿主推（AGENTS.md §2：换掉 Slint 时这段
    /// 不该重写）。用 `Instant` 而不是帧计数，是因为帧率在 60 和 120 之间
    /// 变时转速不该跟着变。
    start: std::time::Instant,
    /// 判据只打印一次，否则 120fps 下 logcat 直接被刷爆。
    reported: bool,
}

/// 模糊的降采样倍率（边长）。4 是常见取值：σ 跟着除 4，采样数降 16 倍，
/// 而模糊本来就在丢高频，看不出来。见 shaders/glass_blur.frag。
const DOWNSCALE: i32 = 4;

/// 多尺度模糊最多几级。4 级在 σ=10 下等效 σ 已经 73.7 逻辑像素，比胶囊
/// 本身还高，再多没有意义。
const MAX_BLUR_LEVELS: usize = 4;

/// 第 1 级往后每级在**本级分辨率**下的 σ。
///
/// 2.0 配合「每级边长减半」给出等效 σ 逐级翻倍：第 i 级（i≥1）在全分辨率
/// 下的等效 σ 是 `2.0 × 4 × 2^i`。方差相加之后，σ=10 的四级依次是
/// 10 / 18.9 / 37.0 / 73.7。**改这个值就要改 material.rs 里那串数字**。
const SUB_SIGMA: f32 = 2.0;

/// 一个形状，逻辑像素、左上原点（Slint 的坐标系）。
///
/// `radius` 决定它是圆角矩形、圆还是胶囊——三者在 glass_sdf.glsl 里是同一个
/// SDF，见那边的文件头。
#[derive(Clone, Copy, Default)]
struct Shape {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
}

/// 一帧里要打进 shader 的几何与光照。
///
/// 捆成一个结构体而不是十个参数：`set_uniforms` 的签名会直接撞上 clippy 的
/// `too_many_arguments`，而且这些值本来就是**一起**算出来的（都来自同一次
/// 「逻辑像素 → 物理像素 → 覆盖矩形局部坐标」的换算），分开传很容易出现
/// 一半用新值一半用旧值。
struct FrameUniforms {
    /// 覆盖矩形（多形状时是并集外接框）的逻辑尺寸。
    size: [f32; 2],
    /// 覆盖矩形左下角在窗口帧缓冲里的位置，物理像素。
    origin_px: [f32; 2],
    scale: f32,
    count: i32,
    /// 各形状中心，相对覆盖矩形左下角的逻辑像素，**y 向上**。
    center: [[f32; 2]; 2],
    half: [[f32; 2]; 2],
    radius: [f32; 2],
    /// 已归一化的光源方向。
    light_dir: [f32; 2],
    /// 0 = 不模糊 / 1 = 单级 / ≥2 = 多尺度。和 material 的 blur_levels 不是
    /// 一回事：blur_radius 为 0 时这里是 0。
    blur_levels: i32,
}

/// 把探针挂到窗口上。桌面与 Android 走同一条路径。
///
/// 挂不上（渲染器不支持 notifier）时**把那一格还回去**并报一行：不还的话
/// 画面上会空出一个 slot，看起来像「GL 玻璃画出来是透明的」——把「没挂上」
/// 误读成「画错了」，是这种探针最容易浪费时间的一种失败。
pub fn install(app: &crate::GlassSpike) {
    let weak = app.as_weak();

    // ---- 摆法：自绘玻璃盖在哪儿（见 .slint 的 gl-overlay-mode）----
    //
    // over-full / over-blur 是 ADR-016 决策四那条「决策已定、实现待验」的
    // 两种读法：同样是「原生负责模糊、自绘负责折射与高光」，差别在**谁画
    // 渐变与描边**。两种都要能一键切，否则判不出哪一种可用。
    let layout = std::env::var("GLASS_GL_LAYOUT").unwrap_or_else(|_| "slot".into());
    let mode = match layout.as_str() {
        "slot" => 0,
        "over-full" => 1,
        "over-blur" => 2,
        "merge" => 3,
        other => {
            plog!(
                "PROBE GL GLASS_GL_LAYOUT={other} 不认识（认 slot/over-full/over-blur/merge），按 slot 跑"
            );
            0
        }
    };
    app.set_gl_overlay_mode(mode);

    // ---- 质量档（ADR-016 决策五：整条表面一起降，不逐效果开关）----
    let quality_name = std::env::var("GLASS_GL_QUALITY").unwrap_or_else(|_| "high".into());
    let quality = GlassQuality::parse(&quality_name).unwrap_or_else(|| {
        // 认不出来时**要吵**。静默回落到 High 的话，拼错一个字母就会得到
        // 一份「标签写着 balanced、实际跑的是 high」的记录——第一轮
        // GLASS_COUNT=10 只画 6 条就是这个形状。
        plog!(
            "PROBE GL GLASS_GL_QUALITY={quality_name} 不认识（认 high/balanced/low），按 high 跑"
        );
        GlassQuality::High
    });

    // ---- 材质档 ----
    //
    //   phase2（默认） Phase 2 全套：多尺度模糊 + 动态高光 + 融合
    //   p0             Phase 1 验收的那一档：单级模糊 + 折射 + Fresnel
    //   baseline       与 Flutter 的 shader 路径同式，用来判「像不像」
    //
    // 三档都留着是必须的：只有最新那档的话，画面上的差别分不清哪些是
    // 「自绘补的新能力」、哪些是「移植没对上」。
    let material_env = std::env::var("GLASS_GL_MATERIAL").unwrap_or_else(|_| "phase2".into());
    let (mut material, mut material_name) = match material_env.as_str() {
        "baseline" => {
            let mut m = GlassMaterial::FLUTTER_BASELINE;
            // **这一行很要紧**：Flutter 的 shader 路径不叠加模糊。那边
            // `ImageFilter.shader` 与 `ImageFilter.blur(10)` 是二选一
            // （floating_navigation_bar.dart 的 supportsShader 分支），
            // `blur_radius: 10.0` 记的是**回退档**的值，不是 shader 档的值。
            //
            // 不清零的话这个「对照档」会比 Flutter 多一层模糊，拿它判「像不像」
            // 等于拿一个并不存在的参照物去比。
            m.blur_radius = 0.0;
            (
                m,
                String::from("FLUTTER_BASELINE(无模糊，与 shader 路径同式)"),
            )
        }
        "p0" => (GlassMaterial::P0_TARGET, String::from("P0_TARGET")),
        "phase2" => (GlassMaterial::PHASE2_TARGET, String::from("PHASE2_TARGET")),
        other => {
            plog!(
                "PROBE GL GLASS_GL_MATERIAL={other} 不认识（认 phase2/p0/baseline），按 phase2 跑"
            );
            (GlassMaterial::PHASE2_TARGET, String::from("PHASE2_TARGET"))
        }
    };

    // 质量档整体降级。放在材质档之后、摆法之前：降级改的是「怎么画」，
    // 而摆法改的是「哪一层归谁画」，后者必须有最终话语权。
    material = material.at_quality(quality);
    if !quality.draws_overlay() {
        // Low 档要省的就是 shader 那一趟，所以是整个不挂，不是挂上去画个空。
        plog!("PROBE GL 质量档 low：自绘层整个不画，只留原生 backdrop-blur");
        app.set_gl_overlay_visible(false);
        app.set_gl_status(slint::format!(
            "GL: 质量=low，自绘层不画（只有原生 backdrop-blur）"
        ));
        return;
    }

    // ---- 摆法对材质的影响 ----
    match mode {
        1 => {
            // 读法甲：原生画全套（模糊 + 渐变 + 描边），自绘只加折射与高光。
            // 自绘这层再画一遍渐变与描边就是双份，所以清零。
            material.blur_radius = 0.0;
            material.surface_alpha = [0.0; 3];
            material.border_alpha = 0.0;
            material_name.push_str("+over-full(材质层归原生)");
        }
        2 => {
            // 读法乙：原生只画模糊（透明、无描边，见 .slint 的 blur-only），
            // 渐变与描边仍由自绘画——于是自绘采到的背板里没有描边可抹。
            material.blur_radius = 0.0;
            material_name.push_str("+over-blur(材质层归自绘)");
        }
        _ => {}
    }

    // 光源方向钉死在某个角度（度），同时停掉自转。
    //
    // 和融合那个间距同一个理由：动态高光**动起来**才看得出是动态的，但
    // 「某一帧的光斑在哪」取决于截图时刻减去启动耗时，而启动耗时每次不同。
    // 要拿静态图当证据就得能钉住角度；不设这个变量时照常自转。
    if let Ok(v) = std::env::var("GLASS_GL_LIGHT")
        && let Ok(deg) = v.parse::<f32>()
    {
        material.light_angle = deg.to_radians();
        material.light_speed = 0.0;
    }

    // 模糊单独可关：和原生 backdrop-blur 并排时，关掉它才看得出
    // 「折射 + 高光」这两件原生做不到的事各自贡献了多少。
    if std::env::var("GLASS_GL_BLUR")
        .map(|v| v == "0")
        .unwrap_or(false)
    {
        material.blur_radius = 0.0;
    }

    // 档位印到画面上。截图自带档位，核对不再依赖「我记得当时设的是几」
    // ——这是第一轮记错档位之后立的规矩，Phase 2 的开关更多，更需要它。
    app.set_gl_status(slint::format!(
        "GL: 材质={} 质量={} 摆法={} σ={} 级={} 厚={} 光={:.0}°{} 融合k={}",
        material_name,
        quality_name,
        layout,
        material.blur_radius,
        material.blur_levels,
        material.thickness,
        material.light_angle.to_degrees(),
        if material.light_speed > 0.0 {
            slint::format!("+{:.2}rad/s", material.light_speed)
        } else {
            slint::SharedString::from("(钉死)")
        },
        material.merge_k
    ));

    let mut overlay: Option<Overlay> = None;
    // 初始化失败只报一次：失败通常是 shader 编译错，每帧打一遍没有新信息。
    let mut init_failed = false;

    let result = app.window().set_rendering_notifier(move |state, api| {
        let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
            // 开了 wgpu-probe 又开这个，就会走到这里。
            if matches!(state, slint::RenderingState::RenderingSetup) {
                plog!("PROBE GL 拿到的不是 NativeOpenGL，这个探针不适用（两个探针不要同时开）");
            }
            return;
        };

        match state {
            slint::RenderingState::RenderingSetup => {
                // SAFETY: Slint 保证回调期间 GL 上下文是 current 的。
                let gl =
                    unsafe { glow::Context::from_loader_function_cstr(|s| get_proc_address(s)) };
                let version = gl.version().clone();
                // SAFETY: 同上。
                let (renderer, glsl) = unsafe {
                    (
                        gl.get_parameter_string(glow::RENDERER),
                        gl.get_parameter_string(glow::SHADING_LANGUAGE_VERSION),
                    )
                };
                plog!(
                    "PROBE GL setup version={}.{} embedded={} renderer={renderer} glsl={glsl}",
                    version.major,
                    version.minor,
                    version.is_embedded
                );
                match Overlay::new(gl, material, material_name.clone()) {
                    Ok(o) => overlay = Some(o),
                    Err(err) => {
                        init_failed = true;
                        plog!("PROBE GL 初始化失败：{err}");
                    }
                }
            }
            slint::RenderingState::AfterRendering => {
                if init_failed {
                    return;
                }
                let (Some(overlay), Some(app)) = (overlay.as_mut(), weak.upgrade()) else {
                    return;
                };
                if !app.get_gl_overlay_visible() {
                    return;
                }
                let scale = app.window().scale_factor();
                // 形状由 .slint 算，Rust 不重算一遍布局——重算就会和原生胶囊
                // 的摆法漂移，而这两者摆得一不一样正是要看的东西。
                //
                // 定长数组而不是 Vec：这段每帧都跑，120fps 下一帧一次堆分配
                // 没有必要。
                let mut shapes = [Shape::default(); 2];
                shapes[0] = Shape {
                    x: app.get_overlay_x(),
                    y: app.get_overlay_y(),
                    w: app.get_overlay_w(),
                    h: app.get_overlay_h(),
                    radius: app.get_overlay_radius(),
                };
                let count = if app.get_overlay2_visible() {
                    shapes[1] = Shape {
                        x: app.get_overlay2_x(),
                        y: app.get_overlay2_y(),
                        w: app.get_overlay2_w(),
                        h: app.get_overlay2_h(),
                        radius: app.get_overlay2_radius(),
                    };
                    2
                } else {
                    1
                };
                overlay.draw(&shapes[..count], scale);
            }
            slint::RenderingState::RenderingTeardown => {
                if let Some(o) = overlay.take() {
                    o.destroy();
                }
            }
            _ => {}
        }
    });

    if let Err(err) = result {
        plog!("PROBE GL 挂不上渲染回调：{err}（这一轮没有 GL 玻璃）");
        app.set_gl_overlay_visible(false);
    }
}

impl Overlay {
    fn new(
        gl: glow::Context,
        material: GlassMaterial,
        material_name: String,
    ) -> Result<Self, String> {
        // GLES 与桌面 GL 的版本声明不同，正文是同一份。
        let embedded = gl.version().is_embedded;
        let header = if embedded {
            "#version 300 es\nprecision highp float;\nprecision highp sampler2D;\n"
        } else {
            "#version 330 core\n"
        };

        // 全屏三角形：三个顶点覆盖整个 viewport，不用顶点缓冲，也少一条
        // 对角线上的重复片元。
        let vs_src = format!(
            "{header}
void main() {{
    vec2 p = vec2(float((gl_VertexID & 1) << 2) - 1.0,
                  float((gl_VertexID & 2) << 1) - 1.0);
    gl_Position = vec4(p, 0.0, 1.0);
}}"
        );
        // 形状与法线单独一个文件，拼在玻璃 shader 前面。模糊那趟不需要它
        // （它模糊的是整块矩形，形状由玻璃那趟的 coverage 裁），所以只拼这一个。
        let fs_src = format!(
            "{header}{}{}",
            include_str!("../shaders/glass_sdf.glsl"),
            include_str!("../shaders/glass_overlay.frag")
        );

        let blur_src = format!("{header}{}", include_str!("../shaders/glass_blur.frag"));

        // SAFETY: 全部在 Slint 的渲染回调里，上下文 current。
        unsafe {
            let program = link(&gl, &vs_src, &fs_src).map_err(|e| format!("玻璃 program：{e}"))?;
            let blur_program =
                link(&gl, &vs_src, &blur_src).map_err(|e| format!("模糊 program：{e}"))?;

            // 核心 profile 下没有默认 VAO，不绑一个 draw_arrays 直接是
            // INVALID_OPERATION。
            let vao = gl
                .create_vertex_array()
                .map_err(|e| format!("create_vertex_array: {e}"))?;

            let backdrop = gl
                .create_texture()
                .map_err(|e| format!("create_texture: {e}"))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(backdrop));
            // clamp-to-edge 是必须的：折射把 uv 推出边界是常态，repeat 的话
            // 边缘会采到对侧的内容。
            for (k, v) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, k, v);
            }
            gl.bind_texture(glow::TEXTURE_2D, None);

            // 模糊金字塔的纹理，每级两张。尺寸在第一帧按实际矩形分配，
            // 这里只是把名字要出来——没 tex_image_2d 之前它们不占显存。
            let mut names = Vec::with_capacity(MAX_BLUR_LEVELS * 2);
            for _ in 0..MAX_BLUR_LEVELS * 2 {
                names.push(new_blur_texture(&gl)?);
            }
            let blur_tex: [[glow::Texture; 2]; MAX_BLUR_LEVELS] =
                std::array::from_fn(|i| [names[i * 2], names[i * 2 + 1]]);
            let blur_fbo = gl
                .create_framebuffer()
                .map_err(|e| format!("create_framebuffer: {e}"))?;

            Ok(Self {
                gl,
                program,
                blur_program,
                vao,
                backdrop,
                tex_size: (0, 0),
                backdrop_origin: (0.0, 0.0),
                backdrop_size: (1.0, 1.0),
                blur_fbo,
                blur_tex,
                blur_sizes: std::cell::Cell::new([(0, 0); MAX_BLUR_LEVELS]),
                material,
                material_name,
                start: std::time::Instant::now(),
                reported: false,
            })
        }
    }

    /// 把背板糊成一座金字塔，返回 (细级, 粗级) 两张纹理。
    ///
    /// σ 为 0 时（`GLASS_GL_BLUR=0`、材质 `blur_radius` 为 0，或模糊整个交给
    /// 了原生 `backdrop-blur`）**原样返回清晰背板**，不走任何 pass——这就是
    /// 和 Flutter 版对齐的那一档，两条路要能一键切换，否则「像不像」没有
    /// 参照物。
    ///
    /// # 金字塔怎么搭（#130 Phase 2「多尺度模糊」）
    ///
    /// - 第 0 级：全分辨率背板 → 边长 1/`DOWNSCALE`，σ 用 `sigma_px / 4`。
    ///   σ 必须跟着降采样一起除，模糊的**视觉半径**才不变；漏掉这一除等于
    ///   把模糊悄悄调弱四倍，看起来像「blur 没生效」。
    /// - 第 i 级（i≥1）：上一级的输出 → 边长再减半，σ 固定 [`SUB_SIGMA`]。
    ///
    /// 于是第 i 级在全分辨率下的等效 σ 是
    /// `sqrt(σ² + Σ_{j=1..i} (SUB_SIGMA × DOWNSCALE × 2^j)²)`（高斯卷积的
    /// 方差相加）。σ=10 时四级依次是 **10 / 18.9 / 37.0 / 73.7** 逻辑像素。
    ///
    /// **每多一级只要约 1/5 的代价**：像素数是上一级的 1/4，而每像素的采样
    /// 数也更少（σ 在本级分辨率下固定 2.0，半径 6；第 0 级 σ=2.5，半径 8）。
    /// 这正是多尺度比「把单级 σ 调大」划算的地方——后者的采样数随 σ 线性涨，
    /// 前者按 1/4 几何衰减。
    ///
    /// # Safety
    /// 调用方保证 GL 上下文 current，且已经把深度/裁剪/混合关掉。
    unsafe fn blur(
        &self,
        w: i32,
        h: i32,
        sigma_px: f32,
        levels: u32,
    ) -> (glow::Texture, glow::Texture) {
        if sigma_px <= 0.0 {
            return (self.backdrop, self.backdrop);
        }
        let levels = (levels.max(1) as usize).min(MAX_BLUR_LEVELS);
        let gl = &self.gl;
        let mut sizes = self.blur_sizes.get();

        unsafe {
            gl.use_program(Some(self.blur_program));
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.blur_fbo));
            gl.active_texture(glow::TEXTURE0);
            let loc = |n: &str| gl.get_uniform_location(self.blur_program, n);
            gl.uniform_1_i32(loc("u_src").as_ref(), 0);

            let mut src = self.backdrop;
            for level in 0..levels {
                let (bw, bh) = if level == 0 {
                    ((w / DOWNSCALE).max(1), (h / DOWNSCALE).max(1))
                } else {
                    let prev = sizes[level - 1];
                    ((prev.0 / 2).max(1), (prev.1 / 2).max(1))
                };
                let sigma = if level == 0 {
                    (sigma_px / DOWNSCALE as f32).max(0.5)
                } else {
                    SUB_SIGMA
                };

                if sizes[level] != (bw, bh) {
                    for tex in self.blur_tex[level] {
                        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
                        gl.tex_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glow::RGBA8 as i32,
                            bw,
                            bh,
                            0,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(None),
                        );
                    }
                    sizes[level] = (bw, bh);
                }

                gl.viewport(0, 0, bw, bh);
                gl.uniform_1_f32(loc("u_sigma").as_ref(), sigma);
                gl.uniform_2_f32(loc("u_target_size").as_ref(), bw as f32, bh as f32);

                // 横向：上一级（或原始背板）→ 本级的中间纹理。
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(self.blur_tex[level][0]),
                    0,
                );
                gl.bind_texture(glow::TEXTURE_2D, Some(src));
                gl.uniform_2_f32(loc("u_direction").as_ref(), 1.0 / bw as f32, 0.0);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);

                // 竖向：中间纹理 → 本级输出。
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(self.blur_tex[level][1]),
                    0,
                );
                gl.bind_texture(glow::TEXTURE_2D, Some(self.blur_tex[level][0]));
                gl.uniform_2_f32(loc("u_direction").as_ref(), 0.0, 1.0 / bh as f32);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);

                src = self.blur_tex[level][1];
            }

            self.blur_sizes.set(sizes);
            (self.blur_tex[0][1], self.blur_tex[levels - 1][1])
        }
    }

    /// 把这一帧的玻璃画上去。
    ///
    /// `shapes` 由 `.slint` 算好传进来（1 个或 2 个），Rust 只把它们换算成
    /// 物理像素并求并集外接框——**不重算布局**，重算就会和原生胶囊的摆法
    /// 漂移，而两者摆得一不一样正是要看的东西。
    fn draw(&mut self, shapes: &[Shape], scale: f32) {
        let gl = &self.gl;
        // SAFETY: 在渲染回调里，上下文 current；下面每一处改动的状态都在
        // 末尾按查到的原值恢复，理由见复原那一段的注释。
        unsafe {
            // 当前 viewport 就是整个窗口帧缓冲，顺便当窗口尺寸用——不用再去
            // 问一次 window size，少一处可能对不上的来源。
            let mut vp = [0i32; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut vp);
            let fb_h = vp[3];

            // ---- 覆盖矩形 = 所有形状的并集外接框，物理像素 ----
            //
            // 多形状时必须按并集来：viewport 收在哪儿 shader 就只能画到哪儿，
            // 按第一个形状收的话，融合出来的颈部会被裁掉半截。
            let mut x0 = i32::MAX;
            let mut y0 = i32::MAX;
            let mut x1 = i32::MIN;
            let mut y1 = i32::MIN;
            // 每个形状在窗口帧缓冲里的位置（左下原点），物理像素。
            let mut boxes = [[0i32; 4]; 2];
            for (i, shape) in shapes.iter().enumerate() {
                let sx = (shape.x * scale).round() as i32;
                let sw = (shape.w * scale).round() as i32;
                let sh = (shape.h * scale).round() as i32;
                // Slint 的 y 从上往下，GL 的帧缓冲原点在左下。
                let sy = fb_h - ((shape.y * scale).round() as i32) - sh;
                boxes[i] = [sx, sy, sw, sh];
                x0 = x0.min(sx);
                y0 = y0.min(sy);
                x1 = x1.max(sx + sw);
                y1 = y1.max(sy + sh);
            }
            let (x, y, w, h) = (x0, y0, x1 - x0, y1 - y0);
            if w <= 0 || h <= 0 {
                return;
            }

            // ---- 先把渲染器的 GL 状态存下来，再动任何东西 ----
            //
            // 必须复原。Skia 的 GrDirectContext 自己缓存了一份 GL 状态的影子，
            // 而我们拿不到它的 reset_context()；femtovg 同理。不复原的话下一帧
            // 的渲染会以「状态还是我离开时那样」为前提发命令，症状是画面错乱
            // 而不是报错，很难往这边想。
            //
            // **存档这一段 Phase 1 放在背板拷贝之后，那是错的**：拷贝本身就
            // 会 bind_texture，于是存下来的「原纹理」其实是我们自己那张背板，
            // 复原等于把渲染器的绑定换成了我们的。症状同样是静默的，这一轮
            // 一并挪到最前面。
            let prev_fbo = gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING);
            let prev_program = gl.get_parameter_i32(glow::CURRENT_PROGRAM);
            let prev_vao = gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING);
            let prev_active = gl.get_parameter_i32(glow::ACTIVE_TEXTURE);
            // 三个纹理单元都要存：清晰背板、细级、粗级各占一个。
            let mut prev_tex = [0i32; 3];
            for (unit, slot) in prev_tex.iter_mut().enumerate() {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                *slot = gl.get_parameter_i32(glow::TEXTURE_BINDING_2D);
            }
            let prev_blend = gl.is_enabled(glow::BLEND);
            let prev_depth = gl.is_enabled(glow::DEPTH_TEST);
            let prev_scissor = gl.is_enabled(glow::SCISSOR_TEST);
            let prev_cull = gl.is_enabled(glow::CULL_FACE);
            let prev_blend_src_rgb = gl.get_parameter_i32(glow::BLEND_SRC_RGB);
            let prev_blend_dst_rgb = gl.get_parameter_i32(glow::BLEND_DST_RGB);
            let prev_blend_src_alpha = gl.get_parameter_i32(glow::BLEND_SRC_ALPHA);
            let prev_blend_dst_alpha = gl.get_parameter_i32(glow::BLEND_DST_ALPHA);

            if !self.reported {
                // 判据 1：这个回调里到底绑着什么帧缓冲。
                plog!(
                    "PROBE GL AfterRendering draw_fbo={prev_fbo} read_fbo={} viewport={vp:?} scale={scale} rect_px=({x},{y},{w},{h}) 形状数={}",
                    gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING),
                    shapes.len()
                );
            }

            // ---- 判据 2：把本帧已经画好的内容拷进自己的纹理 ----
            //
            // 这是整个关口 2 的那一下。copy_tex_sub_image_2d 读的是当前
            // READ_FRAMEBUFFER，也就是 Slint 刚画完、还没 present 的那一帧。
            //
            // 拷贝区域要**比覆盖矩形大一圈**，边距取 max(3σ, 折射强度)：
            //
            // - 3σ 那一半给模糊。只拷自己那一块的话，边缘只能 clamp 自己那
            //   一行像素，糊出来是一道假硬边；3σ 和 Slint 原生 backdrop-blur
            //   同口径（它的文档说最远采到该值的三倍）——#130 关口 2 问的
            //   「捕获范围」就是这件事。
            // - 折射强度那一半是 Phase 2 补的。折射把采样点沿法线推出矩形最多
            //   refraction_strength 个逻辑像素，而 Phase 1 这里只算了 3σ：
            //   σ=0 的 baseline 档边距于是是 0，外缘那 7px 一直在采 clamp 值。
            //   症状轻到三轮都没看出来（边上一圈颜色发直），但它是错的；
            //   决策四那两档（模糊归原生、自绘 σ=0）会把它放大成一圈硬边。
            //
            // 窗口边上截断是正常的：那里本来就没有更多内容，clamp 是对的。
            let pad_logical =
                (self.material.blur_radius * 3.0).max(self.material.refraction_strength);
            let pad = (pad_logical * scale).ceil() as i32;
            let px = (x - pad).max(0);
            let py = (y - pad).max(0);
            let pw = (x + w + pad).min(vp[2]) - px;
            let ph = (y + h + pad).min(vp[3]) - py;
            // 覆盖矩形左下角落在这张带边距纹理里的位置。
            self.backdrop_origin = ((x - px) as f32, (y - py) as f32);
            self.backdrop_size = (pw as f32, ph as f32);

            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.backdrop));
            if self.tex_size != (pw, ph) {
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    pw,
                    ph,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                self.tex_size = (pw, ph);
            }
            gl.copy_tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, px, py, pw, ph);
            if !self.reported {
                let err = gl.get_error();
                plog!(
                    "PROBE GL 结论 背板拷贝 {} (glGetError=0x{err:x}, {pw}x{ph}，含 {pad}px 边距)",
                    if err == glow::NO_ERROR {
                        "成功——拿到的是本帧真实帧缓冲"
                    } else {
                        "失败"
                    }
                );
            }

            // ---- 画玻璃 ----
            gl.bind_vertex_array(Some(self.vao));
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::CULL_FACE);
            // 模糊那几趟是覆盖写，不能开混合；下面画玻璃之前再开。
            gl.disable(glow::BLEND);

            // ---- P0 的「实时背景模糊」：在真实背板上做，不是在合成背板上 ----
            //
            // 这一步让自绘这条**同时**有模糊和折射。Flutter 版做不到：那边
            // ImageFilter.shader 与 ImageFilter.blur 是二选一（见
            // floating_navigation_bar.dart 的 supportsShader 分支）。
            //
            // Phase 2 起它返回两张（细级 + 粗级）：边缘要清楚、体部要糊这件事
            // 单级做不到，见 glass_overlay.frag 的 backdrop()。
            let sigma_px = self.material.blur_radius * scale;
            let (fine, coarse) = self.blur(pw, ph, sigma_px, self.material.blur_levels);

            gl.bind_framebuffer(glow::FRAMEBUFFER, framebuffer_of(prev_fbo));
            gl.use_program(Some(self.program));
            gl.enable(glow::BLEND);
            // 形状由 shader 的 coverage 给，所以是标准的 src-alpha 混合。
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            // viewport 收到覆盖矩形上，全屏三角形就只覆盖这一块；shader 里
            // 仍然用 gl_FragCoord（窗口坐标）减去 origin。
            gl.viewport(x, y, w, h);

            // 三张背板一起绑。σ 为 0 时 fine / coarse 就是清晰背板本身
            // （blur() 原样返回），绑的是同一个纹理名，不浪费。
            //
            // 为什么不按 blur_levels 少绑几张：GLES 下采样一张**没分配过**的
            // 纹理是未定义行为，而 shader 里那几个分支是运行时的，编译器不会
            // 帮忙剪掉采样指令。三张都绑是最省心的写法，代价只有两次 bind。
            for (unit, tex) in [self.backdrop, fine, coarse].into_iter().enumerate() {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            }

            let m = self.material;
            // 光源方向由 Rust 推（AGENTS.md §2：换宿主时这段不该重写）。
            // 用 Instant 而不是帧计数——帧率在 60 和 120 之间变时转速不该跟着变。
            let angle = m.light_angle + m.light_speed * self.start.elapsed().as_secs_f32();
            let mut frame = FrameUniforms {
                size: [w as f32 / scale, h as f32 / scale],
                origin_px: [x as f32, y as f32],
                scale,
                count: shapes.len() as i32,
                center: [[0.0; 2]; 2],
                half: [[0.0; 2]; 2],
                radius: [0.0; 2],
                light_dir: [angle.cos(), angle.sin()],
                // blur_levels 和材质里那个不是一回事：σ 为 0（模糊交给原生、
                // 或 baseline 档）时这里是 0，shader 直接采清晰背板。
                blur_levels: if sigma_px > 0.0 {
                    m.blur_levels as i32
                } else {
                    0
                },
            };
            // 形状的中心与半宽半高，相对覆盖矩形左下角的逻辑像素。
            //
            // 按**取整之后**的物理像素反算，而不是直接用 .slint 给的逻辑值：
            // shader 里的 p 是 gl_FragCoord 减 origin 得来的，整数基准；
            // 两边用同一套取整，形状才不会在 HiDPI 下偏半个像素。
            for (i, shape) in shapes.iter().enumerate() {
                let [bx, by, bw, bh] = boxes[i];
                frame.center[i] = [
                    (bx - x) as f32 / scale + bw as f32 / scale * 0.5,
                    (by - y) as f32 / scale + bh as f32 / scale * 0.5,
                ];
                frame.half[i] = [bw as f32 / scale * 0.5, bh as f32 / scale * 0.5];
                frame.radius[i] = shape.radius;
            }

            self.set_uniforms(&frame, &m);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            if !self.reported {
                let err = gl.get_error();
                plog!(
                    "PROBE GL 结论 自定义 shader 覆盖绘制 {} (glGetError=0x{err:x})",
                    if err == glow::NO_ERROR {
                        "成功"
                    } else {
                        "失败"
                    }
                );
                plog!(
                    "PROBE GL 材质 {} 模糊σ={}×{}级（物理 {sigma_px}px，首级降采样 1/{DOWNSCALE}）过渡深度={} 折射={} 带宽={} glint={} 带宽={} 集中度={} 顶边={} Fresnel={}^{} 融合k={} 圆角={:?}",
                    self.material_name,
                    m.blur_radius,
                    frame.blur_levels,
                    m.thickness,
                    m.refraction_strength,
                    m.edge_highlight,
                    m.specular_strength,
                    m.specular_width,
                    m.specular_sharpness,
                    m.top_highlight,
                    m.fresnel_strength,
                    m.fresnel_power,
                    m.merge_k,
                    &frame.radius[..shapes.len()]
                );
                self.reported = true;
            }

            // ---- 复原 ----
            for (unit, name) in prev_tex.iter().enumerate() {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                gl.bind_texture(glow::TEXTURE_2D, texture_of(*name));
            }
            gl.active_texture(prev_active as u32);
            gl.viewport(vp[0], vp[1], vp[2], vp[3]);
            gl.blend_func_separate(
                prev_blend_src_rgb as u32,
                prev_blend_dst_rgb as u32,
                prev_blend_src_alpha as u32,
                prev_blend_dst_alpha as u32,
            );
            set_enabled(gl, glow::BLEND, prev_blend);
            set_enabled(gl, glow::DEPTH_TEST, prev_depth);
            set_enabled(gl, glow::SCISSOR_TEST, prev_scissor);
            set_enabled(gl, glow::CULL_FACE, prev_cull);
            gl.bind_vertex_array(vertex_array_of(prev_vao));
            gl.use_program(program_of(prev_program));
            gl.bind_framebuffer(glow::FRAMEBUFFER, framebuffer_of(prev_fbo));
        }
    }

    /// 几何、光照与材质参数进 uniform。
    ///
    /// 材质那部分的值全部来自 `GlassMaterial`，不在这里另写一套常数——那张表
    /// 是单一来源，shader 和宿主都只是它的消费者（AGENTS.md §2）。
    ///
    /// # Safety
    /// 调用方保证 GL 上下文 current 且 `self.program` 已 use。
    unsafe fn set_uniforms(&self, f: &FrameUniforms, m: &GlassMaterial) {
        let gl = &self.gl;
        unsafe {
            let loc = |n: &str| gl.get_uniform_location(self.program, n);

            // ---- 覆盖矩形 ----
            gl.uniform_2_f32(loc("u_size").as_ref(), f.size[0], f.size[1]);
            gl.uniform_2_f32(loc("u_origin").as_ref(), f.origin_px[0], f.origin_px[1]);
            gl.uniform_1_f32(loc("u_scale").as_ref(), f.scale);

            // ---- 形状。数组一次传满两个，shader 按 u_shape_count 决定读几个 ----
            //
            // 只传 count 个也行，但数组 uniform 的部分更新很容易写成
            // 「第二个形状还留着上一帧的值」，而那种错只有在 count 从 2 变回 1
            // 再变回 2 的那一帧才看得见。传满省掉这种时序问题。
            gl.uniform_1_i32(loc("u_shape_count").as_ref(), f.count);
            gl.uniform_2_f32_slice(
                loc("u_shape_center").as_ref(),
                &[
                    f.center[0][0],
                    f.center[0][1],
                    f.center[1][0],
                    f.center[1][1],
                ],
            );
            gl.uniform_2_f32_slice(
                loc("u_shape_half").as_ref(),
                &[f.half[0][0], f.half[0][1], f.half[1][0], f.half[1][1]],
            );
            gl.uniform_1_f32_slice(loc("u_shape_radius").as_ref(), &f.radius);
            gl.uniform_1_f32(loc("u_merge_k").as_ref(), m.merge_k);

            // ---- 折射与高光 ----
            gl.uniform_1_f32(loc("u_rim_width").as_ref(), m.edge_highlight);
            gl.uniform_1_f32(loc("u_refraction").as_ref(), m.refraction_strength);
            gl.uniform_1_f32(loc("u_specular_strength").as_ref(), m.specular_strength);
            gl.uniform_1_f32(loc("u_specular_width").as_ref(), m.specular_width);
            gl.uniform_1_f32(loc("u_specular_sharpness").as_ref(), m.specular_sharpness);
            gl.uniform_1_f32(loc("u_top_highlight").as_ref(), m.top_highlight);
            gl.uniform_2_f32(loc("u_light_dir").as_ref(), f.light_dir[0], f.light_dir[1]);
            gl.uniform_1_f32(loc("u_fresnel_power").as_ref(), m.fresnel_power);
            gl.uniform_1_f32(loc("u_fresnel_strength").as_ref(), m.fresnel_strength);

            // ---- 半透明材质层（Flutter 的 DecoratedBox 那一半）----
            gl.uniform_3_f32(
                loc("u_surface_alpha").as_ref(),
                m.surface_alpha[0],
                m.surface_alpha[1],
                m.surface_alpha[2],
            );
            gl.uniform_1_f32(loc("u_border_alpha").as_ref(), m.border_alpha);
            gl.uniform_1_f32(loc("u_border_width").as_ref(), m.border_width);
            gl.uniform_1_f32(loc("u_opacity").as_ref(), m.opacity);

            // ---- 背板。三个纹理单元与那一圈边距 ----
            gl.uniform_2_f32(
                loc("u_backdrop_origin").as_ref(),
                self.backdrop_origin.0,
                self.backdrop_origin.1,
            );
            gl.uniform_2_f32(
                loc("u_backdrop_size").as_ref(),
                self.backdrop_size.0,
                self.backdrop_size.1,
            );
            gl.uniform_1_i32(loc("u_backdrop").as_ref(), 0);
            gl.uniform_1_i32(loc("u_blur_fine").as_ref(), 1);
            gl.uniform_1_i32(loc("u_blur_coarse").as_ref(), 2);
            gl.uniform_1_i32(loc("u_blur_levels").as_ref(), f.blur_levels);
            // thickness 在 Phase 2 的含义就是这个过渡深度，见 material.rs。
            gl.uniform_1_f32(loc("u_blur_depth").as_ref(), m.thickness);
        }
    }

    fn destroy(self) {
        // SAFETY: RenderingTeardown 仍在渲染线程、上下文 current。
        unsafe {
            self.gl.delete_program(self.program);
            self.gl.delete_program(self.blur_program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_texture(self.backdrop);
            self.gl.delete_framebuffer(self.blur_fbo);
            for level in self.blur_tex {
                for tex in level {
                    self.gl.delete_texture(tex);
                }
            }
        }
    }
}

/// `glGetIntegerv` 给的是裸的 name，glow 的 bind 要的是 NonZero 包装。
/// 0 表示「没有绑定」。下面三个只是同一件事的三个类型。
fn framebuffer_of(name: i32) -> Option<glow::Framebuffer> {
    std::num::NonZeroU32::new(name as u32).map(glow::NativeFramebuffer)
}

fn texture_of(name: i32) -> Option<glow::Texture> {
    std::num::NonZeroU32::new(name as u32).map(glow::NativeTexture)
}

fn vertex_array_of(name: i32) -> Option<glow::VertexArray> {
    std::num::NonZeroU32::new(name as u32).map(glow::NativeVertexArray)
}

fn program_of(name: i32) -> Option<glow::Program> {
    std::num::NonZeroU32::new(name as u32).map(glow::NativeProgram)
}

/// `glEnable` / `glDisable` 没有「设成这个 bool」的形式，包一下免得每处写两行。
///
/// # Safety
/// 调用方保证 GL 上下文 current。
unsafe fn set_enabled(gl: &glow::Context, cap: u32, on: bool) {
    unsafe {
        if on {
            gl.enable(cap);
        } else {
            gl.disable(cap);
        }
    }
}

/// 编译 + 链接一个 program，错误里带上 info log——GLSL 的报错全在那里面，
/// 丢了它就只剩「链接失败」四个字。
///
/// # Safety
/// 调用方保证 GL 上下文 current。
unsafe fn link(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let program = gl
            .create_program()
            .map_err(|e| format!("create_program: {e}"))?;
        for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
            let shader = gl
                .create_shader(kind)
                .map_err(|e| format!("create_shader: {e}"))?;
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                return Err(format!("编译失败：{}", gl.get_shader_info_log(shader)));
            }
            gl.attach_shader(program, shader);
            gl.delete_shader(shader);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(format!("链接失败：{}", gl.get_program_info_log(program)));
        }
        Ok(program)
    }
}

/// 模糊用的中间纹理。LINEAR + clamp-to-edge：降采样那一趟靠 LINEAR 盖掉
/// 走样，边缘靠 clamp 免得采到对侧。
///
/// # Safety
/// 调用方保证 GL 上下文 current。
unsafe fn new_blur_texture(gl: &glow::Context) -> Result<glow::Texture, String> {
    unsafe {
        let tex = gl
            .create_texture()
            .map_err(|e| format!("create_texture: {e}"))?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        for (k, v) in [
            (glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32),
            (glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, k, v);
        }
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(tex)
    }
}
