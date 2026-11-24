//! CVPixelBuffer → MTLTexture → wgpu::Texture → slint::Image，全程不经过 CPU。
//!
//! 只要 Slint 用的是 femtovg-wgpu（iOS 上就是 Metal），一张 IOSurface 支撑的
//! CVPixelBuffer 就能被 Slint 直接当图片采样。像素由谁写进来不重要：mpv 用
//! GLES 画进同一块 IOSurface（`platform::spike_target_*`），或者 AVPlayer 的
//! `AVPlayerItemVideoOutput` 直接吐出来，走的都是这里。

use std::ffi::c_void;
use std::ptr;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLDevice, MTLTexture, MTLTextureType};
use slint::wgpu_30::wgpu;

use crate::platform;

pub struct Importer {
    device: wgpu::Device,
    mtl_device: *mut c_void,
}

impl Importer {
    /// 从 Slint 的 wgpu 设备取出底下的 MTLDevice。不是 Metal 后端就返回 None。
    pub fn new(device: wgpu::Device) -> Option<Self> {
        // SAFETY: 只读取 hal 设备的原始指针；`device` 被 self 持有，指针随之有效。
        let mtl_device = unsafe {
            let hal = device.as_hal::<wgpu::hal::api::Metal>()?;
            let raw: &Retained<ProtocolObject<dyn MTLDevice>> = hal.raw_device();
            Retained::as_ptr(raw) as *mut c_void
        };
        Some(Self { device, mtl_device })
    }

    /// 把一张 BGRA 的 CVPixelBuffer 包成 Slint 能画的 Image。不拷贝像素：
    /// 返回的 Image 与 pixbuf 共享同一块 IOSurface，之后往 pixbuf 里写的内容
    /// 下一帧就会被 Slint 看到。
    pub fn import(&self, pixbuf: *mut c_void) -> Result<slint::Image, String> {
        let mut keep = ptr::null_mut();
        // SAFETY: pixbuf 是有效的 CVPixelBufferRef；返回值是 +1 的 id<MTLTexture>。
        let raw = unsafe { platform::spike_mtl_texture(pixbuf, self.mtl_device, &mut keep) };
        // SAFETY: 同上，所有权转给 Retained。
        let raw: Retained<ProtocolObject<dyn MTLTexture>> =
            unsafe { Retained::from_raw(raw.cast()) }
                .ok_or("IOSurface 上的 MTLTexture 建不出来")?;
        let (width, height) = (raw.width() as u32, raw.height() as u32);
        // CVPixelBuffer 必须活到 GPU 不再用这张纹理为止：交给 wgpu 的
        // drop 回调，wgpu-core 会等提交完成才真正销毁。
        let keep = keep as usize;
        let on_drop: wgpu::hal::DropCallback =
            Box::new(move || unsafe { platform::spike_cf_release(keep as *mut c_void) });
        // raw 是 IOSurface 上的 RGBA8Unorm + B↔R swizzle 视图（见 bridge.m
        // `spike_mtl_texture`），格式与这里声明的一致：femtovg 与 Skia 两种渲染器都能吃。
        // SAFETY: raw 是 2D、单层、单 mip 的纹理，尺寸与 copy_size 一致。
        let hal_texture = unsafe {
            wgpu::hal::metal::Device::texture_from_raw(
                raw,
                wgpu::TextureFormat::Rgba8Unorm,
                MTLTextureType::Type2D,
                1,
                1,
                wgpu::hal::CopyExtent {
                    width,
                    height,
                    depth: 1,
                },
                Some(on_drop),
            )
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("cvpixelbuffer"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            // RENDER_ATTACHMENT 只为过 Slint 的检查；我们只采样它。
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        // SAFETY: hal_texture 来自同一个 Metal 设备，desc 与它的真实属性一致（格式除外，见上）。
        let texture = unsafe {
            self.device
                .create_texture_from_hal::<wgpu::hal::api::Metal>(
                    hal_texture,
                    &desc,
                    wgpu::TextureUses::RESOURCE,
                )
        };
        slint::Image::try_from(texture).map_err(|e| format!("Image::try_from: {e:?}"))
    }
}
