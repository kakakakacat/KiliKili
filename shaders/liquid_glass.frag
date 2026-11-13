#version 460 core
#include <flutter/runtime_effect.glsl>

// 尺寸和背板图像由 Flutter 的 ImageFilter 自动绑定。
uniform vec2 u_size;
uniform sampler2D u_backdrop;
out vec4 fragColor;

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
  vec2 uv = (p + normal * rim * 7.0) / u_size;
#ifdef IMPELLER_TARGET_OPENGLES
  uv.y = 1.0 - uv.y;
#endif
  vec4 backdrop = texture(u_backdrop, clamp(uv, vec2(0.0), vec2(1.0)));
  float glint = (1.0 - smoothstep(0.0, 4.0, -distance)) * 0.18;
  fragColor = vec4(mix(backdrop.rgb, vec3(1.0), glint), backdrop.a);
}
