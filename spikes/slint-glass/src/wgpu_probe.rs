//! 关口 2 的判据（评估文档 §2.1、§3）。只在 `--features wgpu-probe` 下编译。
//!
//! 它要回答的是**两个**问题，分清楚很重要：
//!
//! 1. 能不能在 Slint 这一帧上直接加画？——**不能，已经定了。**
//!    `GraphicsAPI::WGPU30` 只有 `instance / device / queue` 三个字段
//!    （i-slint-core 1.19.1 `api.rs:306`，`#[non_exhaustive]` 但实际就这三个），
//!    **没有这一帧的渲染目标**。wgpu 不像 GL 有隐式的当前 FBO，拿不到
//!    `TextureView` 就发不出 render pass。候选 A 出局。
//!    下面的 notifier 把每个状态真实拿到的东西打出来，是为了让这条结论
//!    有运行时证据，而不是只有读源码的推断。
//!
//! 2. 那换一条：自己出一张纹理交给 Slint 显示，这条通不通？——下面就是。
//!    这条通了，#130 的「共享材质引擎」才有落点：原生 backdrop-blur 给不了
//!    边缘折射和单点高光，这两件事只能自己画。
//!
//! **这个探针没有解决的那一关，也写在这里，别被画面骗了**：背板纹理是
//! 合成的（下面 `make_backdrop` 自己画的条带），**不是 Slint 这一帧的真实
//! 背景**。「怎么拿到真实背板」仍然是没解决的问题，而它才是关口 2 的本体。
//! 这个探针证明的是管线通、WGSL 跑得起来、纹理导得进去。

use slint::ComponentHandle;
use slint::wgpu_30::wgpu;

use crate::material::{GlassMaterial, GlassUniforms};

/// 玻璃表面的逻辑尺寸，和 .slint 里那个 Image 的摆放一致。
const LOGICAL: [f32; 2] = [430.0, 80.0];
/// 自绘纹理的超采样倍率。2 是为了在 HiDPI 屏上不糊，同时也顺便验一次
/// `glass.wgsl` 里物理→逻辑像素那一除对不对（填 1 的话那个 bug 看不出来）。
const SCALE: f32 = 2.0;

pub fn run_probe() -> Result<(), Box<dyn std::error::Error>> {
    // 没有这一句，后面 notifier 里拿到的就是 NativeOpenGL 而不是 WGPU30。
    slint::BackendSelector::new()
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::default())
        .select()?;

    let app = crate::GlassSpike::new()?;
    crate::apply_env_overrides(&app);

    let weak = app.as_weak();
    // 只建一次。notifier 每帧都会被调用，而建管线、传纹理是一次性的事。
    let mut built = false;

    app.window().set_rendering_notifier(move |state, api| {
        // 先把「这个状态下到底拿到了什么」原样记下来。这是判据 1 的证据。
        let api_desc = match api {
            slint::GraphicsAPI::NativeOpenGL { .. } => "NativeOpenGL",
            slint::GraphicsAPI::WebGL { .. } => "WebGL",
            slint::GraphicsAPI::WGPU30 { .. } => "WGPU30{instance,device,queue}（没有渲染目标）",
            _ => "未知（Slint 又加了新变体）",
        };
        println!("PROBE state={state:?} api={api_desc}");

        let slint::GraphicsAPI::WGPU30 { device, queue, .. } = api else {
            return;
        };

        if !matches!(state, slint::RenderingState::RenderingSetup) || built {
            return;
        }
        built = true;

        match build_glass_texture(device, queue) {
            Ok(texture) => {
                // 这一步就是判据 2：外部纹理能不能进 Slint 的场景。
                // 失败只有两种原因（TextureImportError）：格式不是
                // Rgba8Unorm / Rgba8UnormSrgb，或者 usage 少了
                // TEXTURE_BINDING | RENDER_ATTACHMENT。
                match slint::Image::try_from(texture) {
                    Ok(image) => {
                        if let Some(app) = weak.upgrade() {
                            app.set_probe_texture(image);
                            app.set_probe_visible(true);
                        }
                        println!("PROBE 结论2 自绘纹理导入 Slint：成功");
                    }
                    Err(err) => println!("PROBE 结论2 自绘纹理导入失败：{err:?}"),
                }
            }
            Err(err) => println!("PROBE 建纹理失败：{err}"),
        }
    })?;

    app.run()?;
    Ok(())
}

/// 跑一遍 `shaders/glass.wgsl`，把结果画进一张自己的纹理。
fn build_glass_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<wgpu::Texture, Box<dyn std::error::Error>> {
    let width = (LOGICAL[0] * SCALE) as u32;
    let height = (LOGICAL[1] * SCALE) as u32;

    // ---- 背板。**合成的**，理由见文件头。----
    let backdrop = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("glass-backdrop"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let pixels = make_backdrop(width, height);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &backdrop,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    // ---- 输出纹理。格式与 usage 由 Slint 的导入条件反推，不是随便填的。----
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("glass-out"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    // ---- uniform。字节序与排布见 material.rs 里 as_f32_array 的说明。----
    let uniforms = GlassUniforms::new(&GlassMaterial::FLUTTER_BASELINE, LOGICAL, SCALE);
    let mut uniform_bytes = Vec::with_capacity(32);
    for v in uniforms.as_f32_array() {
        uniform_bytes.extend_from_slice(&v.to_le_bytes());
    }
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glass-uniforms"),
        size: uniform_bytes.len() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buffer, 0, &uniform_bytes);

    // clamp-to-edge 不是可选项：折射把 uv 推出边界是常态，repeat 的话
    // 边缘会出现对侧的背景，看起来像「折射错了」。
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("glass-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("glass.wgsl"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/glass.wgsl").into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("glass-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let backdrop_view = backdrop.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("glass-bg"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&backdrop_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glass-pl"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        // wgpu 30 把 push constant 换成了 immediate data，这里一个都不用。
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("glass-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("glass-enc"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("glass-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        // 全屏三角形，三个顶点，没有顶点缓冲。
        pass.draw(0..3, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    Ok(target)
}

/// 合成一张条带背板，颜色与 `MovingBackdrop` 对齐（#10131a 底 + 三色条
/// alpha 0.55），这样自绘那条和原生那条看起来才是同一个背景。
///
/// **它是画出来的，不是抓来的。** 真实背板怎么拿仍然没解决。
fn make_backdrop(width: u32, height: u32) -> Vec<u8> {
    // 条带几何照 .slint：条宽 56、间距 90 逻辑像素，这里乘 SCALE。
    let band_w = (56.0 * SCALE) as u32;
    let period = (90.0 * SCALE) as u32;
    let base = [16u8, 19, 26];
    // 三色按 alpha 0.55 预先混到底色上，省掉运行时的混合。
    let bands = [[41u8, 80, 147], [137, 48, 96], [26, 117, 63]];

    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for _y in 0..height {
        for x in 0..width {
            let t = x % period;
            let rgb = if t < band_w {
                bands[((x / period) % 3) as usize]
            } else {
                base
            };
            out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    out
}
