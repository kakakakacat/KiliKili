// Flutter 侧玻璃的完整 GLSL 移植，加上 #130 Phase 2 的四项新能力。
//
// **它移植的不止那 28 行 shader。** Flutter 的玻璃是两层叠出来的：
//
//   BackdropFilter(ImageFilter.shader(liquid_glass.frag))   ← 折射 + glint
//     └─ DecoratedBox(gradient 0.24/0.10/0.18, border 白 0.36)  ← 半透明材质层
//
// 只移植上面那层的话，画出来是一圈几乎看不见的折射边——2026-11-25 第一次
// 跑 GL 覆盖就是这个样子，和原生那条根本没法并排比。两层都在这里。
//
// 在此之上补了 #130 P0 / Phase 2 里 **Flutter 没有**的几项：
//
// - **模糊**：不在这个文件里。宿主把清晰背板与**两级**模糊背板一起绑上来，
//   这里按 u_blur_levels 选。分支留在 CPU 侧决定级数，GPU 这边一条路。
//   （Flutter 那边模糊和折射是 `supportsShader` 的二选一，不是叠加。）
// - **Fresnel 边缘反射**：下面 u_fresnel_* 那两个。
// - **多尺度模糊**（Phase 2）：边缘采细级、体部采粗级，见 backdrop() 与
//   u_blur_depth。
// - **动态高光**（Phase 2）：光源方向 + 顶边常驻高光，见 u_light_dir /
//   u_specular_sharpness / u_top_highlight。u_specular_sharpness 为 0 时
//   整段退回 Flutter 的静态 glint——**baseline 档靠这个退化保持同式**。
// - **形状融合**（Phase 2）：u_shape_count 为 2 时两个形状走 smooth union，
//   见 glass_sdf.glsl。
//
// 形状与法线在 `glass_sdf.glsl` 里，由 `src/gl_probe.rs` 拼在本文件前面；
// 版本声明与 precision 同样由它按 `#version 330 core` / `#version 300 es`
// 补在最前——桌面是 GL，Android 是 GLES，正文同一份。
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

uniform vec2  u_size;                // 覆盖矩形（多形状时是并集外接框）尺寸，逻辑像素
uniform vec2  u_origin;              // 覆盖矩形在窗口中的左下角，物理像素
uniform float u_scale;               // 物理像素 / 逻辑像素

// ---- 形状。1 个或 2 个，中心与半宽半高都相对覆盖矩形左下角，逻辑像素 ----
//
// 圆角矩形 / 圆 / 胶囊是同一个 SDF，差别只在 u_shape_radius，见 glass_sdf.glsl。
uniform int   u_shape_count;
uniform vec2  u_shape_center[2];
uniform vec2  u_shape_half[2];
uniform float u_shape_radius[2];
uniform float u_merge_k;             // 融合半径，逻辑像素；0 = 硬并集

uniform float u_rim_width;           // liquid_glass.frag 原式写死的 15.0
uniform float u_refraction;          // 原式写死的 7.0
uniform float u_specular_strength;   // 原式写死的 0.18
uniform float u_specular_width;      // 原式写死的 4.0
uniform float u_specular_sharpness;  // N·L 的集中度；0 = 退回静态 glint
uniform float u_top_highlight;       // 顶边常驻高光强度；0 = 关
uniform vec2  u_light_dir;           // 光源方向，已归一化，左下原点的 y 向上
uniform vec3  u_surface_alpha;       // 渐变三个停靠点的 alpha
uniform float u_border_alpha;        // 白描边 alpha
uniform float u_border_width;        // 白描边宽度，逻辑像素
uniform float u_fresnel_power;       // Fresnel 指数，0 等于关掉
uniform float u_fresnel_strength;    // Fresnel 强度
uniform float u_opacity;             // 材质整体不透明度

// ---- 背板。三张一起绑，按 u_blur_levels 选，见 backdrop() ----
uniform sampler2D u_backdrop;        // 本帧真实背板，清晰，见 gl_probe.rs
uniform sampler2D u_blur_fine;       // 第 0 级模糊（σ）
uniform sampler2D u_blur_coarse;     // 最后一级模糊（σ 逐级翻倍）
uniform int   u_blur_levels;         // 0 = 不模糊 / 1 = 单级 / ≥2 = 多尺度
uniform float u_blur_depth;          // 从细级过渡到粗级的深度，逻辑像素
// 背板纹理**比覆盖矩形大一圈**：模糊要采到矩形外面的内容，不然边缘只能
// clamp 自己那一圈像素，糊出来是一道假的硬边；**折射同样会把采样点推出
// 矩形**，所以即使不模糊这圈边距也不能是 0（Phase 1 这里是 0，baseline 档
// 外缘那 7px 一直在采 clamp 值，Phase 2 一并修了）。边距取
// max(3σ, 折射强度)，3σ 那一半和 Slint 原生 backdrop-blur 同口径。
// 下面两个 uniform 描述这圈边距：
uniform vec2  u_backdrop_origin;     // 覆盖矩形左下角在背板纹理里的位置，物理像素
uniform vec2  u_backdrop_size;       // 背板纹理整体尺寸，物理像素

out vec4 frag_color;

/// 按深度在两级模糊之间取背板。
///
/// `coarse_t` 是「这一点有多深」：边缘为 0 取细级，体部为 1 取粗级。
///
/// 为什么边缘要用**细**级：折射靠的是背板里的高频细节沿法线错位，粗级
/// 已经把那些细节糊没了，边缘再采粗级的话折射就只剩一道颜色差。所以
/// 多尺度在这里不是「省钱」，是**边缘要清楚、体部要糊**这件事本身只能
/// 靠两级才做得到——单级没法同时满足。
vec3 backdrop(vec2 uv, float coarse_t) {
    if (u_blur_levels <= 0) {
        return texture(u_backdrop, uv).rgb;
    }
    vec3 fine = texture(u_blur_fine, uv).rgb;
    if (u_blur_levels == 1) {
        return fine;
    }
    return mix(fine, texture(u_blur_coarse, uv).rgb, coarse_t);
}

void main() {
    // 窗口坐标 → 覆盖矩形内局部物理像素 → 逻辑像素。
    vec2 local_px = gl_FragCoord.xy - u_origin;
    vec2 p = local_px / u_scale;

    // ---- 形状与法线 ----
    float distance = sd_round_box(p - u_shape_center[0], u_shape_half[0], u_shape_radius[0]);
    vec2 normal = sd_round_box_normal(p - u_shape_center[0], u_shape_half[0], u_shape_radius[0]);
    if (u_shape_count > 1) {
        float d1 = sd_round_box(p - u_shape_center[1], u_shape_half[1], u_shape_radius[1]);
        vec2 n1 = sd_round_box_normal(p - u_shape_center[1], u_shape_half[1], u_shape_radius[1]);
        float h;
        distance = sd_smooth_union(distance, d1, u_merge_k, h);
        // 用同一个 h 混法线，理由见 sd_smooth_union 的文档。
        normal = normalize(mix(n1, normal, h) + vec2(1e-4));
    }

    // ---- 第一层：liquid_glass.frag，算法逐行同式 ----
    float rim = 1.0 - smoothstep(0.0, u_rim_width, -distance);

    // 这一点有多深：边缘 0、体部 1。u_blur_depth 为 0 时整面都用粗级。
    float coarse_t = u_blur_depth > 0.0
        ? smoothstep(0.0, u_blur_depth, -distance)
        : 1.0;

    // 折射后的采样点：逻辑像素 → 物理像素 → 加上边距偏移 → 归一化。
    // 中间那一步就是上面 u_backdrop_origin 的用处，漏掉它整张背板会偏移
    // 一圈边距那么多，看起来像「折射强度调过头了」。
    vec2 sample_px = (p + normal * rim * u_refraction) * u_scale + u_backdrop_origin;
    vec2 uv = sample_px / u_backdrop_size;
    vec3 color = backdrop(clamp(uv, vec2(0.0), vec2(1.0)), coarse_t);

    // ---- 高光 ----
    //
    // 基础那一圈（贴边 u_specular_width 的恒定 glint）**两档都有**，动态那
    // 两项是加在它上面的，不是替换它。
    //
    // 一开始写成了「有方向性就只画方向性」，结果是朝光那一小段之外整圈
    // glint 全没了——比 Phase 1 还暗，而且实测四个角度之间的亮度差只有
    // 1~2 个灰阶，等于「动态」这件事看不出来。两个毛病同一个根因。
    float band = 1.0 - smoothstep(0.0, u_specular_width, -distance);
    float glint = band * u_specular_strength;
    if (u_specular_sharpness > 0.0) {
        // 局部高亮：法线朝着光源的那一段再亮一道，指数控制范围。
        //
        // 它铺在**折射带**（rim，15px）上而不是那 4px 的 glint 带上：厚玻璃
        // 的边缘高光铺在整条斜面上，挤进最外两像素里是看不出来的——而
        // 「看得出来」正是这一项要验的事。
        float ndl = max(dot(normal, u_light_dir), 0.0);
        glint += rim * pow(ndl, u_specular_sharpness) * u_specular_strength;
        // 顶边常驻高光：真玻璃的上沿总比下沿亮一道（天光），与光源方向
        // 无关，所以单独一项而不是并进上面。
        glint += rim * max(normal.y, 0.0) * u_top_highlight;
    }
    color = mix(color, vec3(1.0), clamp(glint, 0.0, 1.0));

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
    // 多形状时它横跨整个并集外接框——两个形状共享一道渐变，正是「融合成
    // 一块玻璃」该有的样子。
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
    frag_color = vec4(color, coverage * u_opacity);
}
