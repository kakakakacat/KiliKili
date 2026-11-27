#version 460 core
#include <flutter/runtime_effect.glsl>

// 按住底栏时水滴里的放大透镜（#266）：把水滴下方已经画好的底栏与图标、
// 文字一起放大，就像 iOS 按住 tab bar 时那块透镜。
//
// 它是叠在导航内容**之上**的第二个 BackdropFilter，裁成水滴的形状。主
// 着色器（liquid_glass.frag）做不到这件事：它在导航内容之前执行，背板里
// 还没有图标。只在按压程度大于 0 时挂上，平时不付这一次背板采样。
//
// 坐标约定与 liquid_glass.frag 相同：FlutterFragCoord 与 u_size 都是这块
// 被裁剪区域（即水滴外接框）的背板像素。

uniform vec2 u_size;
// 放大倍数，`bili_core::glass_droplet` 的 `lens_zoom`。只用标量 float，
// 理由见 liquid_glass.frag 顶部（std140 对齐）。
uniform float u_zoom;
uniform sampler2D u_backdrop;
out vec4 fragColor;

void main() {
  vec2 p = FlutterFragCoord().xy;
  vec2 c = u_size * 0.5;
  vec2 b = c;
  float r = b.y;
  // 胶囊 SDF：越靠边，透镜越像一圈厚玻璃，把采样点往里收，边缘的放大
  // 更强，中心保持平直放大，字不会被扭成弧形。
  vec2 q = abs(p - c) - b + r;
  float d = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
  float edge = 1.0 - smoothstep(0.0, r * 0.6, -d);
  float base = max(u_zoom, 1.0);
  float zoom = base + (base - 1.0) * 0.45 * edge * edge;
  vec2 sp = c + (p - c) / zoom;
  vec4 color = texture(u_backdrop, clamp(sp / u_size, vec2(0.0), vec2(1.0)));
  // 贴边一圈细亮线，和主着色器里水滴的边缘高光对上。
  float rimLine = 1.0 - smoothstep(0.0, max(r / 12.0, 1.0), -d);
  color.rgb = mix(color.rgb, vec3(1.0), rimLine * 0.25);
  fragColor = color;
}
