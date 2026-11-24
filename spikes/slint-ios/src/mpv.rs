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
//! 本文件先做软件渲染（第 2 步的基线）。

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
    fn mpv_set_option_string(ctx: *mut MpvHandle, name: *const c_char, data: *const c_char)
    -> c_int;
    fn mpv_command(ctx: *mut MpvHandle, args: *mut *const c_char) -> c_int;
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
}

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

pub struct Mpv {
    handle: *mut MpvHandle,
    render: *mut MpvRenderContext,
}

impl Mpv {
    /// `options` 在 `mpv_initialize` 之前设，例如 `("hwdec", "videotoolbox")`。
    pub fn new_sw(options: &[(&str, &str)]) -> Result<Self, String> {
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

            let api = cstr("sw");
            let mut params = [
                RenderParam {
                    kind: RENDER_PARAM_API_TYPE,
                    data: api.as_ptr() as *mut c_void,
                },
                RenderParam {
                    kind: RENDER_PARAM_INVALID,
                    data: ptr::null_mut(),
                },
            ];
            let mut render = ptr::null_mut();
            check(
                mpv_render_context_create(&mut render, handle, params.as_mut_ptr()),
                "mpv_render_context_create(sw)",
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

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: 先释放渲染上下文，再销毁核心——文档要求的顺序。
        unsafe {
            mpv_render_context_free(self.render);
            mpv_terminate_destroy(self.handle);
        }
    }
}
