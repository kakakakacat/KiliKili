// Flutter 侧玻璃的完整 GLSL 移植（issue #130）。
//
// **它移植的不止那 28 行 shader。** Flutter 的玻璃是两层叠出来的：
//
//   BackdropFilter(ImageFilter.shader(liquid_glass.frag))   ← 折射 + glint
//     └─ DecoratedBox(gradient 0.24/0.10/0.18, border 白 0.36)  ← 半透明材质层
//
// 只移植上面那层的话，画出来是一圈几乎看不见的折射边——2026-11-25 第一次
// 跑 GL 覆盖就是这个样子，和原生那条根本没法并排比。两层都在这里。
//
// 在此之上补了 #130 P0 表里 **Flutter 没有**的两项：
// - **模糊**：不在这个文件里。宿主把「清晰背板」还是「模糊背板」绑到
//   u_backdrop 上，shader 只管采样——分支留在 CPU 侧，GPU 这边一条路。
//   （Flutter 那边模糊和折射是 `supportsShader` 的二选一，不是叠加。）
// - **Fresnel 边缘反射**：下面 u_fresnel_* 那两个。
//
// 版本声明与 precision 由 `src/gl_probe.rs` 在运行时按 `#version 330 core` /
// `#version 300 es` 拼在前面——桌面是 GL，Android 是 GLES，正文同一份。
//
// 坐标系，最容易弄反的一处：
//
//   GL 的帧缓冲原点在**左下**，gl_FragCoord 也在左下。于是这里的 p 和
//   backdrop 纹理**同时**是左下原点的——两边一起镜像等价于两边都不镜像，
//   最终像素与 Flutter 版逐像素一致。
//
//   **不要在这里补 uv.y 翻转**。#230 刚交过这个学费（`ea825c773`：GLES 上
//   那次翻转补偿本身才是画面倒置的原因）。只有 p 与纹理朝向不一致时才要翻，
//   这里一致。唯一要照顾的是渐变：Flutter 的 topLeft→bottomRight 在左下
//   原点里起点是 (0, h)，见下面 grad_t。

uniform vec2  u_size;                // 玻璃表面尺寸，逻辑像素
uniform vec2  u_origin;              // 玻璃矩形在窗口中的左下角，物理像素
uniform float u_scale;               // 物理像素 / 逻辑像素
uniform float u_corner_radius;       // 胶囊 = 高度 / 2
uniform float u_rim_width;           // liquid_glass.frag 原式写死的 15.0
uniform float u_refraction;          // 原式写死的 7.0
uniform float u_specular_strength;   // 原式写死的 0.18
uniform float u_specular_width;      // 原式写死的 4.0
uniform vec3  u_surface_alpha;       // 渐变三个停靠点的 alpha
uniform float u_border_alpha;        // 白描边 alpha
uniform float u_border_width;        // 白描边宽度，逻辑像素
uniform float u_fresnel_power;       // Fresnel 指数，0 等于关掉
uniform float u_fresnel_strength;    // Fresnel 强度
uniform sampler2D u_backdrop;        // 本帧真实背板（清晰或已模糊），见 gl_probe.rs
// 背板纹理**比玻璃矩形大一圈**：模糊要采到矩形外面的内容，不然边缘只能
// clamp 自己那一圈像素，糊出来是一道假的硬边。Slint 原生 backdrop-blur
// 的规则是最远采到 3σ 外（#130 关口 2 问的「捕获范围」就是这件事），
// 这里照同一个口径留边距。下面两个 uniform 描述这圈边距：
uniform vec2  u_backdrop_origin;     // 玻璃矩形左下角在背板纹理里的位置，物理像素
uniform vec2  u_backdrop_size;       // 背板纹理整体尺寸，物理像素

out vec4 frag_color;

void main() {
    // 窗口坐标 → 矩形内局部物理像素 → 逻辑像素。
    vec2 local_px = gl_FragCoord.xy - u_origin;
    vec2 p = local_px / u_scale;

    vec2 center = u_size * 0.5;
    float radius = u_corner_radius;

    // ---- 第一层：liquid_glass.frag，算法逐行同式 ----
    vec2 q = abs(p - center) - vec2(max(center.x - radius, 0.0), 0.0);
    float distance = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;

    // +0.0001 防零向量：正中心时两个分量都是 0，normalize 未定义。
    float dx = p.x - center.x;
    float dy = p.y - center.y;
    vec2 normal = normalize(
        vec2(sign(dx) * max(abs(dx) - (center.x - radius), 0.0), dy) + vec2(0.0001)
    );

    float rim = 1.0 - smoothstep(0.0, u_rim_width, -distance);
    // 折射后的采样点：逻辑像素 → 物理像素 → 加上边距偏移 → 归一化。
    // 中间那一步就是上面 u_backdrop_origin 的用处，漏掉它整张背板会偏移
    // 一圈边距那么多，看起来像「折射强度调过头了」。
    vec2 sample_px = (p + normal * rim * u_refraction) * u_scale + u_backdrop_origin;
    vec2 uv = sample_px / u_backdrop_size;
    vec3 color = texture(u_backdrop, clamp(uv, vec2(0.0), vec2(1.0))).rgb;

    float glint = (1.0 - smoothstep(0.0, u_specular_width, -distance)) * u_specular_strength;
    color = mix(color, vec3(1.0), glint);

    // ---- Fresnel 边缘反射（#130 P0，Flutter 版没有）----
    //
    // 2D SDF 里没有视线与法线的夹角可用，用「离边缘多近」当掠射角的代理：
    // 边缘处 rim≈1，往里迅速掉到 0，pow 控制集中度。这是 2D 玻璃的常用
    // 近似，不是物理 Fresnel——ADR 里要按近似写，别写成 Schlick。
    if (u_fresnel_power > 0.0) {
        float fresnel = pow(rim, u_fresnel_power) * u_fresnel_strength;
        color = mix(color, vec3(1.0), fresnel);
    }

    // ---- 第二层：DecoratedBox 的渐变 + 描边 ----
    //
    // Flutter 的 topLeft → bottomRight：左下原点下起点是 (0, h)，所以 y 要
    // 取反。alpha 在三个停靠点之间线性插值，和 LinearGradient 同口径。
    float grad_t = (p.x / u_size.x + (1.0 - p.y / u_size.y)) * 0.5;
    float surface_a = grad_t < 0.5
        ? mix(u_surface_alpha.x, u_surface_alpha.y, grad_t * 2.0)
        : mix(u_surface_alpha.y, u_surface_alpha.z, (grad_t - 0.5) * 2.0);
    // Demo 里用白色近似 ColorScheme.surface，见 material.rs 的 surface_alpha。
    color = mix(color, vec3(1.0), surface_a);

    // 描边：贴着边缘往里 border_width 的一圈。
    float aa = 1.0 / u_scale;
    float border = 1.0 - smoothstep(u_border_width - aa, u_border_width + aa, -distance);
    color = mix(color, vec3(1.0), border * u_border_alpha);

    // Flutter 那边胶囊外由 ClipRRect 裁掉，这里画的是整个外接矩形，
    // 所以形状得自己出：用 SDF 当覆盖率，配 src-alpha 混合，边缘才不是锯齿。
    float coverage = 1.0 - smoothstep(-aa, aa, distance);
    frag_color = vec4(color, coverage);
}
