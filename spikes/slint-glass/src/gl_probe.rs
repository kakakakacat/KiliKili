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
//! 背板是整帧拷下来的，**玻璃自己也在那一帧里**——如果矩形压在原生胶囊上，
//! 采到的就是「玻璃 + 玻璃」，一帧比一帧白。所以这里的矩形由 `.slint` 让出
//! 最底下那个 slot（`gl-overlay-visible` 为真时原生胶囊整体上移一格），
//! 矩形里只有背景。这条约束换成真实应用同样成立，是 Phase 2 的输入之一。

use glow::HasContext;
use slint::ComponentHandle;

use crate::material::GlassMaterial;

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
    /// 玻璃矩形左下角在背板纹理里的位置，以及这张纹理的整体尺寸，物理像素。
    /// shader 要用它们把折射后的采样点换算成 uv，见 glass_overlay.frag。
    backdrop_origin: (f32, f32),
    backdrop_size: (f32, f32),
    /// 模糊的两张 ping-pong 纹理与它们的 FBO。尺寸是背板的 1/DOWNSCALE。
    blur_fbo: glow::Framebuffer,
    blur_tex: [glow::Texture; 2],
    /// Cell 而不是裸字段：`draw()` 里已经借着 `&self.gl`，再要个 `&mut self`
    /// 去改尺寸就借用冲突了。模糊这一步除了这个尺寸没有别的可变状态，
    /// 用 Cell 把它单独包起来，比把整条 draw 拆开更不容易写错。
    blur_size: std::cell::Cell<(i32, i32)>,
    /// 这一轮用哪组材质参数。**不在这里写常数**：那张表在 material.rs，
    /// 它是宿主与 shader 共同的单一来源（AGENTS.md §2）。
    material: GlassMaterial,
    material_name: &'static str,
    /// 判据只打印一次，否则 120fps 下 logcat 直接被刷爆。
    reported: bool,
}

/// 模糊的降采样倍率（边长）。4 是常见取值：σ 跟着除 4，采样数降 16 倍，
/// 而模糊本来就在丢高频，看不出来。见 shaders/glass_blur.frag。
const DOWNSCALE: i32 = 4;

/// 逻辑像素的矩形，左上原点（Slint 的坐标系）。
struct LogicalRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// 把探针挂到窗口上。桌面与 Android 走同一条路径。
///
/// 挂不上（渲染器不支持 notifier）时**把那一格还回去**并报一行：不还的话
/// 画面上会空出一个 slot，看起来像「GL 玻璃画出来是透明的」——把「没挂上」
/// 误读成「画错了」，是这种探针最容易浪费时间的一种失败。
pub fn install(app: &crate::GlassSpike) {
    let weak = app.as_weak();

    // 跑哪一档材质。默认 P0 全套（模糊 + 折射 + Fresnel + 高光 + 材质层），
    // 这是本单 Phase 1 要验收的东西；`GLASS_GL_MATERIAL=baseline` 切到
    // 「和 Flutter 完全同式」那一档，用来判「像不像」。
    //
    // 两档都留着是必须的：只有 P0 那档的话，看到的差别里分不清哪些是
    // 「自绘补的新能力」、哪些是「移植没对上」。
    let baseline = std::env::var("GLASS_GL_MATERIAL")
        .map(|v| v == "baseline")
        .unwrap_or(false);
    let (mut material, material_name) = if baseline {
        let mut m = GlassMaterial::FLUTTER_BASELINE;
        // **这一行很要紧**：Flutter 的 shader 路径不叠加模糊。那边
        // `ImageFilter.shader` 与 `ImageFilter.blur(10)` 是二选一
        // （floating_navigation_bar.dart 的 supportsShader 分支），
        // `blur_radius: 10.0` 记的是**回退档**的值，不是 shader 档的值。
        //
        // 不清零的话这个「对照档」会比 Flutter 多一层模糊，拿它判「像不像」
        // 等于拿一个并不存在的参照物去比。
        m.blur_radius = 0.0;
        (m, "FLUTTER_BASELINE(无模糊，与 shader 路径同式)")
    } else {
        (GlassMaterial::P0_TARGET, "P0_TARGET")
    };
    // 模糊单独可关：和原生 backdrop-blur 并排时，关掉它才看得出
    // 「折射 + 高光」这两件原生做不到的事各自贡献了多少。
    if std::env::var("GLASS_GL_BLUR")
        .map(|v| v == "0")
        .unwrap_or(false)
    {
        material.blur_radius = 0.0;
    }

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
                match Overlay::new(gl, material, material_name) {
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
                // 矩形由 .slint 算，Rust 不重算一遍布局——重算就会和原生胶囊
                // 的摆法漂移，而这两者摆得一不一样正是要看的东西。
                let rect = LogicalRect {
                    x: app.get_overlay_x(),
                    y: app.get_overlay_y(),
                    w: app.get_overlay_w(),
                    h: app.get_overlay_h(),
                };
                overlay.draw(rect, scale);
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
        material_name: &'static str,
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
        let fs_src = format!("{header}{}", include_str!("../shaders/glass_overlay.frag"));

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

            // 模糊的两张 ping-pong 纹理。尺寸在第一帧按实际矩形分配。
            let blur_tex = [new_blur_texture(&gl)?, new_blur_texture(&gl)?];
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
                blur_size: std::cell::Cell::new((0, 0)),
                material,
                material_name,
                reported: false,
            })
        }
    }

    /// 把背板模糊一遍，返回该拿去采样的那张纹理。
    ///
    /// σ 为 0（`GLASS_GL_BLUR=0` 或材质的 blur_radius 是 0）时**原样返回清晰
    /// 背板**，不走任何 pass——这就是和 Flutter 版对齐的那一档，两条路要能
    /// 一键切换，否则「像不像」没有参照物。
    ///
    /// # Safety
    /// 调用方保证 GL 上下文 current，且已经把深度/裁剪/混合关掉。
    unsafe fn blur(&self, w: i32, h: i32, sigma_px: f32) -> glow::Texture {
        if sigma_px <= 0.0 {
            return self.backdrop;
        }
        let gl = &self.gl;
        let bw = (w / DOWNSCALE).max(1);
        let bh = (h / DOWNSCALE).max(1);
        // σ 跟着降采样一起除，模糊的**视觉半径**才不变。漏掉这一除的话
        // 降采样等于顺带把模糊调弱四倍，看起来像「blur 没生效」。
        let sigma = (sigma_px / DOWNSCALE as f32).max(0.5);

        unsafe {
            if self.blur_size.get() != (bw, bh) {
                for tex in self.blur_tex {
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
                self.blur_size.set((bw, bh));
            }

            gl.use_program(Some(self.blur_program));
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.blur_fbo));
            gl.viewport(0, 0, bw, bh);
            gl.active_texture(glow::TEXTURE0);

            let loc = |n: &str| gl.get_uniform_location(self.blur_program, n);
            gl.uniform_1_f32(loc("u_sigma").as_ref(), sigma);
            gl.uniform_2_f32(loc("u_target_size").as_ref(), bw as f32, bh as f32);
            gl.uniform_1_i32(loc("u_src").as_ref(), 0);

            // 第一趟：全分辨率背板 → 1/4 的 tex[0]，横向。
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(self.blur_tex[0]),
                0,
            );
            gl.bind_texture(glow::TEXTURE_2D, Some(self.backdrop));
            gl.uniform_2_f32(loc("u_direction").as_ref(), 1.0 / bw as f32, 0.0);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            // 第二趟：tex[0] → tex[1]，竖向。
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(self.blur_tex[1]),
                0,
            );
            gl.bind_texture(glow::TEXTURE_2D, Some(self.blur_tex[0]));
            gl.uniform_2_f32(loc("u_direction").as_ref(), 0.0, 1.0 / bh as f32);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            self.blur_tex[1]
        }
    }

    fn draw(&mut self, rect: LogicalRect, scale: f32) {
        let gl = &self.gl;
        // SAFETY: 在渲染回调里，上下文 current；下面每一处改动的状态都在
        // 末尾按查到的原值恢复，理由见复原那一段的注释。
        unsafe {
            // 当前 viewport 就是整个窗口帧缓冲，顺便当窗口尺寸用——不用再去
            // 问一次 window size，少一处可能对不上的来源。
            let mut vp = [0i32; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut vp);
            let fb_h = vp[3];

            let x = (rect.x * scale).round() as i32;
            let w = (rect.w * scale).round() as i32;
            let h = (rect.h * scale).round() as i32;
            // Slint 的 y 从上往下，GL 的帧缓冲原点在左下。
            let y = fb_h - ((rect.y * scale).round() as i32) - h;
            if w <= 0 || h <= 0 {
                return;
            }

            if !self.reported {
                // 判据 1：这个回调里到底绑着什么帧缓冲。
                let draw_fbo = gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING);
                let read_fbo = gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING);
                plog!(
                    "PROBE GL AfterRendering draw_fbo={draw_fbo} read_fbo={read_fbo} viewport={vp:?} scale={scale} rect_px=({x},{y},{w},{h})"
                );
            }

            // ---- 判据 2：把本帧已经画好的内容拷进自己的纹理 ----
            //
            // 这是整个关口 2 的那一下。copy_tex_sub_image_2d 读的是当前
            // READ_FRAMEBUFFER，也就是 Slint 刚画完、还没 present 的那一帧。
            // 拷贝区域要**比玻璃矩形大一圈**：模糊会采到矩形外面，只拷自己
            // 那一块的话边缘只能 clamp 自己那一行像素，糊出来是一道假硬边。
            // 边距取 3σ，和 Slint 原生 backdrop-blur 的口径一致（它的文档说
            // 最远采到该值的三倍）——#130 关口 2 问的「捕获范围」就是这件事。
            //
            // 窗口边上截断是正常的：那里本来就没有更多内容，clamp 是对的。
            let pad = (self.material.blur_radius * scale * 3.0).ceil() as i32;
            let px = (x - pad).max(0);
            let py = (y - pad).max(0);
            let pw = (x + w + pad).min(vp[2]) - px;
            let ph = (y + h + pad).min(vp[3]) - py;
            // 玻璃矩形左下角落在这张带边距纹理里的位置。
            self.backdrop_origin = ((x - px) as f32, (y - py) as f32);
            self.backdrop_size = (pw as f32, ph as f32);

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
            //
            // 这里改的 GL 状态必须复原。Skia 的 GrDirectContext 自己缓存了一份
            // GL 状态的影子，而我们拿不到它的 reset_context()；femtovg 同理。
            // 不复原的话下一帧的渲染会以「状态还是我离开时那样」为前提发命令，
            // 症状是画面错乱而不是报错，很难往这边想。
            let prev_fbo = gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING);
            let prev_program = gl.get_parameter_i32(glow::CURRENT_PROGRAM);
            let prev_vao = gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING);
            let prev_active = gl.get_parameter_i32(glow::ACTIVE_TEXTURE);
            let prev_tex = gl.get_parameter_i32(glow::TEXTURE_BINDING_2D);
            let prev_blend = gl.is_enabled(glow::BLEND);
            let prev_depth = gl.is_enabled(glow::DEPTH_TEST);
            let prev_scissor = gl.is_enabled(glow::SCISSOR_TEST);
            let prev_cull = gl.is_enabled(glow::CULL_FACE);
            let prev_blend_src_rgb = gl.get_parameter_i32(glow::BLEND_SRC_RGB);
            let prev_blend_dst_rgb = gl.get_parameter_i32(glow::BLEND_DST_RGB);
            let prev_blend_src_alpha = gl.get_parameter_i32(glow::BLEND_SRC_ALPHA);
            let prev_blend_dst_alpha = gl.get_parameter_i32(glow::BLEND_DST_ALPHA);

            gl.bind_vertex_array(Some(self.vao));
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::CULL_FACE);
            // 模糊那两趟是覆盖写，不能开混合；下面画玻璃之前再开。
            gl.disable(glow::BLEND);

            // ---- P0 的「实时背景模糊」：在真实背板上做，不是在合成背板上 ----
            //
            // 这一步让自绘这条**同时**有模糊和折射。Flutter 版做不到：那边
            // ImageFilter.shader 与 ImageFilter.blur 是二选一（见
            // floating_navigation_bar.dart 的 supportsShader 分支）。
            let sigma_px = self.material.blur_radius * scale;
            let body = self.blur(pw, ph, sigma_px);

            gl.bind_framebuffer(glow::FRAMEBUFFER, framebuffer_of(prev_fbo));
            gl.use_program(Some(self.program));
            gl.enable(glow::BLEND);
            // 形状由 shader 的 coverage 给，所以是标准的 src-alpha 混合。
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            // viewport 收到玻璃矩形上，全屏三角形就只覆盖这一块；shader 里
            // 仍然用 gl_FragCoord（窗口坐标）减去 origin。
            gl.viewport(x, y, w, h);

            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(body));

            let m = self.material;
            self.set_uniforms(&rect, scale, x, y, &m);
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
                    "PROBE GL 材质 {} 模糊σ={}（物理 {sigma_px}px，降采样 1/{DOWNSCALE}）折射={} 带宽={} glint={} 带宽={} Fresnel={}^{} 圆角={}",
                    self.material_name,
                    m.blur_radius,
                    m.refraction_strength,
                    m.edge_highlight,
                    m.specular_strength,
                    m.specular_width,
                    m.fresnel_strength,
                    m.fresnel_power,
                    rect.h / 2.0
                );
                self.reported = true;
            }

            // ---- 复原 ----
            gl.bind_texture(glow::TEXTURE_2D, texture_of(prev_tex));
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

    /// 材质参数进 uniform。值全部来自 `GlassMaterial`，不在这里另写一套常数
    /// ——那张表是单一来源，shader 和宿主都只是它的消费者（AGENTS.md §2）。
    ///
    /// # Safety
    /// 调用方保证 GL 上下文 current 且 `self.program` 已 use。
    unsafe fn set_uniforms(
        &self,
        rect: &LogicalRect,
        scale: f32,
        x: i32,
        y: i32,
        m: &GlassMaterial,
    ) {
        let gl = &self.gl;
        unsafe {
            let loc = |n: &str| gl.get_uniform_location(self.program, n);
            gl.uniform_2_f32(loc("u_size").as_ref(), rect.w, rect.h);
            gl.uniform_2_f32(loc("u_origin").as_ref(), x as f32, y as f32);
            gl.uniform_1_f32(loc("u_scale").as_ref(), scale);
            // 胶囊：圆角等于高度的一半。
            gl.uniform_1_f32(loc("u_corner_radius").as_ref(), rect.h / 2.0);
            gl.uniform_1_f32(loc("u_rim_width").as_ref(), m.edge_highlight);
            gl.uniform_1_f32(loc("u_refraction").as_ref(), m.refraction_strength);
            gl.uniform_1_f32(loc("u_specular_strength").as_ref(), m.specular_strength);
            gl.uniform_1_f32(loc("u_specular_width").as_ref(), m.specular_width);
            // 半透明材质层（Flutter 的 DecoratedBox 那一半）。
            gl.uniform_3_f32(
                loc("u_surface_alpha").as_ref(),
                m.surface_alpha[0],
                m.surface_alpha[1],
                m.surface_alpha[2],
            );
            gl.uniform_1_f32(loc("u_border_alpha").as_ref(), m.border_alpha);
            gl.uniform_1_f32(loc("u_border_width").as_ref(), m.border_width);
            gl.uniform_1_f32(loc("u_fresnel_power").as_ref(), m.fresnel_power);
            gl.uniform_1_f32(loc("u_fresnel_strength").as_ref(), m.fresnel_strength);
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
            for tex in self.blur_tex {
                self.gl.delete_texture(tex);
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
