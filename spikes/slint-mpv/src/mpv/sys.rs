//! libmpv 的最小 FFI 面，**运行期 dlopen**，不在构建期链接。
//!
//! 为什么自己写而不是用 libmpv2/libmpv-sys：那两个都在构建期找 libmpv
//! （pkg-config 或 bindgen + libclang）。对一个要在两台不同系统的开发机上
//! 跑起来的 spike 来说，这是最大的一处摩擦——macOS 上 `brew install mpv`、
//! Windows 上放一个 dll 就该能跑。代价是下面这些声明要自己盯住，所以：
//!
//! **改这个文件前先对着 `mpv/client.h` 与 `mpv/render_gl.h` 核一遍。**
//! 当前声明核对自 mpv-dev-x86_64-20260607-git-43b14a4（即 media_kit 在
//! Windows 上给 app 用的同一份构建，见 README）。结构体字段顺序错了不会
//! 编译失败，只会在运行时给出垃圾值或直接崩。

// 声明保持成套，哪怕这一版没用上：对着头文件核一遍比「用到哪个加哪个」
// 安全得多，少一个字段就是一次运行期崩溃，而不是编译错误。
#![allow(dead_code)]

use std::ffi::{CString, c_char, c_double, c_int, c_void};

use anyhow::{Context, Result, bail};

#[repr(C)]
#[derive(Debug)]
pub struct MpvHandle {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Debug)]
pub struct MpvRenderContext {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MpvEvent {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MpvEventProperty {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MpvRenderParam {
    pub type_: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MpvOpenglInitParams {
    pub get_proc_address:
        Option<unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char) -> *mut c_void>,
    pub get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MpvOpenglFbo {
    pub fbo: c_int,
    pub w: c_int,
    pub h: c_int,
    pub internal_format: c_int,
}

pub mod format {
    use std::ffi::c_int;
    pub const NONE: c_int = 0;
    pub const STRING: c_int = 1;
    pub const FLAG: c_int = 3;
    pub const INT64: c_int = 4;
    pub const DOUBLE: c_int = 5;
}

pub mod event {
    use std::ffi::c_int;
    pub const NONE: c_int = 0;
    pub const SET_PROPERTY_REPLY: c_int = 4;
    pub const COMMAND_REPLY: c_int = 5;
    pub const SHUTDOWN: c_int = 1;
    pub const START_FILE: c_int = 6;
    pub const END_FILE: c_int = 7;
    pub const FILE_LOADED: c_int = 8;
    pub const SEEK: c_int = 20;
    pub const PLAYBACK_RESTART: c_int = 21;
    pub const PROPERTY_CHANGE: c_int = 22;
}

pub mod render_param {
    use std::ffi::c_int;
    pub const INVALID: c_int = 0;
    pub const API_TYPE: c_int = 1;
    pub const OPENGL_INIT_PARAMS: c_int = 2;
    pub const OPENGL_FBO: c_int = 3;
    pub const FLIP_Y: c_int = 4;
    pub const NEXT_FRAME_INFO: c_int = 11;
    pub const BLOCK_FOR_TARGET_TIME: c_int = 12;
}

/// `mpv_render_frame_info`（render.h）。`NEXT_FRAME_INFO` 经
/// `mpv_render_context_get_info` 取回，说的是**下一张**要画的帧。
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MpvRenderFrameInfo {
    pub flags: u64,
    /// 与 `mpv_get_time_us` 同一时基；redraw 或 display-sync 模式下可能是 0。
    pub target_time: i64,
}

/// `mpv_render_frame_info_flag`。
pub mod frame_info {
    pub const PRESENT: u64 = 1 << 0;
    pub const REDRAW: u64 = 1 << 1;
    pub const REPEAT: u64 = 1 << 2;
    pub const BLOCK_VSYNC: u64 = 1 << 3;
}

/// `mpv_render_context_update` 的返回位：有新帧要画。
pub const RENDER_UPDATE_FRAME: u64 = 1;

type FnCreate = unsafe extern "C" fn() -> *mut MpvHandle;
type FnInitialize = unsafe extern "C" fn(*mut MpvHandle) -> c_int;
type FnTerminateDestroy = unsafe extern "C" fn(*mut MpvHandle);
type FnSetOptionString =
    unsafe extern "C" fn(*mut MpvHandle, *const c_char, *const c_char) -> c_int;
type FnSetPropertyString =
    unsafe extern "C" fn(*mut MpvHandle, *const c_char, *const c_char) -> c_int;
type FnSetProperty =
    unsafe extern "C" fn(*mut MpvHandle, *const c_char, c_int, *mut c_void) -> c_int;
type FnGetProperty =
    unsafe extern "C" fn(*mut MpvHandle, *const c_char, c_int, *mut c_void) -> c_int;
type FnCommandAsync = unsafe extern "C" fn(*mut MpvHandle, u64, *mut *const c_char) -> c_int;
type FnSetPropertyAsync =
    unsafe extern "C" fn(*mut MpvHandle, u64, *const c_char, c_int, *mut c_void) -> c_int;
type FnObserveProperty = unsafe extern "C" fn(*mut MpvHandle, u64, *const c_char, c_int) -> c_int;
type FnWaitEvent = unsafe extern "C" fn(*mut MpvHandle, c_double) -> *mut MpvEvent;
type FnWakeup = unsafe extern "C" fn(*mut MpvHandle);
type FnFree = unsafe extern "C" fn(*mut c_void);
type FnErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type FnClientApiVersion = unsafe extern "C" fn() -> u64;
type FnRenderContextCreate =
    unsafe extern "C" fn(*mut *mut MpvRenderContext, *mut MpvHandle, *mut MpvRenderParam) -> c_int;
type FnRenderContextRender =
    unsafe extern "C" fn(*mut MpvRenderContext, *mut MpvRenderParam) -> c_int;
type FnRenderContextUpdate = unsafe extern "C" fn(*mut MpvRenderContext) -> u64;
type FnRenderContextFree = unsafe extern "C" fn(*mut MpvRenderContext);
type FnRenderContextGetInfo = unsafe extern "C" fn(*mut MpvRenderContext, MpvRenderParam) -> c_int;
type FnGetTimeUs = unsafe extern "C" fn(*mut MpvHandle) -> i64;

pub struct MpvLib {
    pub create: FnCreate,
    pub initialize: FnInitialize,
    pub terminate_destroy: FnTerminateDestroy,
    pub set_option_string: FnSetOptionString,
    pub set_property_string: FnSetPropertyString,
    pub set_property: FnSetProperty,
    pub set_property_async: FnSetPropertyAsync,
    pub get_property: FnGetProperty,
    pub command_async: FnCommandAsync,
    pub observe_property: FnObserveProperty,
    pub wait_event: FnWaitEvent,
    pub wakeup: FnWakeup,
    pub free: FnFree,
    pub error_string: FnErrorString,
    pub client_api_version: FnClientApiVersion,
    pub render_context_create: FnRenderContextCreate,
    pub render_context_render: FnRenderContextRender,
    pub render_context_update: FnRenderContextUpdate,
    pub render_context_free: FnRenderContextFree,
    pub render_context_get_info: FnRenderContextGetInfo,
    pub get_time_us: FnGetTimeUs,
    /// 必须活到最后一个函数指针不再被调用为止。
    _lib: libloading::Library,
    pub path: String,
}

impl std::fmt::Debug for MpvLib {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MpvLib").field("path", &self.path).finish()
    }
}

/// 各平台的默认文件名。显式指定用 `--mpv-lib` 或 `KILIKILI_MPV_LIB`。
#[must_use]
pub fn default_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["libmpv-2.dll", "mpv-2.dll", "libmpv.dll", "mpv-1.dll"]
    } else if cfg!(target_os = "macos") {
        // Homebrew 的 mpv 装在 /opt/homebrew/lib（Apple Silicon）或
        // /usr/local/lib（Intel）。dlopen 不会去翻这两个目录以外的地方，
        // 所以这里把绝对路径也列上。
        &[
            "libmpv.2.dylib",
            "libmpv.dylib",
            "/opt/homebrew/lib/libmpv.2.dylib",
            "/usr/local/lib/libmpv.2.dylib",
        ]
    } else {
        &["libmpv.so.2", "libmpv.so.1", "libmpv.so"]
    }
}

impl MpvLib {
    /// 按候选名依次 dlopen，全部失败就把每一条失败原因都报出来——
    /// 「找不到 libmpv」而不说找过哪些名字，是最浪费时间的一类错误信息。
    pub fn load(explicit: Option<&str>) -> Result<Self> {
        let candidates: Vec<String> = match explicit {
            Some(path) => vec![path.to_owned()],
            None => default_names().iter().map(|s| (*s).to_owned()).collect(),
        };
        let mut failures = Vec::new();
        for name in &candidates {
            match unsafe { libloading::Library::new(name) } {
                Ok(lib) => return unsafe { Self::bind(lib, name.clone()) },
                Err(err) => failures.push(format!("  {name}: {err}")),
            }
        }
        bail!(
            "加载 libmpv 失败，试过：\n{}\n用 --mpv-lib <路径> 指定，或装一个 mpv \
             （macOS: brew install mpv）",
            failures.join("\n")
        );
    }

    unsafe fn bind(lib: libloading::Library, path: String) -> Result<Self> {
        macro_rules! sym {
            ($name:literal) => {
                *unsafe { lib.get(concat!($name, "\0").as_bytes()) }
                    .with_context(|| format!("libmpv 里没有符号 {}", $name))?
            };
        }
        Ok(Self {
            create: sym!("mpv_create"),
            initialize: sym!("mpv_initialize"),
            terminate_destroy: sym!("mpv_terminate_destroy"),
            set_option_string: sym!("mpv_set_option_string"),
            set_property_string: sym!("mpv_set_property_string"),
            set_property: sym!("mpv_set_property"),
            set_property_async: sym!("mpv_set_property_async"),
            get_property: sym!("mpv_get_property"),
            command_async: sym!("mpv_command_async"),
            observe_property: sym!("mpv_observe_property"),
            wait_event: sym!("mpv_wait_event"),
            wakeup: sym!("mpv_wakeup"),
            free: sym!("mpv_free"),
            error_string: sym!("mpv_error_string"),
            client_api_version: sym!("mpv_client_api_version"),
            render_context_create: sym!("mpv_render_context_create"),
            render_context_render: sym!("mpv_render_context_render"),
            render_context_update: sym!("mpv_render_context_update"),
            render_context_free: sym!("mpv_render_context_free"),
            render_context_get_info: sym!("mpv_render_context_get_info"),
            get_time_us: sym!("mpv_get_time_us"),
            _lib: lib,
            path,
        })
    }

    #[must_use]
    pub fn api_version(&self) -> String {
        let raw = unsafe { (self.client_api_version)() };
        format!("{}.{}", raw >> 16, raw & 0xFFFF)
    }

    /// 把 mpv 的错误码翻成可读文本。
    #[must_use]
    pub fn error_text(&self, code: c_int) -> String {
        let ptr = unsafe { (self.error_string)(code) };
        if ptr.is_null() {
            return format!("mpv 错误 {code}");
        }
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }

    pub fn check(&self, code: c_int, what: &str) -> Result<()> {
        if code < 0 {
            bail!("{what} 失败：{}", self.error_text(code));
        }
        Ok(())
    }
}

/// 把 `&str` 转成以 NUL 结尾的 C 串。
pub fn cstr(value: &str) -> Result<CString> {
    CString::new(value).context("字符串里有 NUL，不能传给 mpv")
}
