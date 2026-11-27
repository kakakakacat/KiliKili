#version 460 core
#include <flutter/runtime_effect.glsl>

// 尺寸和背板图像由 Flutter 的 ImageFilter 自动绑定。
uniform vec2 u_size;
// 以下由 `GlassNavigationSurface` 设置，float 下标从 2 起，顺序即声明顺序。
// 全部留 0 时与 #266 之前逐像素相同：不着色、不画水滴。
//
// **只用标量 float，不用 vec3/vec4。** `FragmentShader.setFloat` 的下标是
// 紧密排列的，而 vec2 之后紧跟 vec4 时 std140 会补 8 字节对齐，后面的值
// 整体错位——第一次上真机就是这样：着色和水滴全读成了错的值，底栏透得
// 看不清（#266）。标量按 4 字节对齐，不会插填充。
//
// 选中水滴的外接框：左、上、右、下，**占底栏宽高的比例**而不是像素——
// ImageFilter 给到的坐标是背板纹理的像素，宿主那边只有逻辑像素，用比例
// 就不必关心两者之间差了几倍的设备像素比。右 <= 左 表示不画水滴。
uniform float u_drop_left;
uniform float u_drop_top;
uniform float u_drop_right;
uniform float u_drop_bottom;
// 水滴拉伸程度 0..1（`bili_core::glass_droplet` 的 `stretch`），决定中段
// 收细多少。
uniform float u_drop_stretch;
// 底栏着色：颜色，与沿对角线三档的不透明度（左上 / 中 / 右下）。着色
// 原先是盖在 shader 输出之上的一层 DecoratedBox，水滴里的折射会被那层
// 0.7 左右的遮罩压得几乎看不见，所以挪进来，水滴内外用不同的浓度。
uniform float u_tint_r;
uniform float u_tint_g;
uniform float u_tint_b;
uniform float u_tint_alpha0;
uniform float u_tint_alpha1;
uniform float u_tint_alpha2;
uniform sampler2D u_backdrop;
out vec4 fragColor;

// 背板采样，坐标是像素。
vec3 sampleBackdrop(vec2 p) {
  return texture(u_backdrop, clamp(p / u_size, vec2(0.0), vec2(1.0))).rgb;
}

// 圆角矩形 SDF 与解析法线，与 spikes/slint-glass/shaders/glass_sdf.glsl 的
// `sd_round_box` / `sd_round_box_normal` 同式（ADR-016 决策八）。那份在
// spike 目录里，产品 shader 不该依赖 spike，所以抄过来；改动时两边一起改，
// 等 #224 把它迁进正式目录后改成 include。
float sdRoundBox(vec2 p, vec2 b, float r) {
  r = min(r, min(b.x, b.y));
  vec2 q = abs(p) - b + r;
  return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

vec2 sdRoundBoxNormal(vec2 p, vec2 b, float r) {
  r = min(r, min(b.x, b.y));
  vec2 q = abs(p) - b + r;
  vec2 g = max(q, vec2(0.0));
  if (g.x <= 0.0 && g.y <= 0.0) {
    g = q.x > q.y ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
  }
  return normalize(sign(p) * g + vec2(1e-4));
}

void main() {
  vec2 p = FlutterFragCoord().xy;
  vec2 center = u_size * 0.5;
  float radius = u_size.y * 0.5;
  // 胶囊的有符号距离决定边缘折射强度，中心区域保持内容清晰。
  vec2 q = abs(p - center) - vec2(max(center.x - radius, 0.0), 0.0);
  float distance = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
  vec2 normal = normalize(vec2(
    sign(p.x - center.x) * max(abs(p.x - center.x) - (center.x - radius), 0.0),
    p.y - center.y
  ) + vec2(0.0001));
  float rim = 1.0 - smoothstep(0.0, 15.0, -distance);
  // 不要在 GLES 上翻 uv.y。这里原本有一个 `#ifdef IMPELLER_TARGET_OPENGLES`
  // 的补偿，照搬「GLES 纹理原点在左下」的老经验盲写，而它在 #230 打开
  // Impeller 之前从未被执行过（iOS 走 Metal，Android 走 Skia）。真跑起来
  // 才发现 Flutter 3.47 的 Impeller 给到 ImageFilter.shader 的背板两条
  // 后端朝向一致，补偿反而把画面翻了过来：导航栏里映出屏幕对称位置的内容，
  // 看上去既倒置又不透明。见 #230。
  vec2 uv = (p + normal * rim * 7.0) / u_size;
  vec4 backdrop = texture(u_backdrop, clamp(uv, vec2(0.0), vec2(1.0)));
  float glint = (1.0 - smoothstep(0.0, 4.0, -distance)) * 0.18;
  vec3 bar = mix(backdrop.rgb, vec3(1.0), glint);

  // 着色：与原先 DecoratedBox 的 LinearGradient(topLeft → bottomRight) 同
  // 一条投影，三档在 0 / 0.5 / 1 处线性插值。
  float t = clamp(dot(p, u_size) / dot(u_size, u_size), 0.0, 1.0);
  float tintAlpha = t < 0.5
    ? mix(u_tint_alpha0, u_tint_alpha1, t * 2.0)
    : mix(u_tint_alpha1, u_tint_alpha2, t * 2.0 - 1.0);
  vec3 tint = vec3(u_tint_r, u_tint_g, u_tint_b);
  vec3 color = mix(bar, tint, tintAlpha);

  // ---- 选中水滴（#266） ----
  vec2 dropMin = vec2(u_drop_left, u_drop_top) * u_size;
  vec2 dropMax = vec2(u_drop_right, u_drop_bottom) * u_size;
  if (dropMax.x > dropMin.x && dropMax.y > dropMin.y) {
    vec2 dc = (dropMin + dropMax) * 0.5;
    vec2 db = (dropMax - dropMin) * 0.5;
    vec2 dp = p - dc;
    // 中段收细：按横向位置把纵坐标放大，等效于中间一段变矮。只是对定义
    // 域做形变，距离不再严格是欧氏距离，但收细量最多 12%，对只有几个像素
    // 宽的边缘带影响看不出来。
    float along = dp.x / max(db.x, 1.0);
    float neck = 1.0 - 0.12 * u_drop_stretch * exp(-along * along * 4.0);
    vec2 wp = vec2(dp.x, dp.y / neck);
    float d = sdRoundBox(wp, db, db.y);
    vec2 n = sdRoundBoxNormal(wp, db, db.y);
    // 抗锯齿带宽随尺寸走：静止高 56 逻辑像素的水滴，在 1x 到 3x 屏上都约
    // 一个设备像素。
    float aa = max(db.y / 28.0, 0.75);
    float inside = 1.0 - smoothstep(-aa, aa, d);

    if (inside > 0.0) {
      // 透镜：中心略放大，边缘沿法线把采样点往外推——水滴的厚度感。
      float edge = 1.0 - smoothstep(0.0, db.y * 0.7, -d);
      vec2 lens = dc + dp * 0.9 + n * edge * edge * db.y * 0.35;
      // 轻度模糊：水滴里的着色比底栏薄，不糊一下的话底下封面上的字会和
      // 选中标签打架（#252 同一个问题）。四个点，只在水滴范围内付。
      float r = db.y * 0.08;
      vec3 inner = (sampleBackdrop(lens + vec2(r, r))
        + sampleBackdrop(lens + vec2(-r, r))
        + sampleBackdrop(lens + vec2(r, -r))
        + sampleBackdrop(lens + vec2(-r, -r))) * 0.25;
      inner = mix(inner, tint, tintAlpha * 0.72);
      // 高光：贴边一圈细亮线，朝左上方的那段更亮。
      float rimLine = 1.0 - smoothstep(0.0, aa * 2.5, -d);
      float facing = clamp(dot(n, normalize(vec2(-0.6, -0.8))), 0.0, 1.0);
      float spec = rimLine * (0.28 + 0.45 * facing * facing);
      // 体内自上而下一层很淡的提亮，像光从玻璃顶面透进来。
      float body = (1.0 - clamp((dp.y + db.y) / (2.0 * db.y), 0.0, 1.0)) * 0.06;
      inner = mix(inner, vec3(1.0), clamp(spec + body, 0.0, 1.0));
      color = mix(color, inner, inside);
    }
    // 水滴外缘一圈很窄的暗边，浅色背景上也分得清轮廓。
    float outer = (1.0 - smoothstep(0.0, aa * 3.0, d)) * (1.0 - inside);
    color = mix(color, vec3(0.0), outer * 0.10);
  }

  fragColor = vec4(color, backdrop.a);
}
