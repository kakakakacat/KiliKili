// Flutter 侧 `shaders/liquid_glass.frag` 的逐行移植（issue #130）。
//
// **这份没有编译过，naga 的报错一个都还没见过。** 逐项对照账见
// docs/m0/spikes/2026-11-24-glass-wgpu-eval.md §5。
//
// 算法刻意一行不改：那 28 行 GLSL 是目前玻璃观感的事实规格，重新调参会让
// 两端的视觉再也对不回去。这里只处理语言与环境差异，差异有且只有三处：
// 坐标系（逻辑像素 vs 物理像素）、背板纹理的绑定方式、以及不需要 Y 翻转。

struct GlassUniforms {
    // 玻璃表面尺寸，**逻辑像素**。对应 GLSL 的 u_size。
    size: vec2<f32>,
    // 物理像素 / 逻辑像素。
    //
    // 这是移植里第一个会咬人的地方：Flutter 的 fragment shader 跑在逻辑
    // 像素坐标系里，下面 rim_width / refraction / specular_width 三个常数
    // 都是按逻辑像素调出来的；而 wgpu 的 @builtin(position) 给的是物理
    // 像素。不在入口处除掉这个比例，2x 屏上边缘折射带只有一半宽——而且
    // 它不会报错，只会「看起来有点不对」。
    scale: f32,
    // 胶囊：等于 size.y * 0.5。做成参数是为了 Phase 2 扩展圆角矩形与圆形，
    // 这一轮调用方仍然按胶囊填。
    corner_radius: f32,
    // GLSL 里写死的 15.0：边缘折射带宽，逻辑像素。
    rim_width: f32,
    // GLSL 里写死的 7.0：沿法线的背景 UV 偏移量，逻辑像素。
    refraction_strength: f32,
    // GLSL 里写死的 0.18：高光向白色 mix 的强度。
    specular_strength: f32,
    // GLSL 里写死的 4.0：高光带宽，逻辑像素。
    specular_width: f32,
}

@group(0) @binding(0) var<uniform> u: GlassUniforms;
// 背板纹理。
//
// Flutter 那边这一张是 ImageFilter 自动绑定的；在 wgpu 侧「这张纹理从哪
// 来」就是 #130 的关口 2 本身，见评估文档 §3。shader 不关心它是怎么来的，
// 但**关心它干不干净**：如果它含有上一帧的玻璃，下面的采样就是自采，
// 画面会一帧比一帧白。
@group(0) @binding(1) var backdrop_tex: texture_2d<f32>;
// address mode 必须是 clamp-to-edge。下面虽然已经 clamp 了 uv，但折射偏移
// 把 uv 推出边界是常态，两道保险都要有。
@group(0) @binding(2) var backdrop_sampler: sampler;

// 全屏三角形（不是两个三角形的 quad）：少一条对角线上的重复片元，也不用
// 顶点缓冲。
@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(idx) / 2) * 4.0 - 1.0;
    let y = f32(i32(idx) & 1) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // 物理像素 → 逻辑像素。这一除就是上面 scale 注释里说的那件事。
    let p = frag_coord.xy / u.scale;
    let center = u.size * 0.5;
    let radius = u.corner_radius;

    // 胶囊的有符号距离决定边缘折射强度，中心区域保持内容清晰。
    // 与 GLSL 完全同式。
    let q = abs(p - center) - vec2<f32>(max(center.x - radius, 0.0), 0.0);
    let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;

    // +0.0001 是防零向量：正中心时两个分量都是 0，normalize 在 WGSL 里对
    // 零向量同样未定义。GLSL 原式带着它，这里必须一起带过来。
    let dx = p.x - center.x;
    let dy = p.y - center.y;
    let normal = normalize(
        vec2<f32>(sign(dx) * max(abs(dx) - (center.x - radius), 0.0), dy)
            + vec2<f32>(0.0001)
    );

    let rim = 1.0 - smoothstep(0.0, u.rim_width, -distance);
    // 偏移后归一化到 [0,1]。注意这里**没有** GLSL 那段
    // `#ifdef IMPELLER_TARGET_OPENGLES` 的 uv.y 翻转：wgpu 的纹理坐标统一
    // 左上原点，没有 GL 的原点差异。
    //
    // 但这不等于翻转问题不会再出现——背板纹理自己是怎么产生的决定了它的
    // 朝向。SPIKE-001 / SPIKE-004 记过 mpv 那条路上三端要 --flip-y、iOS
    // 不要，同一个坑换了个地方。
    let uv = (p + normal * rim * u.refraction_strength) / u.size;
    let backdrop = textureSample(
        backdrop_tex,
        backdrop_sampler,
        clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0))
    );

    let glint = (1.0 - smoothstep(0.0, u.specular_width, -distance)) * u.specular_strength;
    // 向白色 mix。
    //
    // 待验证：导入 Slint 的纹理只能是 Rgba8Unorm 或 Rgba8UnormSrgb
    // （TextureImportError 只有 InvalidFormat / InvalidUsage 两种），而同一个
    // mix 在 linear 与 sRGB 下观感不同。选哪个不能在纸上定，要并排看一次。
    return vec4<f32>(mix(backdrop.rgb, vec3<f32>(1.0), glint), backdrop.a);
}
