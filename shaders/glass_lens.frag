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
// 已经有底栏与图标。坐标约定与 liquid_glass.frag 相同：FlutterFragCoord
// 与 u_size 都是这块被裁剪区域（透镜外接框）的背板像素。

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
uniform sampler2D u_backdrop;
out vec4 fragColor;

vec4 sampleAt(vec2 p) {
  return texture(u_backdrop, clamp(p / u_size, vec2(0.0), vec2(1.0)));
}

void main() {
  vec2 p = FlutterFragCoord().xy;
  vec2 c = u_size * 0.5;
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
  // 透镜体：中心平直放大，边缘一圈（约半径的 45%）像厚玻璃一样把采样
  // 点往里收，越靠边收得越狠——录屏里边缘的图标被拉成弧形就是这个。
  float edge = 1.0 - smoothstep(0.0, r * 0.45, -d);
  vec2 base = c + (p - c) / zoom;
  vec2 bend = -n * edge * edge * r * 0.55;
  // 色散：三个通道沿法线错开一点，只在边缘带里出现。
  vec3 lensed = vec3(
    sampleAt(base + bend * 0.92).r,
    sampleAt(base + bend).g,
    sampleAt(base + bend * 1.10).b
  );

  // 染色：透镜里的「墨」（浅色主题下比底色深的像素，深色主题反过来）换
  // 成选中色。按亮度判断，底色本身不受影响。
  float lum = dot(lensed, vec3(0.299, 0.587, 0.114));
  float ink = u_light > 0.5
    ? 1.0 - smoothstep(0.35, 0.80, lum)
    : smoothstep(0.30, 0.70, lum);
  vec3 accent = vec3(u_accent_r, u_accent_g, u_accent_b);
  lensed = mix(lensed, accent, ink * 0.9);

  // 高光：贴边一圈细亮线，上沿更亮；体内一层很淡的提亮。
  float rimLine = 1.0 - smoothstep(0.0, max(r / 14.0, 1.0), -d);
  float top = clamp(-n.y, 0.0, 1.0);
  lensed = mix(lensed, vec3(1.0), rimLine * (0.35 + 0.35 * top) + 0.04);

  // 整体按升起程度淡入：刚升起时和原画面几乎一样，不会「啪」一下出现。
  vec4 original = sampleAt(p);
  fragColor = vec4(mix(original.rgb, lensed, lift), original.a);
}
