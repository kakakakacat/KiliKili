//! 只借三个 OpenGL 函数：取渲染器名、取当前 FBO、读回像素。
//!
//! 没有引 glow 之类的加载器——用到的就这三个，多一个依赖就多一处版本要对。
//! 读回像素是本 spike 的**证据产出手段**：一张合成后的 PNG 能直接回答
//! 「弹幕在视频之上吗、HUD 在最上吗、画面有没有被背景盖掉」，比任何文字
//! 描述都硬。

use std::ffi::{CStr, c_void};
use std::path::Path;

use anyhow::{Context, Result, bail};

type GetString = unsafe extern "C" fn(u32) -> *const u8;
type GetIntegerv = unsafe extern "C" fn(u32, *mut i32);
type ReadPixels = unsafe extern "C" fn(i32, i32, i32, i32, u32, u32, *mut c_void);

const GL_VENDOR: u32 = 0x1F00;
const GL_RENDERER: u32 = 0x1F01;
const GL_VERSION: u32 = 0x1F02;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;

pub struct Gl {
    get_string: GetString,
    get_integerv: GetIntegerv,
    read_pixels: ReadPixels,
    /// Windows 上 wglGetProcAddress 对 GL 1.1 的核心函数返回 null，
    /// 得从 opengl32.dll 自己取。保住句柄，别让 dll 在函数指针还活着时卸载。
    _fallback: Option<libloading::Library>,
}

impl std::fmt::Debug for Gl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Gl")
    }
}

impl Gl {
    /// # Safety
    /// 调用时必须处于 Slint 的渲染回调中：那时 GL 上下文是 current 的，
    /// `loader` 也还活着。
    pub unsafe fn load(loader: &dyn Fn(&CStr) -> *const c_void) -> Result<Self> {
        // opengl32.dll 只在 Windows 上需要；其他平台 loader 自己就能给全。
        let fallback = if cfg!(windows) {
            unsafe { libloading::Library::new("opengl32.dll") }.ok()
        } else {
            None
        };

        let resolve = |name: &str| -> Result<*const c_void> {
            let c_name = std::ffi::CString::new(name)?;
            let mut ptr = loader(&c_name);
            if ptr.is_null()
                && let Some(lib) = fallback.as_ref()
            {
                let symbol: Result<libloading::Symbol<*const c_void>, _> =
                    unsafe { lib.get(c_name.as_bytes_with_nul()) };
                if let Ok(symbol) = symbol {
                    ptr = *symbol;
                }
            }
            if ptr.is_null() {
                bail!("取不到 OpenGL 函数 {name}");
            }
            Ok(ptr)
        };

        let get_string = resolve("glGetString")?;
        let get_integerv = resolve("glGetIntegerv")?;
        let read_pixels = resolve("glReadPixels")?;

        Ok(Self {
            get_string: unsafe { std::mem::transmute::<*const c_void, GetString>(get_string) },
            get_integerv: unsafe {
                std::mem::transmute::<*const c_void, GetIntegerv>(get_integerv)
            },
            read_pixels: unsafe { std::mem::transmute::<*const c_void, ReadPixels>(read_pixels) },
            _fallback: fallback,
        })
    }

    fn string(&self, name: u32) -> String {
        let ptr = unsafe { (self.get_string)(name) };
        if ptr.is_null() {
            return "<null>".to_owned();
        }
        unsafe { CStr::from_ptr(ptr.cast()) }
            .to_string_lossy()
            .into_owned()
    }

    /// 报告里的「渲染器」一栏：vendor / renderer / version 三件套，
    /// 它同时能证明跑在独显还是核显、是不是落到了软件渲染。
    pub fn describe(&self) -> String {
        format!(
            "{} | {} | {}",
            self.string(GL_VENDOR),
            self.string(GL_RENDERER),
            self.string(GL_VERSION)
        )
    }

    /// 当前绑定的 draw framebuffer。mpv 要往这个 FBO 上画。
    pub fn draw_framebuffer(&self) -> i32 {
        let mut fbo = 0;
        unsafe { (self.get_integerv)(GL_DRAW_FRAMEBUFFER_BINDING, &raw mut fbo) };
        fbo
    }

    /// 读回整帧并写成 PNG。像素是 GL 坐标（原点在左下），写文件前翻转。
    pub fn capture_png(&self, width: u32, height: u32, path: &Path) -> Result<()> {
        anyhow::ensure!(width > 0 && height > 0, "截图尺寸为 0");
        let row = width as usize * 4;
        let mut pixels = vec![0u8; row * height as usize];
        unsafe {
            (self.read_pixels)(
                0,
                0,
                width as i32,
                height as i32,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                pixels.as_mut_ptr().cast(),
            );
        }
        let mut flipped = vec![0u8; pixels.len()];
        for y in 0..height as usize {
            let src = (height as usize - 1 - y) * row;
            flipped[y * row..y * row + row].copy_from_slice(&pixels[src..src + row]);
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let file = std::fs::File::create(path)
            .with_context(|| format!("建不了截图文件：{}", path.display()))?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // 截图是**在事件循环里同步做的**，压缩越慢堵得越久：4K 一帧是 33MB，
        // debug 构建下默认压缩等级曾经把循环堵了 8.5 秒（整段实验的 p99 就是
        // 那一下）。证据图要的是看得清，不是省磁盘。
        encoder.set_compression(png::Compression::Fast);
        encoder
            .write_header()?
            .write_image_data(&flipped)
            .context("写 PNG 失败")?;
        Ok(())
    }
}
