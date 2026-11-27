#version 460 core
#include <flutter/runtime_effect.glsl>

// 底栏选中透镜（#266）：切换 tab 或按住时升起的那块透明玻璃。
//
// 参照所有者给的 iOS 26 录屏逐帧看到的样子：
// - 放大下面的图标与文字；
// - 边缘折射很强，带一点色散（彩边）；
// - 透镜里的图标被染成选中色（录屏里深色图标进了透镜就变蓝）；
// - 贴边一圈白色高光。
// 静止时没有透镜，只有一块哑光灰色胶囊（宿主那边画），所以本层只在
// 升起程度 > 0 时挂上，平时不付这一次背板采样。
//
// 它是叠在导航内容**之上**的 BackdropFilter，裁成透镜的胶囊形，背板里
// 已经有底栏与图标。
//
// ## 坐标：不要假设 FlutterFragCoord 相对透镜
//
// 第一版照 liquid_glass.frag 的写法，以为 FlutterFragCoord 与 u_size 都是
// 「被裁剪的这一小块」。DN-ENG81（Impeller/Vulkan）上录屏证明不是：它们是
// **整屏背板**的物理像素。结果透镜以屏幕中心为圆心放大、采到屏幕别处的
// 内容——白底页面上是一团纯白，封面上是一块彩虹色的乱码（#266）。
//
// 所以宿主把透镜在屏幕上的物理像素位置传进来。为防另一种后端真按裁剪区
// 域给坐标，u_size 与透镜尺寸几乎相等时按本地坐标处理。

uniform vec2 u_size;
// 以下由 `GlassNavigationSurface` 设置，float 下标从 2 起，顺序即声明顺序。
// **只用标量 float**：vec2 后面紧跟 vec3/vec4 时 std140 会补对齐，而
// `setFloat` 的下标是紧密排列的，值会整体错位（#266 在真机上踩过）。
//
// 放大倍数，`bili_core::glass_droplet` 的 `lens_zoom`。
uniform float u_zoom;
// 升起程度 0..1：透镜效果整体按它淡入淡出。
uniform float u_lift;
// 选中色（主题 primary），透镜里的图标染成它。
uniform float u_accent_r;
uniform float u_accent_g;
uniform float u_accent_b;
// 1 = 浅色主题（图标比底色深），0 = 深色主题（图标比底色浅）。
uniform float u_light;
// 透镜外接框在屏幕上的位置与尺寸，物理像素。
uniform float u_lens_left;
uniform float u_lens_top;
uniform float u_lens_width;
uniform float u_lens_height;
uniform sampler2D u_backdrop;
out vec4 fragColor;

// 背板左上角到透镜左上角的偏移（见文件头「坐标」一节）。
vec2 lensOrigin() {
  vec2 lens = vec2(u_lens_width, u_lens_height);
  bool local = all(lessThan(abs(u_size - lens), vec2(4.0)));
  return local ? vec2(0.0) : vec2(u_lens_left, u_lens_top);
}

vec4 texel(vec2 g) {
  return texture(u_backdrop, clamp(g / u_size, vec2(0.0), vec2(1.0)));
}

// 按透镜本地坐标（像素）采背板，**手动双线性插值**。
//
// 背板纹理在真机上是按最近邻采样的：透镜放大 1.2 倍后，图标与文字的边
// 缘露出一级级像素台阶，长按时最明显（#266 所有者反馈「有锯齿」）。所以
// 取周围四个像素中心按距离加权。
vec4 sampleAt(vec2 p) {
  vec2 g = p + lensOrigin() - 0.5;
  vec2 i = floor(g);
  vec2 f = g - i;
  vec2 o = i + 0.5;
  vec4 a = mix(texel(o), texel(o + vec2(1.0, 0.0)), f.x);
  vec4 b = mix(texel(o + vec2(0.0, 1.0)), texel(o + vec2(1.0, 1.0)), f.x);
  return mix(a, b, f.y);
}

void main() {
  vec2 p = FlutterFragCoord().xy - lensOrigin();
  vec2 c = vec2(u_lens_width, u_lens_height) * 0.5;
  float r = c.y;
  // 胶囊 SDF 与指向外侧的法线（与 liquid_glass.frag 同式）。
  vec2 q = abs(p - c) - vec2(max(c.x - r, 0.0), 0.0);
  float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
  vec2 n = normalize(vec2(
    sign(p.x - c.x) * max(abs(p.x - c.x) - (c.x - r), 0.0),
    p.y - c.y
  ) + vec2(0.0001));

  float lift = clamp(u_lift, 0.0, 1.0);
  float zoom = max(u_zoom, 1.0);
  // 透镜体：中心平直放大；边缘一圈（约半径的 45%）像凸玻璃的边一样，
  // 把透镜**外侧**的内容压缩进来，越靠边越狠——录屏里透镜边上相邻 tab
  // 的图标被拉成弧形就是这个。
  //
  // 方向必须朝外（+n）。第一次上机用的是朝里（-n），结果边缘采到的是透
  // 镜中间的内容，图标上下各多出一个倒影、文字下面拖着一截重影（#266）。
  float edge = 1.0 - smoothstep(0.0, r * 0.45, -d);
  vec2 base = c + (p - c) / zoom;
  vec2 bend = n * edge * edge * r * 0.35 * lift;
  // 色散：三个通道沿法线错开一点，只在边缘带里出现。第一版是 0.92 / 1.10，轮廓
  // 上彩色噪点太重，收到 0.96 / 1.05。
  vec3 lensed = vec3(
    sampleAt(base + bend * 0.96).r,
    sampleAt(base + bend).g,
    sampleAt(base + bend * 1.05).b
  );

  // 染色：透镜里的「墨」（浅色主题下比底色深的像素，深色主题反过来）换
  // 成选中色。按亮度判断，底色本身不受影响。
  float lum = dot(lensed, vec3(0.299, 0.587, 0.114));
  float ink = u_light > 0.5
    ? 1.0 - smoothstep(0.35, 0.80, lum)
    : smoothstep(0.30, 0.70, lum);
  // 只染透镜内部：边缘带采的是透镜外侧被压进来的内容（底栏外的封面），
  // 那里的深色染上去就是一圈品红色的环——第二次上机在彩色封面上看到的
  // 正是这个（#266）。iOS 的边缘是白亮的，不带选中色。
  ink *= (1.0 - edge) * lift;
  vec3 accent = vec3(u_accent_r, u_accent_g, u_accent_b);
  lensed = mix(lensed, accent, ink * 0.9);

  // 高光：贴边一圈细亮线，上沿更亮；体内一层很淡的提亮。
  float rimLine = 1.0 - smoothstep(0.0, max(r / 14.0, 1.0), -d);
  float top = clamp(-n.y, 0.0, 1.0);
  lensed = mix(lensed, vec3(1.0), rimLine * (0.35 + 0.35 * top) + 0.04);

  // 不按升起程度做交叉淡化：放大倍数与边缘折射已经随它回到 1，画面自己
  // 会连续缩回原样。直接按 lift 混合的话，半升起时是「放大 1.1 倍」叠在
  // 「原样」上，文字出现双重影（#266 第三次上机看到）。只在最开始一小段
  // 淡入，避免高光边「啪」一下出现。
  vec4 original = sampleAt(p);
  // 轮廓内侧 1.5 像素渐变回原画面：外面那道裁剪是硬边，轮廓处折射又最
  // 强、采到的全是高频内容，不过渡的话长按时边上一圈是参差的彩色像素
  // （#266 第四次上机）。
  float rimFade = 1.0 - smoothstep(-1.5, 0.0, d);
  float amount = smoothstep(0.0, 0.2, lift) * rimFade;
  fragColor = vec4(mix(original.rgb, lensed, amount), original.a);
}
