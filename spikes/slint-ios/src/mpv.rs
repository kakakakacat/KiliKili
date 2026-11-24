//! libmpv 的最小绑定（iOS）。
//!
//! 和 `../slint-mpv` 的不同：那边运行期 dlopen，这边**构建期链接**——
//! iOS 上 App 自带的 framework 由 Xcode 嵌进包里、按 `@rpath` 加载，
//! staticlib 里留着未解析的 `mpv_*` 符号，最后由 Xcode 链 `Mpv.framework`
//! 补上。只声明本 spike 用得到的那几个函数。
//!
//! media_kit 带的这份 iOS 构建是 mpv 0.39，编进了 `gl` / `ios-gl`
//! （OpenGL ES + VideoToolbox→GLES 零拷贝）与 `audiounit`，**没有** Vulkan、
//! 没有 avfoundation 输出。所以渲染只有两条路：OpenGL ES，或软件渲染。
//! 两条都在这里：`new(.., RenderApi::Sw)` 是第 2 步的基线，
//! `RenderApi::OpenGl` 是零拷贝路线（画进 IOSurface 支撑的 FBO）。

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;

#[repr(C)]
struct MpvHandle {
    _private: [u8; 0],
}
#[repr(C)]
struct MpvRenderContext {
    _private: [u8; 0],
}

#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}

#[repr(C)]
pub struct MpvEvent {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

const RENDER_PARAM_INVALID: c_int = 0;
const RENDER_PARAM_API_TYPE: c_int = 1;
const RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
const RENDER_PARAM_OPENGL_FBO: c_int = 3;
const RENDER_PARAM_FLIP_Y: c_int = 4;
const RENDER_PARAM_NEXT_FRAME_INFO: c_int = 11;
const RENDER_PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;
const RENDER_PARAM_SW_SIZE: c_int = 17;
const RENDER_PARAM_SW_FORMAT: c_int = 18;
const RENDER_PARAM_SW_STRIDE: c_int = 19;
const RENDER_PARAM_SW_POINTER: c_int = 20;

pub const EVENT_NONE: c_int = 0;
pub const EVENT_END_FILE: c_int = 7;
pub const EVENT_FILE_LOADED: c_int = 8;
pub const EVENT_PLAYBACK_RESTART: c_int = 21;

unsafe extern "C" {
    fn mpv_create() -> *mut MpvHandle;
    fn mpv_initialize(ctx: *mut MpvHandle) -> c_int;
    fn mpv_terminate_destroy(ctx: *mut MpvHandle);
    fn mpv_set_option_string(
        ctx: *mut MpvHandle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    fn mpv_command(ctx: *mut MpvHandle, args: *mut *const c_char) -> c_int;
    fn mpv_set_property_async(
        ctx: *mut MpvHandle,
        reply_userdata: u64,
        name: *const c_char,
        format: c_int,
        data: *mut c_void,
    ) -> c_int;
    fn mpv_set_property_string(
        ctx: *mut MpvHandle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    fn mpv_get_property_string(ctx: *mut MpvHandle, name: *const c_char) -> *mut c_char;
    fn mpv_free(data: *mut c_void);
    fn mpv_wait_event(ctx: *mut MpvHandle, timeout: f64) -> *mut MpvEvent;
    fn mpv_error_string(error: c_int) -> *const c_char;

    fn mpv_render_context_create(
        res: *mut *mut MpvRenderContext,
        mpv: *mut MpvHandle,
        params: *mut RenderParam,
    ) -> c_int;
    fn mpv_render_context_update(ctx: *mut MpvRenderContext) -> u64;
    fn mpv_render_context_render(ctx: *mut MpvRenderContext, params: *mut RenderParam) -> c_int;
    fn mpv_render_context_free(ctx: *mut MpvRenderContext);
    fn mpv_render_context_get_info(ctx: *mut MpvRenderContext, param: RenderParam) -> c_int;
    fn mpv_get_time_us(ctx: *mut MpvHandle) -> i64;
}

/// `mpv_render_frame_info`（#218 插帧 spike 用）。
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct FrameInfo {
    pub flags: u64,
    /// mpv 0.42 实测是**纳秒**（render.h 注释说与 mpv_get_time_us 同为微秒）。
    pub target_time: i64,
}

pub const FRAME_INFO_REDRAW: u64 = 1 << 1;

fn check(code: c_int, what: &str) -> Result<(), String> {
    if code >= 0 {
        return Ok(());
    }
    // SAFETY: mpv_error_string 对任意错误码都返回静态字符串。
    let msg = unsafe { CStr::from_ptr(mpv_error_string(code)) }.to_string_lossy();
    Err(format!("{what}: {msg} ({code})"))
}

fn cstr(s: &str) -> CString {
    CString::new(s).expect("参数里不该有 NUL")
}

#[repr(C)]
struct OpenGlInitParams {
    get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
struct OpenGlFbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

pub enum RenderApi {
    Sw,
    /// 调用方保证此刻 EAGLContext 已是当前（ios-gl hwdec 互操作在创建时绑定它）。
    OpenGl(unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void),
}

pub struct Mpv {
    handle: *mut MpvHandle,
    render: *mut MpvRenderContext,
}

impl Mpv {
    /// `options` 在 `mpv_initialize` 之前设，例如 `("hwdec", "videotoolbox")`。
    pub fn new(options: &[(&str, &str)], api: RenderApi) -> Result<Self, String> {
        // SAFETY: 以下都是 libmpv 的公开 C API，按文档的调用顺序使用。
        unsafe {
            let handle = mpv_create();
            if handle.is_null() {
                return Err("mpv_create 返回 NULL".into());
            }
            for (name, value) in options {
                let (n, v) = (cstr(name), cstr(value));
                check(
                    mpv_set_option_string(handle, n.as_ptr(), v.as_ptr()),
                    &format!("设置 {name}"),
                )?;
            }
            check(mpv_initialize(handle), "mpv_initialize")?;

            let (name, mut gl) = match api {
                RenderApi::Sw => (cstr("sw"), None),
                RenderApi::OpenGl(get_proc_address) => (
                    cstr("opengl"),
                    Some(OpenGlInitParams {
                        get_proc_address,
                        get_proc_address_ctx: ptr::null_mut(),
                    }),
                ),
            };
            let mut params = vec![RenderParam {
                kind: RENDER_PARAM_API_TYPE,
                data: name.as_ptr() as *mut c_void,
            }];
            if let Some(gl) = gl.as_mut() {
                params.push(RenderParam {
                    kind: RENDER_PARAM_OPENGL_INIT_PARAMS,
                    data: (gl as *mut OpenGlInitParams).cast(),
                });
            }
            params.push(RenderParam {
                kind: RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            });
            let mut render = ptr::null_mut();
            check(
                mpv_render_context_create(&mut render, handle, params.as_mut_ptr()),
                &format!("mpv_render_context_create({})", name.to_string_lossy()),
            )?;
            Ok(Self { handle, render })
        }
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args.iter().map(|a| cstr(a)).collect();
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(ptr::null());
        // SAFETY: ptrs 以 NULL 结尾，生命周期覆盖调用。
        check(
            unsafe { mpv_command(self.handle, ptrs.as_mut_ptr()) },
            args.first().copied().unwrap_or("command"),
        )
    }

    pub fn property(&self, name: &str) -> Option<String> {
        let n = cstr(name);
        // SAFETY: 返回值要用 mpv_free 释放。
        unsafe {
            let raw = mpv_get_property_string(self.handle, n.as_ptr());
            if raw.is_null() {
                return None;
            }
            let value = CStr::from_ptr(raw).to_string_lossy().into_owned();
            mpv_free(raw.cast());
            Some(value)
        }
    }

    /// 非阻塞地取走所有待处理事件，返回事件 id 列表。
    pub fn drain_events(&self) -> Vec<c_int> {
        let mut ids = Vec::new();
        loop {
            // SAFETY: timeout 0 不阻塞；返回的指针在下次调用前有效，我们只读 id。
            let id = unsafe { (*mpv_wait_event(self.handle, 0.0)).event_id };
            if id == EVENT_NONE {
                return ids;
            }
            ids.push(id);
        }
    }

    /// 有新帧要画时返回 true（`MPV_RENDER_UPDATE_FRAME`）。
    pub fn wants_frame(&self) -> bool {
        // SAFETY: 渲染上下文在 self 存活期间有效。
        unsafe { mpv_render_context_update(self.render) & 1 != 0 }
    }

    /// 软件渲染一帧到 `buf`（RGBA，紧密排列）。
    pub fn render_sw(&self, width: u32, height: u32, buf: &mut [u8]) -> Result<(), String> {
        let mut size: [c_int; 2] = [width as c_int, height as c_int];
        let format = cstr("rgb0");
        let mut stride: usize = width as usize * 4;
        assert!(buf.len() >= stride * height as usize);
        let mut params = [
            RenderParam {
                kind: RENDER_PARAM_SW_SIZE,
                data: size.as_mut_ptr().cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_SW_FORMAT,
                data: format.as_ptr() as *mut c_void,
            },
            RenderParam {
                kind: RENDER_PARAM_SW_STRIDE,
                data: (&raw mut stride).cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_SW_POINTER,
                data: buf.as_mut_ptr().cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];
        // SAFETY: 所有参数指向的内存在调用期间有效。
        check(
            unsafe { mpv_render_context_render(self.render, params.as_mut_ptr()) },
            "mpv_render_context_render(sw)",
        )
    }
}

impl Mpv {
    /// 下一张要画的帧的信息；调用前先用 [`Mpv::wants_frame`] 确认有帧。
    pub fn next_frame_info(&self) -> FrameInfo {
        let mut info = FrameInfo::default();
        let param = RenderParam {
            kind: RENDER_PARAM_NEXT_FRAME_INFO,
            data: (&raw mut info).cast(),
        };
        // SAFETY: info 在调用期间有效。
        let _ = unsafe { mpv_render_context_get_info(self.render, param) };
        info
    }

    /// mpv 时钟（微秒）。
    pub fn time_us(&self) -> i64 {
        // SAFETY: handle 有效。
        unsafe { mpv_get_time_us(self.handle) }
    }

    /// 画一帧到 `fbo`（调用方保证 EAGLContext 是当前）。
    ///
    /// `BLOCK_FOR_TARGET_TIME = 0`：默认值 1 会让 render 一直阻塞到这一帧的
    /// 显示时刻，而我们是在 UI 线程上调它的——那等于把 UI 帧率锁死在视频帧率上。
    pub fn render_gl(&self, fbo: u32, width: u32, height: u32, flip_y: bool) -> Result<(), String> {
        let mut target = OpenGlFbo {
            fbo: fbo as c_int,
            w: width as c_int,
            h: height as c_int,
            internal_format: 0,
        };
        let mut flip: c_int = flip_y.into();
        let mut block: c_int = 0;
        let mut params = [
            RenderParam {
                kind: RENDER_PARAM_OPENGL_FBO,
                data: (&raw mut target).cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_FLIP_Y,
                data: (&raw mut flip).cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                data: (&raw mut block).cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];
        // SAFETY: 参数指向的内存在调用期间有效。
        check(
            unsafe { mpv_render_context_render(self.render, params.as_mut_ptr()) },
            "mpv_render_context_render(gl)",
        )
    }

    pub fn set_property(&self, name: &str, value: &str) -> Result<(), String> {
        let (n, v) = (cstr(name), cstr(value));
        // SAFETY: 两个 C 字符串在调用期间有效。
        check(
            unsafe { mpv_set_property_string(self.handle, n.as_ptr(), v.as_ptr()) },
            name,
        )
    }
}

impl Mpv {
    /// 异步写属性：#219 每次换档都要改 `audio-delay`，而这时正在渲染回调里
    /// （ADR-004 决策三：渲染线程不发同步的 mpv 命令）。
    pub fn set_property_async(&self, name: &str, value: &str) -> Result<(), String> {
        let (n, v) = (cstr(name), cstr(value));
        let mut ptr = v.as_ptr();
        // SAFETY: MPV_FORMAT_STRING（1）的 data 是 char**；异步版返回前就拷走了值。
        check(
            unsafe { mpv_set_property_async(self.handle, 0, n.as_ptr(), 1, (&raw mut ptr).cast()) },
            name,
        )
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: 先释放渲染上下文，再销毁核心——文档要求的顺序。
        unsafe {
            mpv_render_context_free(self.render);
            mpv_terminate_destroy(self.handle);
        }
    }
}
