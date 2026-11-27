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
  // 不要在 GLES 上翻 uv.y。这里原本有一个 `#ifdef IMPELLER_TARGET_OPENGLES`
  // 的补偿，照搬「GLES 纹理原点在左下」的老经验盲写，而它在 #230 打开
  // Impeller 之前从未被执行过（iOS 走 Metal，Android 走 Skia）。真跑起来
  // 才发现 Flutter 3.47 的 Impeller 给到 ImageFilter.shader 的背板两条
  // 后端朝向一致，补偿反而把画面翻了过来：导航栏里映出屏幕对称位置的内容，
  // 看上去既倒置又不透明。见 #230。
  vec2 uv = (p + normal * rim * 7.0) / u_size;
  vec4 backdrop = texture(u_backdrop, clamp(uv, vec2(0.0), vec2(1.0)));
  float glint = (1.0 - smoothstep(0.0, 4.0, -distance)) * 0.18;
  fragColor = vec4(mix(backdrop.rgb, vec3(1.0), glint), backdrop.a);
}
