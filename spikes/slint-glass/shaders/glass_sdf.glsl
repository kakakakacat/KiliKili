// 形状、法线与融合（issue #130 Phase 2「SDF/法线」与「组件融合」）。
//
// 被 `glass_overlay.frag` 拼在前面使用，版本声明由 `src/gl_probe.rs` 统一加。
// 单独成文件是因为它**不含任何材质或采样**：纯几何，换宿主、换后端都不用改。
//
// # 三种形状是同一个 SDF
//
// #130 要求首批支持圆角矩形、圆形和胶囊。它们不需要三段代码——圆角取到
// 半高就是胶囊，宽高相等且圆角取到半边就是圆。分成三个函数只会分出三处
// 要各自同步的近似误差。
//
// 这同时解释了 Phase 1 那版「只支持胶囊」的式子其实早就是通用式：把
// Flutter 的 `liquid_glass.frag` 展开之后，它正是 r = h/2 的 sd_round_box，
// 逐项对照见下面两个函数的注释。**所以这次推广没有动 baseline 档的像素**，
// 这一点很要紧：baseline 是「像不像 Flutter」唯一的参照物，它一动，之前
// 三轮的对照全部作废。

/// 到圆角矩形边界的有符号距离。内部为负，外部为正。
///
/// `p` 相对形状中心，`b` 是半宽半高，`r` 是圆角，单位都是逻辑像素。
///
/// 与 Flutter `liquid_glass.frag` 的等价性：那边是
/// `q = abs(p - center) - vec2(max(center.x - radius, 0.0), 0.0)`，
/// 代进 r = b.y（胶囊）之后本式的 `q = (|dx| - b.x + b.y, |dy| - b.y + b.y)`
/// 与它逐项相同。差别只有 `min(r, min(b.x, b.y))` 这一道夹取——那是防止
/// 圆角填大于半边时形状外翻，Flutter 那边用 `max(..., 0.0)` 只挡住了 x 一侧。
float sd_round_box(vec2 p, vec2 b, float r) {
    r = min(r, min(b.x, b.y));
    vec2 q = abs(p) - b + r;
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

/// 该点处的表面法线，指向形状外侧。也就是 `sd_round_box` 的梯度。
///
/// **不用中心差分**：解析梯度在这里是闭式的，差分要多 4 次距离计算，而且
/// 步长还得跟着 scale 调。
///
/// 同样代进 r = b.y 可以验证它退化成 Flutter 那一行：
/// `normalize(vec2(sign(dx) * max(|dx| - (b.x - b.y), 0.0), dy) + 1e-4)`。
/// 末尾那个 `+1e-4` 就是从那边抄的，防的是正中心两个分量同时为零；
/// baseline 档要逐像素对得上，所以它保留原样，**不要"顺手清理"掉**。
vec2 sd_round_box_normal(vec2 p, vec2 b, float r) {
    r = min(r, min(b.x, b.y));
    vec2 q = abs(p) - b + r;
    vec2 g = max(q, vec2(0.0));
    // 两个分量都落在圆角核心里（q 全负）时梯度退化成轴向。这一带
    // rim ≈ 0、法线乘上去是零，取哪个轴都看不出来；写出来只为避开
    // normalize(vec2(0.0))。
    if (g.x <= 0.0 && g.y <= 0.0) {
        g = q.x > q.y ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    }
    return normalize(sign(p) * g + vec2(1e-4));
}

/// 两个形状的平滑并集（#130 P1「靠近时的玻璃融合」）。
///
/// `k` 是融合半径，单位与距离一致（逻辑像素）；`k <= 0` 退回硬并集。
/// 两个形状相距小于 k 时之间会长出一段颈部，这正是 ADR-016 决策二那条
/// 「相距小于 3σ 会互相采到」的正面用法。
///
/// `h` 一并返回：法线必须用**同一个权重**去混，否则颈部的法线会在两个
/// 形状之间跳变，折射方向跟着跳，画面上是一条闪烁的硬缝。
float sd_smooth_union(float d1, float d2, float k, out float h) {
    if (k <= 0.0) {
        h = d1 < d2 ? 1.0 : 0.0;
        return min(d1, d2);
    }
    h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}
