//! GPU 侧：源帧环的纹理、亮度金字塔、运动场、输出合成，以及 GPU 计时。
//!
//! 我们的 GL 调用夹在 mpv 和宿主 UI（Slint 的 femtovg）之间，所以进出都
//! 保存 / 恢复会被改动的 GL 状态，免得宿主拿到一个被换掉的 VAO 或 program。
//!
//! 所有方法都要求创建它的那个 GL 上下文是 current 的——它们只会在宿主的
//! 渲染回调里被调用。

use std::collections::VecDeque;
use std::ffi::{CStr, c_void};

use glow::HasContext;

use crate::sched;
use crate::shaders::{self, BEST_SCALE, BS, DOWN};

/// GPU 侧的错误。都是初始化或分配期的问题（缺扩展、FBO 不完整、着色器
/// 编不过），宿主遇到就该退回直通，不该重试。
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct GpuError(String);

type Result<T, E = GpuError> = std::result::Result<T, E>;

fn err(message: impl Into<String>) -> GpuError {
    GpuError(message.into())
}

impl From<&str> for GpuError {
    fn from(message: &str) -> Self {
        err(message)
    }
}

/// 输出模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 不走环：mpv 直接画进 Slint 的 FBO（现有路径，也用于 mpv 自带的
    /// `interpolation` 对照）。
    Direct,
    /// 走环、按 target_time 自己排程，但不插帧——用来把「换了排程方式」
    /// 与「插帧本身」的影响分开。
    Hold,
    /// 线性混合。
    Blend,
    /// 运动补偿。
    Mc,
}

impl Mode {
    fn shader_mode(self) -> i32 {
        match self {
            Self::Direct | Self::Hold => 0,
            Self::Blend => 1,
            Self::Mc => 2,
        }
    }
}

/// 可调参数。
#[derive(Debug, Clone, Copy)]
pub struct Params {
    /// 整像素搜索半径（低分辨率像素，半向量）。总位移上限 = 2·R·DOWN 源像素。
    pub radius: i32,
    /// 运动向量长度惩罚，偏向零运动 / 小运动。
    pub lambda: f32,
    /// 全帧平均失配超过它就当转场。
    pub cut_thresh: f32,
    pub occ_lo: f32,
    pub occ_hi: f32,
    /// 块的零运动平均差超过它，就算这个块「变了」。
    pub dup_block: f32,
    /// 变化块占比低于它就当重复帧。
    pub dup_thresh: f32,
    pub debug_field: bool,
    /// SAD 取样步长：1 = 块内全部 64 点，2 = 棋盘格 16 点。
    pub step: i32,
    /// 亚像素细化层数：0 = 不细化，1 = 半像素，2 = 四分之一像素。
    pub subpel: i32,
    /// 失配区域的回退：0 = 最近源帧，1 = 线性混合。
    pub fallback: i32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            radius: 6,
            lambda: 0.002,
            cut_thresh: 0.12,
            occ_lo: 0.25,
            occ_hi: 0.5,
            dup_block: 0.01,
            dup_thresh: 0.0015,
            debug_field: false,
            step: 2,
            subpel: 2,
            fallback: 1,
        }
    }
}

/// GPU 计时的分项。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stage {
    /// 亮度降采样。
    Luma,
    /// 块匹配 + 中值 + 全帧统计（每对源帧一次），下面四项之和。
    Motion,
    /// 其中：（块 × 候选）代价表。
    MeCost,
    /// 其中：每块取最小 + 亚像素细化。
    MeArgmin,
    /// 其中：中值。
    MeSmooth,
    /// 其中：全帧统计。
    MeReduce,
    /// 输出合成（每个 vsync 一次）。
    Output,
}

struct Slot {
    tex: glow::Texture,
    fbo: glow::Framebuffer,
    luma: glow::Texture,
    luma_fbo: glow::Framebuffer,
}

struct Field {
    /// (块 × 候选) 的代价表，尺寸随 R 变，懒分配。
    cost: Option<(glow::Texture, glow::Framebuffer, i32)>,
    raw: glow::Texture,
    raw_fbo: glow::Framebuffer,
    tex: glow::Texture,
    fbo: glow::Framebuffer,
    /// 1×1 的全帧统计（失配均值、变化块占比），输出 pass 读它判转场，
    /// 也异步回读给 CPU 判重复帧。四个轮流用，免得回读还没发出去就被下一对
    /// 覆盖。
    ///
    /// - 第一版用运动场的 mipmap 顶层当全帧平均——macOS（GL on Metal）上
    ///   `glGenerateMipmap` 对 RGBA16F 实测是点采样，顶层只是某一个块的值。
    /// - 第二版是 RGBA16F，运动估计一结束就 `glReadPixels` 进 PBO——Adreno 610
    ///   上这一下让 CPU 同步等 GPU，插帧段的 CPU 时间中位数 9.5 ms，输出掉到
    ///   51 fps；等计算的 fence 到了再发读也还有 p95 17 ms。现在不走
    ///   `glReadPixels`：transform feedback 把它写进 buffer，fence 到了再 map
    ///   （见 shaders::TF_VERTEX）。RGBA8 + 16 位定点编码是第二版留下的，保留。
    stats: [(glow::Texture, glow::Framebuffer); 4],
    /// 当前这一对的统计写在哪一个。
    stats_idx: usize,
    /// 当前内容对应的 (A.seq, B.seq)。
    pair: Option<(u64, u64)>,
}

/// 一次异步回读：transform feedback 写进 `buf`，fence 到了再 map。
struct Readback {
    pair: (u64, u64),
    fence: glow::Fence,
    buf: glow::Buffer,
}

/// 全帧统计，回读到 CPU 之后交给调用方。
#[derive(Debug, Clone, Copy)]
pub struct PairStats {
    pub pair: (u64, u64),
    /// 最优候选的平均失配（0..1）。
    pub best: f32,
    /// 零运动下「变了」的块的占比（0..1）。≈0 说明两帧相同。
    pub changed: f32,
}

struct Programs {
    luma: glow::Program,
    /// 代价表与 argmin 两个程序依赖 (R, STEP, SUBPEL)，参数变了就重编。
    cost: Option<((i32, i32, i32), glow::Program, glow::Program)>,
    smooth: glow::Program,
    reduce: glow::Program,
    tf: glow::Program,
    output: glow::Program,
}

pub struct Gpu {
    gl: glow::Context,
    pub es: bool,
    vao: glow::VertexArray,
    progs: Programs,
    slots: Vec<Slot>,
    capacity: usize,
    /// 源帧尺寸（mpv 画进槽的尺寸）。
    size: (i32, i32),
    lsize: (i32, i32),
    fsize: (i32, i32),
    field: Option<Field>,
    readbacks: VecDeque<Readback>,
    free_bufs: Vec<glow::Buffer>,
    readback_enabled: bool,
    timers: Timers,
}

struct Timers {
    enabled: bool,
    free: Vec<glow::Query>,
    pending: VecDeque<(glow::Query, Stage)>,
    done: Vec<(Stage, f32)>,
}

impl Timers {
    unsafe fn begin(&mut self, gl: &glow::Context, stage: Stage) -> Option<(glow::Query, Stage)> {
        if !self.enabled {
            return None;
        }
        let query = match self.free.pop() {
            Some(q) => q,
            None => unsafe { gl.create_query() }.ok()?,
        };
        unsafe { gl.begin_query(glow::TIME_ELAPSED, query) };
        Some((query, stage))
    }

    unsafe fn end(&mut self, gl: &glow::Context, token: Option<(glow::Query, Stage)>) {
        if let Some(token) = token {
            unsafe { gl.end_query(glow::TIME_ELAPSED) };
            self.pending.push_back(token);
        }
    }

    unsafe fn poll(&mut self, gl: &glow::Context) {
        while let Some(&(query, stage)) = self.pending.front() {
            let ready = unsafe { gl.get_query_parameter_u32(query, glow::QUERY_RESULT_AVAILABLE) };
            if ready == 0 {
                break;
            }
            let ns = unsafe { gl.get_query_parameter_u64(query, glow::QUERY_RESULT) };
            self.pending.pop_front();
            self.free.push(query);
            let ms = ns as f32 / 1.0e6;
            self.done.push((stage, ms));
            // 四段按固定顺序提交、按顺序完成，Reduce 是最后一段：到它时把
            // 这一组加起来记一条 Motion 总计。
            if stage == Stage::MeReduce {
                let n = self.done.len();
                if n >= 4 {
                    let total: f32 = self.done[n - 4..].iter().map(|(_, ms)| ms).sum();
                    self.done.push((Stage::Motion, total));
                }
            }
        }
    }
}

/// 进出我们这一段时要保存 / 恢复的 GL 状态。
struct SavedState {
    draw_fbo: Option<glow::Framebuffer>,
    read_fbo: Option<glow::Framebuffer>,
    viewport: [i32; 4],
    program: Option<glow::Program>,
    vao: Option<glow::VertexArray>,
    active_texture: i32,
    textures: [Option<glow::Texture>; 4],
    blend: bool,
    scissor: bool,
    depth: bool,
    stencil: bool,
    cull: bool,
    pack_buffer: Option<glow::Buffer>,
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gpu")
            .field("es", &self.es)
            .field("size", &self.size)
            .field("field_size", &self.fsize)
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl Gpu {
    /// # Safety
    /// GL 上下文必须是 current 的，`loader` 能解析当前上下文的函数。
    pub unsafe fn new(loader: &dyn Fn(&CStr) -> *const c_void, capacity: usize) -> Result<Self> {
        let gl = unsafe { glow::Context::from_loader_function_cstr(|name| loader(name)) };
        let version = gl.version();
        let es = version.is_embedded;
        let extensions = gl.supported_extensions();
        let timers_available = if es {
            extensions.contains("GL_EXT_disjoint_timer_query")
        } else {
            (version.major, version.minor) >= (3, 3) || extensions.contains("GL_ARB_timer_query")
        };
        if es
            && !extensions.contains("GL_EXT_color_buffer_half_float")
            && !extensions.contains("GL_EXT_color_buffer_float")
        {
            return Err(err(
                "GLES 上缺 EXT_color_buffer_half_float，运动场的 RGBA16F 画不进去",
            ));
        }

        let header = shaders::header(es);
        let build = |name: &str, fragment: &str| -> Result<glow::Program> {
            unsafe { compile(&gl, header, shaders::VERTEX, fragment) }
                .map_err(|e| err(format!("{name} 着色器：{e}")))
        };
        let scale = format!("#define BEST_SCALE {BEST_SCALE:.1}\n");
        let progs = Programs {
            luma: build("luma", shaders::LUMA)?,
            cost: None,
            smooth: build("smooth", shaders::SMOOTH)?,
            reduce: build("reduce", &format!("{scale}{}", shaders::REDUCE))?,
            tf: unsafe { compile_tf(&gl, header) }.map_err(|e| err(format!("tf 着色器：{e}")))?,
            output: build(
                "output",
                &format!(
                    "{scale}{}",
                    shaders::OUTPUT.replace("DECODE\n", shaders::DECODE_BEST)
                ),
            )?,
        };
        let vao = unsafe { gl.create_vertex_array() }.map_err(err)?;

        Ok(Self {
            gl,
            es,
            vao,
            progs,
            slots: Vec::new(),
            capacity,
            size: (0, 0),
            lsize: (0, 0),
            fsize: (0, 0),
            field: None,
            readbacks: VecDeque::new(),
            free_bufs: Vec::new(),
            readback_enabled: true,
            timers: Timers {
                enabled: timers_available,
                free: Vec::new(),
                pending: VecDeque::new(),
                done: Vec::new(),
            },
        })
    }

    #[must_use]
    pub fn renderer(&self) -> String {
        unsafe {
            format!(
                "{} / {}",
                self.gl.get_parameter_string(glow::RENDERER),
                self.gl.get_parameter_string(glow::VERSION)
            )
        }
    }

    #[must_use]
    pub fn field_size(&self) -> (i32, i32) {
        self.fsize
    }

    /// 按源帧尺寸（重新）分配环与运动场。尺寸没变就什么都不做。
    /// 返回 true 表示重新分配了（调用方要清空调度环）。
    pub fn ensure_size(&mut self, width: i32, height: i32) -> Result<bool> {
        if self.size == (width, height) && !self.slots.is_empty() {
            return Ok(false);
        }
        unsafe { self.free_targets() };
        let gl = &self.gl;
        let lw = (width + DOWN - 1) / DOWN;
        let lh = (height + DOWN - 1) / DOWN;
        let fw = (lw + BS - 1) / BS;
        let fh = (lh + BS - 1) / BS;
        unsafe {
            for _ in 0..self.capacity {
                let (tex, fbo) = target(
                    gl,
                    width,
                    height,
                    glow::RGBA8,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::LINEAR,
                )?;
                let (luma, luma_fbo) = target(
                    gl,
                    lw,
                    lh,
                    glow::R8,
                    glow::RED,
                    glow::UNSIGNED_BYTE,
                    glow::LINEAR,
                )?;
                self.slots.push(Slot {
                    tex,
                    fbo,
                    luma,
                    luma_fbo,
                });
            }
            let (raw, raw_fbo) = target(
                gl,
                fw,
                fh,
                glow::RGBA16F,
                glow::RGBA,
                glow::HALF_FLOAT,
                glow::NEAREST,
            )?;
            let (tex, fbo) = target(
                gl,
                fw,
                fh,
                glow::RGBA16F,
                glow::RGBA,
                glow::HALF_FLOAT,
                glow::LINEAR,
            )?;
            let stats = [
                target(
                    gl,
                    1,
                    1,
                    glow::RGBA8,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::NEAREST,
                )?,
                target(
                    gl,
                    1,
                    1,
                    glow::RGBA8,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::NEAREST,
                )?,
                target(
                    gl,
                    1,
                    1,
                    glow::RGBA8,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::NEAREST,
                )?,
                target(
                    gl,
                    1,
                    1,
                    glow::RGBA8,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::NEAREST,
                )?,
            ];
            self.field = Some(Field {
                cost: None,
                raw,
                raw_fbo,
                tex,
                fbo,
                stats,
                stats_idx: 0,
                pair: None,
            });
        }
        self.size = (width, height);
        self.lsize = (lw, lh);
        self.fsize = (fw, fh);
        Ok(true)
    }

    unsafe fn free_targets(&mut self) {
        let gl = &self.gl;
        unsafe {
            for slot in self.slots.drain(..) {
                gl.delete_framebuffer(slot.fbo);
                gl.delete_texture(slot.tex);
                gl.delete_framebuffer(slot.luma_fbo);
                gl.delete_texture(slot.luma);
            }
            if let Some(field) = self.field.take() {
                if let Some((tex, fbo, _)) = field.cost {
                    gl.delete_framebuffer(fbo);
                    gl.delete_texture(tex);
                }
                gl.delete_framebuffer(field.raw_fbo);
                gl.delete_texture(field.raw);
                gl.delete_framebuffer(field.fbo);
                for (tex, fbo) in field.stats {
                    gl.delete_framebuffer(fbo);
                    gl.delete_texture(tex);
                }
                gl.delete_texture(field.tex);
            }
        }
    }

    /// 槽的 FBO 名字，交给 mpv 的 `MPV_RENDER_PARAM_OPENGL_FBO`。
    #[must_use]
    pub fn slot_fbo(&self, slot: usize) -> i32 {
        self.slots[slot].fbo.0.get() as i32
    }

    /// 实验室模式（与 60 fps 真值比画质）：直接往槽里上传一张 RGBA8 图
    /// （行序按 GL，从下往上）。
    pub fn upload(&mut self, slot: usize, rgba: &[u8]) {
        let (w, h) = self.size;
        unsafe {
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.slots[slot].tex));
            self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                w,
                h,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(rgba)),
            );
            self.gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    /// 槽里的新源帧写好之后调用：生成它的低分辨率亮度。
    pub fn prepare_slot(&mut self, slot: usize) {
        let gl = &self.gl;
        let saved = unsafe { save(gl) };
        let token = unsafe { self.timers.begin(gl, Stage::Luma) };
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            disable_fixed(gl);
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.slots[slot].luma_fbo));
            gl.viewport(0, 0, self.lsize.0, self.lsize.1);
            gl.use_program(Some(self.progs.luma));
            bind_tex(gl, 0, self.slots[slot].tex);
            set_i(gl, self.progs.luma, "src", 0);
            set_2f(
                gl,
                self.progs.luma,
                "texel",
                1.0 / self.size.0 as f32,
                1.0 / self.size.1 as f32,
            );
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
        unsafe { self.timers.end(gl, token) };
        unsafe { restore(gl, &saved) };
    }

    /// 保证运动场对应 (a, b) 这一对。已经是就直接返回（同一对的多张输出复用）。
    pub fn ensure_field(&mut self, a: &sched::SrcFrame, b: &sched::SrcFrame, params: &Params) {
        let pair = (a.seq, b.seq);
        let Some(field) = self.field.as_mut() else {
            return;
        };
        if field.pair == Some(pair) {
            return;
        }
        field.pair = Some(pair);
        let gl = &self.gl;
        let saved = unsafe { save(gl) };
        let d = 2 * params.radius + 1;
        if field.cost.as_ref().is_none_or(|c| c.2 != params.radius) {
            unsafe {
                if let Some((tex, fbo, _)) = field.cost.take() {
                    gl.delete_framebuffer(fbo);
                    gl.delete_texture(tex);
                }
                let made = target(
                    gl,
                    self.fsize.0 * d,
                    self.fsize.1 * d,
                    glow::RGBA16F,
                    glow::RGBA,
                    glow::HALF_FLOAT,
                    glow::NEAREST,
                );
                restore(gl, &saved);
                match made {
                    Ok((tex, fbo)) => field.cost = Some((tex, fbo, params.radius)),
                    Err(_) => return,
                }
            }
        }
        let Some((cost_tex, cost_fbo, _)) = field.cost else {
            unsafe { restore(gl, &saved) };
            return;
        };
        let key = (params.radius, params.step, params.subpel);
        if self.progs.cost.as_ref().is_none_or(|c| c.0 != key) {
            unsafe {
                if let Some((_, cost, argmin)) = self.progs.cost.take() {
                    gl.delete_program(cost);
                    gl.delete_program(argmin);
                }
                let defs = format!(
                    "{}{}",
                    shaders::header(self.es),
                    shaders::defines(key.0, key.1, key.2)
                );
                match (
                    compile(gl, &defs, shaders::VERTEX, shaders::COST),
                    compile(gl, &defs, shaders::VERTEX, shaders::ARGMIN),
                ) {
                    (Ok(cost), Ok(argmin)) => self.progs.cost = Some((key, cost, argmin)),
                    (Err(e), _) | (_, Err(e)) => {
                        tracing::error!("运动估计着色器编不过：{e}");
                        restore(gl, &saved);
                        return;
                    }
                }
            }
        }
        let Some((_, cost_prog, argmin_prog)) = self.progs.cost else {
            unsafe { restore(gl, &saved) };
            return;
        };
        // TIME_ELAPSED 不能嵌套，所以不套总计时，分四段各计一次；总和由调用方加。
        let timers = &mut self.timers;
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            disable_fixed(gl);

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(cost_fbo));
            gl.viewport(0, 0, self.fsize.0 * d, self.fsize.1 * d);
            let prog = cost_prog;
            gl.use_program(Some(prog));
            bind_tex(gl, 0, self.slots[a.slot].luma);
            bind_tex(gl, 1, self.slots[b.slot].luma);
            set_i(gl, prog, "A", 0);
            set_i(gl, prog, "B", 1);
            set_2i(gl, prog, "lsize", self.lsize.0, self.lsize.1);
            set_f(gl, prog, "lambda", params.lambda);
            let t = timers.begin(gl, Stage::MeCost);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            timers.end(gl, t);

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(field.raw_fbo));
            gl.viewport(0, 0, self.fsize.0, self.fsize.1);
            let prog = argmin_prog;
            gl.use_program(Some(prog));
            bind_tex(gl, 2, cost_tex);
            set_i(gl, prog, "A", 0);
            set_i(gl, prog, "B", 1);
            set_i(gl, prog, "C", 2);
            set_2i(gl, prog, "lsize", self.lsize.0, self.lsize.1);
            set_f(gl, prog, "lambda", params.lambda);
            let t = timers.begin(gl, Stage::MeArgmin);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            timers.end(gl, t);

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(field.fbo));
            let prog = self.progs.smooth;
            gl.use_program(Some(prog));
            bind_tex(gl, 0, field.raw);
            set_i(gl, prog, "F", 0);
            set_2i(gl, prog, "fsize", self.fsize.0, self.fsize.1);
            set_f(gl, prog, "dupBlock", params.dup_block);
            let t = timers.begin(gl, Stage::MeSmooth);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            timers.end(gl, t);

            field.stats_idx = (field.stats_idx + 1) % field.stats.len();
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(field.stats[field.stats_idx].1));
            gl.viewport(0, 0, 1, 1);
            let prog = self.progs.reduce;
            gl.use_program(Some(prog));
            bind_tex(gl, 0, field.tex);
            set_i(gl, prog, "F", 0);
            set_2i(gl, prog, "fsize", self.fsize.0, self.fsize.1);
            let t = timers.begin(gl, Stage::MeReduce);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            timers.end(gl, t);
        }

        // 全帧平均异步回读：PBO + fence，下几帧再取，不让 CPU 等 GPU。
        if !self.readback_enabled {
            unsafe { restore(gl, &saved) };
            return;
        }
        // transform feedback 把统计写进 buffer（见 shaders::TF_VERTEX 的说明）。
        unsafe {
            let buf = match self.free_bufs.pop() {
                Some(buf) => Some(buf),
                None => gl.create_buffer().ok().inspect(|buf| {
                    gl.bind_buffer(glow::TRANSFORM_FEEDBACK_BUFFER, Some(*buf));
                    gl.buffer_data_size(glow::TRANSFORM_FEEDBACK_BUFFER, 16, glow::STREAM_READ);
                    gl.bind_buffer(glow::TRANSFORM_FEEDBACK_BUFFER, None);
                }),
            };
            if let Some(buf) = buf {
                gl.use_program(Some(self.progs.tf));
                bind_tex(gl, 0, field.stats[field.stats_idx].0);
                set_i(gl, self.progs.tf, "S", 0);
                gl.enable(glow::RASTERIZER_DISCARD);
                gl.bind_buffer_base(glow::TRANSFORM_FEEDBACK_BUFFER, 0, Some(buf));
                gl.begin_transform_feedback(glow::POINTS);
                gl.draw_arrays(glow::POINTS, 0, 1);
                gl.end_transform_feedback();
                gl.bind_buffer_base(glow::TRANSFORM_FEEDBACK_BUFFER, 0, None);
                gl.disable(glow::RASTERIZER_DISCARD);
                match gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0) {
                    Ok(fence) => self.readbacks.push_back(Readback { pair, fence, buf }),
                    Err(_) => self.free_bufs.push(buf),
                }
            }
        }
        unsafe { restore(gl, &saved) };
    }

    /// 输出：画进 `fbo`（Slint 的，或实验室的离屏目标），视口为 letterbox。
    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &mut self,
        fbo: Option<glow::Framebuffer>,
        target: (i32, i32),
        a: &sched::SrcFrame,
        b: &sched::SrcFrame,
        p: f32,
        mode: Mode,
        params: &Params,
    ) {
        let gl = &self.gl;
        let saved = unsafe { save(gl) };
        let token = unsafe { self.timers.begin(gl, Stage::Output) };
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            disable_fixed(gl);
            gl.bind_framebuffer(glow::FRAMEBUFFER, fbo);
            // mpv 原来那一下全屏绘制兼做了清屏（见 MpvRender::has_new_frame 的说明），
            // 现在由我们来清。
            gl.viewport(0, 0, target.0, target.1);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            let (x, y, w, h) = letterbox(self.size, target);
            gl.viewport(x, y, w, h);

            let prog = self.progs.output;
            gl.use_program(Some(prog));
            bind_tex(gl, 0, self.slots[a.slot].tex);
            bind_tex(gl, 1, self.slots[b.slot].tex);
            set_i(gl, prog, "FA", 0);
            set_i(gl, prog, "FB", 1);
            if let Some(field) = &self.field {
                bind_tex(gl, 2, field.tex);
                set_i(gl, prog, "F", 2);
                bind_tex(gl, 3, field.stats[field.stats_idx].0);
                set_i(gl, prog, "S", 3);
            }
            set_f(gl, prog, "p", p);
            set_i(gl, prog, "mode", mode.shader_mode());
            set_2f(gl, prog, "lsize", self.lsize.0 as f32, self.lsize.1 as f32);
            set_f(gl, prog, "cutThresh", params.cut_thresh);
            set_f(gl, prog, "occLo", params.occ_lo);
            set_f(gl, prog, "occHi", params.occ_hi);
            set_i(gl, prog, "debug", i32::from(params.debug_field));
            set_i(gl, prog, "fallback", params.fallback);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
        unsafe { self.timers.end(gl, token) };
        unsafe { restore(gl, &saved) };
    }

    /// 收已完成的 GPU 计时与回读。每个 vsync 调一次。
    pub fn poll(&mut self) -> (Vec<(Stage, f32)>, Vec<PairStats>) {
        let gl = &self.gl;
        unsafe { self.timers.poll(gl) };
        let mut stats = Vec::new();
        while let Some(front) = self.readbacks.front() {
            let status = unsafe { gl.client_wait_sync(front.fence, 0, 0) };
            if status != glow::ALREADY_SIGNALED && status != glow::CONDITION_SATISFIED {
                break;
            }
            let Some(rb) = self.readbacks.pop_front() else {
                break;
            };
            let mut bytes = [0u8; 16];
            unsafe {
                gl.delete_sync(rb.fence);
                let prev = gl.get_parameter_buffer(glow::TRANSFORM_FEEDBACK_BUFFER_BINDING);
                gl.bind_buffer(glow::TRANSFORM_FEEDBACK_BUFFER, Some(rb.buf));
                // GLES 没有 glGetBufferSubData，统一走 map（GL 3.0+ / GLES 3.0 都有）。
                let ptr =
                    gl.map_buffer_range(glow::TRANSFORM_FEEDBACK_BUFFER, 0, 16, glow::MAP_READ_BIT);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr(), 16);
                    gl.unmap_buffer(glow::TRANSFORM_FEEDBACK_BUFFER);
                }
                gl.bind_buffer(glow::TRANSFORM_FEEDBACK_BUFFER, prev);
            }
            self.free_bufs.push(rb.buf);
            // vec4 float，内容是 REDUCE 的 RGBA8 编码：(changed_hi, changed_lo, best_hi, best_lo) / 255。
            let f = |i: usize| {
                f32::from_ne_bytes([
                    bytes[i * 4],
                    bytes[i * 4 + 1],
                    bytes[i * 4 + 2],
                    bytes[i * 4 + 3],
                ])
            };
            let u16_of =
                |hi: f32, lo: f32| ((hi * 255.0).round() * 256.0 + (lo * 255.0).round()) / 65535.0;
            stats.push(PairStats {
                pair: rb.pair,
                changed: u16_of(f(0), f(1)),
                best: u16_of(f(2), f(3)) / BEST_SCALE,
            });
        }
        (std::mem::take(&mut self.timers.done), stats)
    }

    /// 调试：槽的低分辨率亮度之和（**同步回读**，只在排查时用：Adreno 上
    /// `glReadPixels` 会让 CPU 等整条命令流，见 #218 §5）。
    pub fn luma_checksum(&self, slot: usize) -> u64 {
        let (w, h) = self.lsize;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        unsafe {
            let prev = self
                .gl
                .get_parameter_framebuffer(glow::READ_FRAMEBUFFER_BINDING);
            self.gl
                .bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.slots[slot].luma_fbo));
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            self.gl.read_pixels(
                0,
                0,
                w,
                h,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut buf)),
            );
            self.gl.bind_framebuffer(glow::READ_FRAMEBUFFER, prev);
        }
        buf.as_chunks::<4>()
            .0
            .iter()
            .map(|px| u64::from(px[0]))
            .sum()
    }

    /// 实验室模式：读回离屏目标（RGBA8，行序从下往上）。
    pub fn read_rgba(&self, fbo: glow::Framebuffer, w: i32, h: i32) -> Vec<u8> {
        let mut out = vec![0u8; (w * h * 4) as usize];
        unsafe {
            self.gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fbo));
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            self.gl.read_pixels(
                0,
                0,
                w,
                h,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut out)),
            );
            self.gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        }
        out
    }

    /// 实验室模式：建一个离屏 RGBA8 目标。
    pub fn offscreen(&self, w: i32, h: i32) -> Result<glow::Framebuffer> {
        unsafe {
            target(
                &self.gl,
                w,
                h,
                glow::RGBA8,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::LINEAR,
            )
        }
        .map(|(_, fbo)| fbo)
    }

    /// 等 GPU 把已提交的都做完（实验室模式计时用）。
    pub fn finish(&self) {
        unsafe { self.gl.finish() };
    }

    /// 包一层 glow 的 FBO 句柄。0 表示默认帧缓冲。
    #[must_use]
    pub fn framebuffer(name: i32) -> Option<glow::Framebuffer> {
        u32::try_from(name)
            .ok()
            .and_then(std::num::NonZeroU32::new)
            .map(glow::NativeFramebuffer)
    }

    pub fn disable_readback(&mut self) {
        self.readback_enabled = false;
    }

    pub fn disable_timers(&mut self) {
        self.timers.enabled = false;
    }

    pub fn timers_enabled(&self) -> bool {
        self.timers.enabled
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        // RenderingTeardown 时 GL 上下文还在，可以正常释放。
        unsafe {
            self.free_targets();
            let gl = &self.gl;
            for rb in self.readbacks.drain(..) {
                gl.delete_sync(rb.fence);
                gl.delete_buffer(rb.buf);
            }
            for pbo in self.free_bufs.drain(..) {
                gl.delete_buffer(pbo);
            }
            for q in self.timers.free.drain(..) {
                gl.delete_query(q);
            }
            gl.delete_vertex_array(self.vao);
            gl.delete_program(self.progs.luma);
            if let Some((_, cost, argmin)) = self.progs.cost.take() {
                gl.delete_program(cost);
                gl.delete_program(argmin);
            }
            gl.delete_program(self.progs.smooth);
            gl.delete_program(self.progs.reduce);
            gl.delete_program(self.progs.tf);
            gl.delete_program(self.progs.output);
        }
    }
}

/// 视频按比例放进目标，居中。
#[must_use]
pub fn letterbox(video: (i32, i32), target: (i32, i32)) -> (i32, i32, i32, i32) {
    let (vw, vh) = (video.0.max(1) as f32, video.1.max(1) as f32);
    let (tw, th) = (target.0 as f32, target.1 as f32);
    let scale = (tw / vw).min(th / vh);
    let (w, h) = ((vw * scale).round() as i32, (vh * scale).round() as i32);
    ((target.0 - w) / 2, (target.1 - h) / 2, w, h)
}

unsafe fn compile(
    gl: &glow::Context,
    header: &str,
    vs: &str,
    fs: &str,
) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(shader, &format!("{header}{src}"));
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                return Err(gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        Ok(program)
    }
}

/// transform feedback 用的程序：varyings 要在链接之前声明。
unsafe fn compile_tf(gl: &glow::Context, header: &str) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, src) in [
            (glow::VERTEX_SHADER, shaders::TF_VERTEX),
            (glow::FRAGMENT_SHADER, shaders::TF_FRAGMENT),
        ] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(shader, &format!("{header}{src}"));
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                return Err(gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.transform_feedback_varyings(program, &["v"], glow::INTERLEAVED_ATTRIBS);
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        Ok(program)
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn target(
    gl: &glow::Context,
    w: i32,
    h: i32,
    internal: u32,
    format: u32,
    ty: u32,
    filter: u32,
) -> Result<(glow::Texture, glow::Framebuffer)> {
    unsafe {
        let tex = gl.create_texture().map_err(err)?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            internal as i32,
            w,
            h,
            0,
            format,
            ty,
            glow::PixelUnpackData::Slice(None),
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter as i32);
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.bind_texture(glow::TEXTURE_2D, None);
        let fbo = gl.create_framebuffer().map_err(err)?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(tex),
            0,
        );
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        if status != glow::FRAMEBUFFER_COMPLETE {
            return Err(err(format!(
                "FBO 不完整（{w}×{h}，内部格式 0x{internal:X}）：0x{status:X}"
            )));
        }
        Ok((tex, fbo))
    }
}

unsafe fn save(gl: &glow::Context) -> SavedState {
    unsafe {
        let mut viewport = [0; 4];
        gl.get_parameter_i32_slice(glow::VIEWPORT, &mut viewport);
        let active_texture = gl.get_parameter_i32(glow::ACTIVE_TEXTURE);
        let mut textures = [None; 4];
        for (i, slot) in textures.iter_mut().enumerate() {
            gl.active_texture(glow::TEXTURE0 + i as u32);
            *slot = gl.get_parameter_texture(glow::TEXTURE_BINDING_2D);
        }
        gl.active_texture(active_texture as u32);
        SavedState {
            draw_fbo: gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING),
            read_fbo: gl.get_parameter_framebuffer(glow::READ_FRAMEBUFFER_BINDING),
            viewport,
            program: gl.get_parameter_program(glow::CURRENT_PROGRAM),
            vao: gl.get_parameter_vertex_array(glow::VERTEX_ARRAY_BINDING),
            active_texture,
            textures,
            blend: gl.is_enabled(glow::BLEND),
            scissor: gl.is_enabled(glow::SCISSOR_TEST),
            depth: gl.is_enabled(glow::DEPTH_TEST),
            stencil: gl.is_enabled(glow::STENCIL_TEST),
            cull: gl.is_enabled(glow::CULL_FACE),
            pack_buffer: gl.get_parameter_buffer(glow::PIXEL_PACK_BUFFER_BINDING),
        }
    }
}

unsafe fn restore(gl: &glow::Context, s: &SavedState) {
    unsafe {
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, s.draw_fbo);
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, s.read_fbo);
        gl.viewport(s.viewport[0], s.viewport[1], s.viewport[2], s.viewport[3]);
        gl.use_program(s.program);
        gl.bind_vertex_array(s.vao);
        for (i, tex) in s.textures.iter().enumerate() {
            gl.active_texture(glow::TEXTURE0 + i as u32);
            gl.bind_texture(glow::TEXTURE_2D, *tex);
        }
        gl.active_texture(s.active_texture as u32);
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, s.pack_buffer);
        for (cap, on) in [
            (glow::BLEND, s.blend),
            (glow::SCISSOR_TEST, s.scissor),
            (glow::DEPTH_TEST, s.depth),
            (glow::STENCIL_TEST, s.stencil),
            (glow::CULL_FACE, s.cull),
        ] {
            if on { gl.enable(cap) } else { gl.disable(cap) }
        }
    }
}

unsafe fn disable_fixed(gl: &glow::Context) {
    unsafe {
        for cap in [
            glow::BLEND,
            glow::SCISSOR_TEST,
            glow::DEPTH_TEST,
            glow::STENCIL_TEST,
            glow::CULL_FACE,
        ] {
            gl.disable(cap);
        }
        gl.color_mask(true, true, true, true);
        // mpv 的渲染器会给纹理单元绑 sampler 对象；它们优先于纹理自身的过滤与
        // 环绕参数，不解绑的话我们的 texture() 采样行为由 mpv 决定。
        for unit in 0..4 {
            gl.bind_sampler(unit, None);
        }
    }
}

unsafe fn bind_tex(gl: &glow::Context, unit: u32, tex: glow::Texture) {
    unsafe {
        gl.active_texture(glow::TEXTURE0 + unit);
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    }
}

unsafe fn set_i(gl: &glow::Context, prog: glow::Program, name: &str, v: i32) {
    unsafe { gl.uniform_1_i32(gl.get_uniform_location(prog, name).as_ref(), v) };
}

unsafe fn set_f(gl: &glow::Context, prog: glow::Program, name: &str, v: f32) {
    unsafe { gl.uniform_1_f32(gl.get_uniform_location(prog, name).as_ref(), v) };
}

unsafe fn set_2f(gl: &glow::Context, prog: glow::Program, name: &str, x: f32, y: f32) {
    unsafe { gl.uniform_2_f32(gl.get_uniform_location(prog, name).as_ref(), x, y) };
}

unsafe fn set_2i(gl: &glow::Context, prog: glow::Program, name: &str, x: i32, y: i32) {
    unsafe { gl.uniform_2_i32(gl.get_uniform_location(prog, name).as_ref(), x, y) };
}
