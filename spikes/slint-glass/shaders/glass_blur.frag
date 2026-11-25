// 可分离高斯模糊的一趟（issue #130 P0「实时背景模糊」）。
//
// 用两趟（横 + 竖）而不是一趟二维卷积：半径 r 时采样数从 (2r+1)² 降到
// 2(2r+1)，σ=10 这一档差出一个数量级。这是 #130 正文「降低模糊分辨率、
// 减少 Pass」那条优化里最基本的一项。
//
// 另一半优化是**降采样**：宿主把目标纹理开成 1/4 边长，于是 σ 也跟着除以 4，
// 采样数再降四倍，而模糊本来就在丢高频，降采样丢的那部分看不出来。
// 代价是第一趟从全分辨率跨 4 个纹素取一次，会有一点走样——靠 LINEAR 过滤
// 盖掉，模糊之后同样看不出来。真要较真的话该先单独做一趟 box 降采样，
// 这个 spike 不值当。
//
// 注意这个 shader **不认识玻璃的形状**：它模糊的是整块矩形背板，胶囊形状
// 由 glass_overlay.frag 的 coverage 来裁。分开是对的——形状在哪一步裁，
// 决定了边缘会不会把矩形外的内容糊进来。

uniform sampler2D u_src;
// 一步的方向与长度，**归一化坐标**：横向是 (1/目标宽, 0)，竖向是 (0, 1/目标高)。
uniform vec2 u_direction;
// 本级分辨率下的标准差，像素。
uniform float u_sigma;
// 目标分辨率，用来把 gl_FragCoord 归一化。
uniform vec2 u_target_size;

out vec4 frag_color;

void main() {
    vec2 uv = gl_FragCoord.xy / u_target_size;

    // 3σ 之外的权重合计不到千分之三，截断看不出来；上限 24 是防呆——
    // σ 被填成个大数时不至于把一帧卡死。
    int radius = int(min(ceil(u_sigma * 3.0), 24.0));

    vec3 acc = vec3(0.0);
    float wsum = 0.0;
    for (int i = -radius; i <= radius; ++i) {
        float fi = float(i);
        float w = exp(-(fi * fi) / (2.0 * u_sigma * u_sigma));
        acc += texture(u_src, uv + u_direction * fi).rgb * w;
        wsum += w;
    }

    frag_color = vec4(acc / wsum, 1.0);
}
