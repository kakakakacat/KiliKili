//! 插帧着色器（#218 定型）。**只用 fragment shader**：macOS 只有 GL 4.1、iOS 只有
//! GLES 3.0，都没有 compute shader；同一份源码加不同的 `#version` 头就能在
//! 五个平台上跑。
//!
//! 约定：
//! - 全分辨率源帧 `RGBA8`，低分辨率亮度 `R8`（源的 1/`DOWN`），运动场 `RGBA16F`
//!   （每个 `BS`×`BS` 低分辨率块一个像素）。
//! - 运动场 `xy` 存的是**半向量** `h`（低分辨率像素）：中间帧上的点 `x` 在
//!   A 上对应 `x - h`、在 B 上对应 `x + h`，总位移 `v = 2h`。这是「对称块匹配」：
//!   向量定义在插值帧的网格上，所以补偿时没有空洞，也不需要前后向两次估计。
//! - `z` 是最优候选的平均绝对差（0..1，不含惩罚项），`w` 是零运动的平均绝对差；
//!   中值 pass 之后 `w` 改为「零运动下这个块变了没有」（0/1）。
//! - [`REDUCE`] 把整个运动场规约成 1×1 的全帧统计：输出 pass 用它判转场（不用
//!   回读），CPU 异步回读它判重复帧。

/// 低分辨率倍率（每个方向）。720p → 320×180。
pub const DOWN: i32 = 4;
/// 块大小（低分辨率像素）。
pub const BS: i32 = 8;

pub const VERTEX: &str = r"
out vec2 uv;
void main() {
    // 一个盖满屏幕的大三角形：顶点 (0,0) (2,0) (0,2)。
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
";

/// 全分辨率 RGBA → 1/4 分辨率亮度。4 次双线性采样盖住 4×4 个源像素。
pub const LUMA: &str = r"
uniform sampler2D src;
uniform vec2 texel;
in vec2 uv;
out vec4 o;
void main() {
    vec3 c = texture(src, uv + texel * vec2(-1.0, -1.0)).rgb
           + texture(src, uv + texel * vec2( 1.0, -1.0)).rgb
           + texture(src, uv + texel * vec2(-1.0,  1.0)).rgb
           + texture(src, uv + texel * vec2( 1.0,  1.0)).rgb;
    o = vec4(dot(c * 0.25, vec3(0.299, 0.587, 0.114)), 0.0, 0.0, 1.0);
}
";

/// 对称块匹配第一步：**每个（块，候选向量）一个 fragment**，只算一次 SAD。
///
/// 第一版是「每块一个 fragment、在里面循环全部候选」，720p 只有 920 个
/// fragment、每个串行两万多次取样，M4 上要 9 ms——GPU 大半闲着。拆成
/// (块数 × 候选数) 个 fragment 之后并行度高两个数量级，再用 [`ARGMIN`]
/// 每块挑最小。输出纹理尺寸 = (fw·D, fh·D)，D = 2R+1。
///
/// `R`、`D`、`STEP` 由 [`defines`] 在编译期给出：Adreno 610 上第一版用 uniform
/// 做循环边界、在循环里做整数除法，代价表一趟 15 ms；整数除法在移动 GPU 上
/// 是软件模拟的，常量化之后编译器能展开循环、把除法折掉。
pub const COST: &str = r"
uniform sampler2D A;
uniform sampler2D B;
uniform ivec2 lsize;
uniform float lambda;
out vec4 o;

float px(sampler2D s, ivec2 p) {
    return texelFetch(s, clamp(p, ivec2(0), lsize - 1), 0).r;
}

void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    ivec2 blk = c / D;
    ivec2 h = c - blk * D - ivec2(R);
    ivec2 base = blk * BS;
    float sum = 0.0;
    for (int y = 0; y < BS; y += STEP) {
        // STEP=2 时按棋盘格错开，四分之一的点也能盖住整块。
        int xoff = ((y / STEP) % 2) * (STEP - 1);
        for (int x = 0; x < BS; x += STEP) {
            ivec2 q = base + ivec2(x + xoff, y);
            sum += abs(px(A, q - h) - px(B, q + h));
        }
    }
    float raw = sum / float((BS / STEP) * (BS / STEP));
    o = vec4(raw + lambda * float(abs(h.x) + abs(h.y)), raw, 0.0, 1.0);
}
";

/// 对称块匹配第二步：每块在 D×D 个代价里挑最小，再做 `SUBPEL` 层亚像素细化
/// （1 = 半像素，2 = 再加四分之一像素）。细化也按 `STEP` 隔点取样。
pub const ARGMIN: &str = r"
uniform sampler2D A;
uniform sampler2D B;
uniform sampler2D C;
uniform ivec2 lsize;
uniform float lambda;
out vec4 o;

float cost_sub(ivec2 base, vec2 h) {
    vec2 inv = 1.0 / vec2(lsize);
    float c = 0.0;
    for (int y = 0; y < BS; y += STEP) {
        int xoff = ((y / STEP) % 2) * (STEP - 1);
        for (int x = 0; x < BS; x += STEP) {
            vec2 q = vec2(base + ivec2(x + xoff, y)) + 0.5;
            c += abs(texture(A, (q - h) * inv).r - texture(B, (q + h) * inv).r);
        }
    }
    return c / float((BS / STEP) * (BS / STEP));
}

void main() {
    ivec2 blk = ivec2(gl_FragCoord.xy);
    ivec2 origin = blk * D;
    vec2 zc = texelFetch(C, origin + ivec2(R), 0).xy;
    float zero = zc.y;
    float best = zc.x;
    float best_raw = zc.y;
    ivec2 bh = ivec2(0);
    for (int dy = 0; dy < D; dy++) {
        for (int dx = 0; dx < D; dx++) {
            vec2 c = texelFetch(C, origin + ivec2(dx, dy), 0).xy;
            if (c.x < best) { best = c.x; best_raw = c.y; bh = ivec2(dx, dy) - ivec2(R); }
        }
    }
    vec2 bv = vec2(bh);
    ivec2 base = blk * BS;
    float stepSize = 0.5;
    for (int level = 0; level < SUBPEL; level++) {
        vec2 center = bv;
        for (int dy = -1; dy <= 1; dy++) {
            for (int dx = -1; dx <= 1; dx++) {
                if (dx == 0 && dy == 0) continue;
                vec2 h = center + stepSize * vec2(float(dx), float(dy));
                float raw = cost_sub(base, h);
                float c = raw + lambda * (abs(h.x) + abs(h.y));
                if (c < best) { best = c; best_raw = raw; bv = h; }
            }
        }
        stepSize *= 0.5;
    }
    o = vec4(bv, best_raw, zero);
}
";

/// 编译期常量。
#[must_use]
pub fn defines(radius: i32, step: i32, subpel: i32) -> String {
    format!(
        "#define BS {BS}\n#define R {radius}\n#define D {}\n#define STEP {step}\n#define SUBPEL {subpel}\n",
        2 * radius + 1
    )
}

/// 运动场 3×3 分量中值，去掉孤立的错配块。z、w 取中心块原值。
pub const SMOOTH: &str = r"
uniform sampler2D F;
uniform ivec2 fsize;
uniform float dupBlock;
out vec4 o;

float med9(float v[9]) {
    // 9 个数的中值：找排名第 4（从 0 起）的那个。
    for (int i = 0; i < 9; i++) {
        int less = 0;
        int equal = 0;
        for (int j = 0; j < 9; j++) {
            if (v[j] < v[i]) less++;
            else if (v[j] == v[i]) equal++;
        }
        if (less <= 4 && less + equal > 4) return v[i];
    }
    return v[4];
}

void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    float xs[9];
    float ys[9];
    int k = 0;
    for (int dy = -1; dy <= 1; dy++) {
        for (int dx = -1; dx <= 1; dx++) {
            vec4 f = texelFetch(F, clamp(c + ivec2(dx, dy), ivec2(0), fsize - 1), 0);
            xs[k] = f.x;
            ys[k] = f.y;
            k++;
        }
    }
    vec4 mid = texelFetch(F, c, 0);
    // w 改存「这个块在零运动下变了没有」（0/1）：mip 到顶层就是变化块的占比。
    // 用全帧平均差判重复帧不行——慢镜头里大半画面不动，平均差被稀释到和
    // 重复帧一样小（BBB 实测 330 对里误判 307 对）。
    o = vec4(med9(xs), med9(ys), mid.z, mid.w > dupBlock ? 1.0 : 0.0);
}
";

/// 全帧统计：一个 fragment 扫完整个运动场，输出 (失配均值, 变化块占比)。
/// 720p 只有 40×23 个块，单个 fragment 循环一遍很便宜。
pub const REDUCE: &str = r"
uniform sampler2D F;
uniform ivec2 fsize;
out vec4 o;
void main() {
    vec2 sum = vec2(0.0);
    for (int y = 0; y < fsize.y; y++) {
        for (int x = 0; x < fsize.x; x++) {
            sum += texelFetch(F, ivec2(x, y), 0).zw;
        }
    }
    sum /= float(fsize.x * fsize.y);
    // RGBA8 里存两个 16 位定点数：(变化块占比 hi, lo, 失配均值×BEST_SCALE hi, lo)。
    vec2 v = floor(clamp(vec2(sum.y, sum.x * BEST_SCALE), 0.0, 1.0) * 65535.0 + 0.5);
    vec2 hi = floor(v / 256.0);
    vec2 lo = v - hi * 256.0;
    o = vec4(hi.x, lo.x, hi.y, lo.y) / 255.0;
}
";

/// 把 1×1 统计纹理经 transform feedback 写进 buffer，代替 `glReadPixels`。
///
/// Adreno 610 上 `glReadPixels` 进 PBO 哪怕等计算的 fence 到了再发，也会让
/// CPU 等当前整条命令流（回读段 CPU 中位数 3.5 ms、p95 17 ms）。transform
/// feedback 是 GPU 往 buffer 里写，不走帧缓冲回读；fence 到了再 map 不会阻塞。
/// GL 3.0+ / GLES 3.0 都有，五个平台通用。
pub const TF_VERTEX: &str = r"
uniform sampler2D S;
out vec4 v;
void main() {
    v = texelFetch(S, ivec2(0), 0);
    gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
}
";

/// transform feedback 那个程序的片元着色器（光栅化被关掉，不会执行；GLES
/// 链接要求有一个）。
pub const TF_FRAGMENT: &str = r"
out vec4 o;
void main() { o = vec4(0.0); }
";

/// 失配均值的放大倍数：它通常在 0..0.1，放大到 0..1 再存 16 位。
pub const BEST_SCALE: f32 = 8.0;

/// 解码 [`REDUCE`] 的输出（GLSL 侧，输出 pass 判转场用）。
pub const DECODE_BEST: &str = "float decode_best(vec4 s) { return (s.z * 255.0 * 256.0 + s.w * 255.0) / 65535.0 / BEST_SCALE; }\n";

/// 输出：按模式画进目标 FBO（已设好 letterbox 视口）。
///
/// - mode 0（hold）：一直显示 A。等价于「自己按 target_time 排程，不插帧」。
/// - mode 1（blend）：A、B 线性混合。
/// - mode 2（mc）：运动补偿；转场或局部失配时退回最近的源帧。
pub const OUTPUT: &str = r"
uniform sampler2D FA;
uniform sampler2D FB;
uniform sampler2D F;
uniform float p;
uniform int mode;
uniform vec2 lsize;
uniform sampler2D S;
uniform float cutThresh;
uniform float occLo;
uniform float occHi;
uniform int debug;
uniform int fallback;
in vec2 uv;
out vec4 o;
DECODE
void main() {
    vec4 a0 = texture(FA, uv);
    if (mode == 0) { o = vec4(a0.rgb, 1.0); return; }
    vec4 b0 = texture(FB, uv);
    if (mode == 1) { o = vec4(mix(a0.rgb, b0.rgb, p), 1.0); return; }

    // 回退：fallback 0 = 最近的源帧（不重影但局部顿一下），1 = 线性混合（不顿但重影）。
    vec4 near = fallback == 1 ? mix(a0, b0, p) : (p < 0.5 ? a0 : b0);
    float gbest = decode_best(texelFetch(S, ivec2(0), 0));
    if (gbest > cutThresh) { o = vec4(near.rgb, 1.0); return; }

    vec4 f = texture(F, uv);
    vec2 h = f.xy / lsize;
    vec3 a = texture(FA, uv - 2.0 * p * h).rgb;
    vec3 b = texture(FB, uv + 2.0 * (1.0 - p) * h).rgb;
    vec3 c = mix(a, b, p);
    float d = dot(abs(a - b), vec3(1.0 / 3.0));
    float w = smoothstep(occLo, occHi, max(d, f.z));
    if (debug == 1) {
        // 运动场可视化：红绿 = 方向/大小，蓝 = 回退权重。
        o = vec4(0.5 + f.xy / 16.0, w, 1.0);
        return;
    }
    o = vec4(mix(c, near.rgb, w), 1.0);
}
";

#[must_use]
pub fn header(es: bool) -> &'static str {
    if es {
        "#version 300 es\nprecision highp float;\nprecision highp int;\nprecision highp sampler2D;\n"
    } else {
        "#version 330 core\n"
    }
}
