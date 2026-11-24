// SPIKE-004（issue #11）的平台胶水：Rust 那边通过 C ABI 调这里。
//
// 为什么用 Objective-C 写而不是在 Rust 里 msg_send：这几块全是 Apple 框架的
// 对象图（EAGLContext、CVPixelBuffer、AVPictureInPictureController 的 delegate、
// AVFoundation 的异步加载 block），用 ObjC 写一页能看完；在 Rust 里要手搓
// block、define_class、CMTime 的 Encode，篇幅翻几倍而且对结论没有帮助。
// 按 AGENTS.md §2，这些本来就是「平台 API 调用」，换宿主时留在宿主一侧。
//
// 三块：
//   1. 零拷贝目标：一张 IOSurface 支撑的 CVPixelBuffer，同时挂成 GLES 的 FBO
//      （给 mpv 画）和 Metal 纹理（给 Slint 的 wgpu 采样）。
//   2. PiP：AVSampleBufferDisplayLayer + AVPictureInPictureController(contentSource:)。
//   3. AVPlayer 对照：两路远程 DASH 分轨拼成 AVMutableComposition，
//      AVPlayerItemVideoOutput 吐出 CVPixelBuffer，走同一条 Metal 导入。

#import <AVFoundation/AVFoundation.h>
#import <AVKit/AVKit.h>
#import <CoreVideo/CoreVideo.h>
#import <Metal/Metal.h>
#import <OpenGLES/EAGL.h>
#import <OpenGLES/ES3/gl.h>
#import <OpenGLES/ES3/glext.h>
#import <UIKit/UIKit.h>
#include <dlfcn.h>

static void spike_log(NSString *s) { printf("[spike] %s\n", s.UTF8String); fflush(stdout); }

// ---------------------------------------------------------------------------
// 通用
// ---------------------------------------------------------------------------

/// UIApplication.applicationState：0 active，1 inactive，2 background。
int spike_app_state(void) { return (int)UIApplication.sharedApplication.applicationState; }

void spike_cf_release(void *ref) { if (ref) CFRelease(ref); }

/// 把 CVPixelBuffer（BGRA）包成 Slint 能吃的 MTLTexture。返回 +1 的 id<MTLTexture>；
/// `keep_out` 是要一直活到 GPU 用完为止的引用（+1，CFRelease 释放）。
///
/// Slint 的 `Image::try_from(wgpu::Texture)` 只收 Rgba8Unorm，而 CoreVideo / GLES
/// 这边只有 BGRA。办法是在同一块 IOSurface 上建一张 **RGBA8Unorm** 纹理
/// （按 RGBA 解读 BGRA 的字节，R/B 是反的），再在其上建一个 B↔R 互换的
/// swizzle 视图。视图的格式是真的 RGBA8Unorm，Skia 验格式也过得去；采样
/// 出来的颜色是对的；全程不拷贝像素。
///
/// 第一版用的是 CVMetalTextureCache 出 BGRA8Unorm、对 wgpu 谎称 Rgba8Unorm：
/// femtovg 渲染器不验格式所以能过，但 Slint 一开 `unstable-wgpu-30` 就改选
/// Skia 渲染器，`Image::from_texture` 发现格式对不上返回空，Slint 里一个
/// `unwrap()` 直接 abort（skia wgpu_30_surface/metal.rs:100）。
void *spike_mtl_texture(void *pixbuf, void *mtl_device, void **keep_out) {
    CVPixelBufferRef pb = pixbuf;
    IOSurfaceRef surface = CVPixelBufferGetIOSurface(pb);
    if (!surface) { spike_log(@"pixbuf 没有 IOSurface"); return NULL; }
    id<MTLDevice> device = (__bridge id<MTLDevice>)mtl_device;
    MTLTextureDescriptor *desc = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatRGBA8Unorm
                                                                                    width:CVPixelBufferGetWidth(pb)
                                                                                   height:CVPixelBufferGetHeight(pb)
                                                                                mipmapped:NO];
    desc.usage = MTLTextureUsageShaderRead | MTLTextureUsagePixelFormatView;
    id<MTLTexture> base = [device newTextureWithDescriptor:desc iosurface:surface plane:0];
    if (!base) { spike_log(@"newTextureWithDescriptor:iosurface: 失败"); return NULL; }
    id<MTLTexture> view = [base newTextureViewWithPixelFormat:MTLPixelFormatRGBA8Unorm
                                                  textureType:MTLTextureType2D
                                                       levels:NSMakeRange(0, 1)
                                                       slices:NSMakeRange(0, 1)
                                                      swizzle:MTLTextureSwizzleChannelsMake(MTLTextureSwizzleBlue,
                                                                                            MTLTextureSwizzleGreen,
                                                                                            MTLTextureSwizzleRed,
                                                                                            MTLTextureSwizzleAlpha)];
    if (!view) { spike_log(@"swizzle 视图失败"); return NULL; }
    // 留住 CVPixelBuffer 本身：AVPlayer 的缓冲来自池子，放回池子就会被下一帧覆盖。
    *keep_out = (void *)CFRetain(pb);
    return (__bridge_retained void *)view;
}

// ---------------------------------------------------------------------------
// 1. 零拷贝目标
// ---------------------------------------------------------------------------

static EAGLContext *g_gl;
static CVOpenGLESTextureCacheRef g_gl_cache;

/// 建一个 GLES3 上下文并设为当前。mpv 的 render context 与 ios-gl hwdec
/// 互操作都要求创建时上下文已是当前。
int spike_gl_init(void) {
    g_gl = [[EAGLContext alloc] initWithAPI:kEAGLRenderingAPIOpenGLES3];
    if (!g_gl || ![EAGLContext setCurrentContext:g_gl]) return -1;
    CVReturn r = CVOpenGLESTextureCacheCreate(NULL, NULL, g_gl, NULL, &g_gl_cache);
    if (r != kCVReturnSuccess) return -2;
    spike_log([NSString stringWithFormat:@"GL %s | %s", glGetString(GL_RENDERER), glGetString(GL_VERSION)]);
    return 0;
}

int spike_gl_make_current(void) { return [EAGLContext setCurrentContext:g_gl] ? 0 : -1; }

/// mpv 的 get_proc_address：OpenGLES.framework 已链进来，dlsym 就能找到。
void *spike_gl_proc(void *ctx, const char *name) { (void)ctx; return dlsym(RTLD_DEFAULT, name); }

void spike_gl_flush(int finish) { if (finish) glFinish(); else glFlush(); }

typedef struct {
    CVPixelBufferRef pixbuf;
    CVOpenGLESTextureRef gltex;
    GLuint fbo;
} SpikeTarget;

/// 建一张 w×h 的零拷贝目标，返回不透明指针；`fbo_out` 给 mpv。
void *spike_target_create(int w, int h, unsigned *fbo_out) {
    NSDictionary *attrs = @{
        (id)kCVPixelBufferIOSurfacePropertiesKey: @{},
        (id)kCVPixelBufferMetalCompatibilityKey: @YES,
        (id)kCVPixelBufferOpenGLESCompatibilityKey: @YES,
    };
    SpikeTarget *t = calloc(1, sizeof *t);
    CVReturn r = CVPixelBufferCreate(NULL, w, h, kCVPixelFormatType_32BGRA, (__bridge CFDictionaryRef)attrs, &t->pixbuf);
    if (r != kCVReturnSuccess) { spike_log([NSString stringWithFormat:@"CVPixelBufferCreate %d", r]); free(t); return NULL; }
    // 内部格式 RGBA、外部 BGRA：GLES 上 IOSurface 纹理的固定写法。GL 眼里它是
    // 普通 RGBA 纹理，内存里按 BGRA 存，Metal 那边用 BGRA8Unorm 看就是对的颜色。
    r = CVOpenGLESTextureCacheCreateTextureFromImage(NULL, g_gl_cache, t->pixbuf, NULL, GL_TEXTURE_2D, GL_RGBA, w, h,
                                                     GL_BGRA_EXT, GL_UNSIGNED_BYTE, 0, &t->gltex);
    if (r != kCVReturnSuccess) { spike_log([NSString stringWithFormat:@"CVOpenGLESTextureCacheCreateTextureFromImage %d", r]); return NULL; }
    GLuint name = CVOpenGLESTextureGetName(t->gltex);
    glBindTexture(GL_TEXTURE_2D, name);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glBindTexture(GL_TEXTURE_2D, 0);
    glGenFramebuffers(1, &t->fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, t->fbo);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, name, 0);
    GLenum status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    if (status != GL_FRAMEBUFFER_COMPLETE) { spike_log([NSString stringWithFormat:@"FBO status 0x%x", status]); return NULL; }
    *fbo_out = t->fbo;
    return t;
}

void *spike_target_pixbuf(void *target) { return ((SpikeTarget *)target)->pixbuf; }

// ---------------------------------------------------------------------------
// 2. PiP
// ---------------------------------------------------------------------------

typedef void (*SpikeSetPlaying)(int playing);
static SpikeSetPlaying g_set_playing;
static double g_duration = 0, g_position = 0;
static int g_paused = 0;

@interface SpikePipDelegate : NSObject <AVPictureInPictureSampleBufferPlaybackDelegate, AVPictureInPictureControllerDelegate>
@end

@implementation SpikePipDelegate
- (void)pictureInPictureController:(AVPictureInPictureController *)c setPlaying:(BOOL)playing {
    spike_log([NSString stringWithFormat:@"pip setPlaying %d", playing]);
    if (g_set_playing) g_set_playing(playing);
}
- (CMTimeRange)pictureInPictureControllerTimeRangeForPlayback:(AVPictureInPictureController *)c {
    if (g_duration <= 0) return CMTimeRangeMake(kCMTimeNegativeInfinity, kCMTimePositiveInfinity);
    // 时间轴以「现在」为基准往回推 position，这样 PiP 窗口的进度条和 mpv 一致。
    CMTime now = CMClockGetTime(CMClockGetHostTimeClock());
    CMTime start = CMTimeSubtract(now, CMTimeMakeWithSeconds(g_position, 1000));
    return CMTimeRangeMake(start, CMTimeMakeWithSeconds(g_duration, 1000));
}
- (BOOL)pictureInPictureControllerIsPlaybackPaused:(AVPictureInPictureController *)c { return g_paused; }
- (void)pictureInPictureController:(AVPictureInPictureController *)c didTransitionToRenderSize:(CMVideoDimensions)size {
    spike_log([NSString stringWithFormat:@"pip renderSize %dx%d", size.width, size.height]);
}
- (void)pictureInPictureController:(AVPictureInPictureController *)c skipByInterval:(CMTime)interval completionHandler:(void (^)(void))done {
    done();
}
- (void)pictureInPictureControllerWillStartPictureInPicture:(AVPictureInPictureController *)c { spike_log(@"pip willStart"); }
- (void)pictureInPictureControllerDidStartPictureInPicture:(AVPictureInPictureController *)c { spike_log(@"pip didStart"); }
- (void)pictureInPictureController:(AVPictureInPictureController *)c failedToStartPictureInPictureWithError:(NSError *)e {
    spike_log([NSString stringWithFormat:@"pip failedToStart %@", e]);
}
- (void)pictureInPictureControllerDidStopPictureInPicture:(AVPictureInPictureController *)c { spike_log(@"pip didStop"); }
- (void)observeValueForKeyPath:(NSString *)k ofObject:(id)o change:(NSDictionary *)ch context:(void *)ctx {
    spike_log([NSString stringWithFormat:@"pip %@ = %@", k, ch[NSKeyValueChangeNewKey]]);
}
@end

static AVSampleBufferDisplayLayer *g_layer;
static AVPictureInPictureController *g_pip;
static SpikePipDelegate *g_pip_delegate;

static UIView *spike_root_view(void) {
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if (![scene isKindOfClass:UIWindowScene.class]) continue;
        for (UIWindow *w in ((UIWindowScene *)scene).windows) {
            if (w.rootViewController.view) return w.rootViewController.view;
            return w;
        }
    }
    return nil;
}

/// 把显示层挂到窗口右下角（160×90 的缩略图：PiP 要求源层在可见的视图树里，
/// 顺便让人肉眼看到帧确实送进了这一层）。返回 0 成功。
int spike_pip_setup(SpikeSetPlaying set_playing) {
    g_set_playing = set_playing;
    UIView *root = spike_root_view();
    if (!root) return -1;
    if (!AVPictureInPictureController.isPictureInPictureSupported) return -2;
    g_layer = [AVSampleBufferDisplayLayer layer];
    g_layer.videoGravity = AVLayerVideoGravityResizeAspect;
    g_layer.backgroundColor = UIColor.darkGrayColor.CGColor;
    CGRect b = root.bounds;
    g_layer.frame = CGRectMake(b.size.width - 176, b.size.height - 140, 160, 90);
    [root.layer addSublayer:g_layer];
    g_pip_delegate = [SpikePipDelegate new];
    AVPictureInPictureControllerContentSource *src =
        [[AVPictureInPictureControllerContentSource alloc] initWithSampleBufferDisplayLayer:g_layer
                                                                           playbackDelegate:g_pip_delegate];
    g_pip = [[AVPictureInPictureController alloc] initWithContentSource:src];
    g_pip.delegate = g_pip_delegate;
    // 上滑回桌面时自动进 PiP——这是用户真正会走的路径，比按钮更重要。
    g_pip.canStartPictureInPictureAutomaticallyFromInline = YES;
    [g_pip addObserver:g_pip_delegate forKeyPath:@"pictureInPicturePossible" options:NSKeyValueObservingOptionNew context:NULL];
    return 0;
}

void spike_pip_timeline(double position, double duration, int paused) {
    BOOL changed = g_paused != paused || fabs(g_duration - duration) > 0.5;
    g_position = position; g_duration = duration; g_paused = paused;
    if (changed && g_pip) [g_pip invalidatePlaybackState];
}

int spike_pip_active(void) { return g_pip.isPictureInPictureActive; }
int spike_pip_possible(void) { return g_pip.isPictureInPicturePossible; }
void spike_pip_toggle(void) {
    if (g_pip.isPictureInPictureActive) [g_pip stopPictureInPicture]; else [g_pip startPictureInPicture];
}
/// 层的状态：0 unknown，1 rendering，2 failed。失败时打印错误。
int spike_pip_layer_status(void) {
    if (g_layer.status == AVQueuedSampleBufferRenderingStatusFailed) {
        spike_log([NSString stringWithFormat:@"display layer failed: %@", g_layer.error]);
        [g_layer flush];
    }
    return (int)g_layer.status;
}

/// 把一帧 CVPixelBuffer 包成 CMSampleBuffer 送进显示层，立即显示。
void spike_pip_enqueue(void *pixbuf) {
    if (!g_layer || !g_layer.readyForMoreMediaData) return;
    CVPixelBufferRef pb = pixbuf;
    CMVideoFormatDescriptionRef fmt = NULL;
    if (CMVideoFormatDescriptionCreateForImageBuffer(NULL, pb, &fmt) != noErr) return;
    CMSampleTimingInfo timing = {kCMTimeInvalid, CMClockGetTime(CMClockGetHostTimeClock()), kCMTimeInvalid};
    CMSampleBufferRef sb = NULL;
    CMSampleBufferCreateReadyWithImageBuffer(NULL, pb, fmt, &timing, &sb);
    CFRelease(fmt);
    if (!sb) return;
    CFArrayRef atts = CMSampleBufferGetSampleAttachmentsArray(sb, YES);
    CFMutableDictionaryRef d = (CFMutableDictionaryRef)CFArrayGetValueAtIndex(atts, 0);
    CFDictionarySetValue(d, kCMSampleAttachmentKey_DisplayImmediately, kCFBooleanTrue);
    [g_layer enqueueSampleBuffer:sb];
    CFRelease(sb);
}

// ---------------------------------------------------------------------------
// 3. AVPlayer 对照
// ---------------------------------------------------------------------------

static AVPlayer *g_player;
static AVPlayerItemVideoOutput *g_output;
static AVPlayer *g_audio_player;
static char g_avp_status[512] = "idle";

static void avp_status(NSString *s) {
    strlcpy(g_avp_status, s.UTF8String, sizeof g_avp_status);
    spike_log([@"avplayer " stringByAppendingString:s]);
}

const char *spike_avp_status(void) { return g_avp_status; }

/// 轮询两个 item 的状态，打出各自到 readyToPlay 的耗时（起播延迟）。
static void avp_watch_ready(CFAbsoluteTime t0) {
    static int vdone, adone;
    AVPlayerItem *v = g_player.currentItem, *a = g_audio_player.currentItem;
    double ms = (CFAbsoluteTimeGetCurrent() - t0) * 1000;
    if (!vdone && v.status != AVPlayerItemStatusUnknown) {
        vdone = 1;
        avp_status([NSString stringWithFormat:@"video item status=%ld after %.0f ms %@", (long)v.status, ms, v.error ?: @""]);
    }
    if (!adone && a && a.status != AVPlayerItemStatusUnknown) {
        adone = 1;
        spike_log([NSString stringWithFormat:@"avplayer audio item status=%ld after %.0f ms %@", (long)a.status, ms, a.error ?: @""]);
    }
    if ((!vdone || (a && !adone)) && ms < 60000)
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 100 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{ avp_watch_ready(t0); });
}

// ---- 3b. DASH → HLS（不转码、不代理）----
//
// B 站的每路 m4s 开头都是 ftyp + moov + sidx，sidx 里有每个分片（moof+mdat）的
// 字节长度和时长；playurl 响应的 segment_base 也直接给了 index_range。于是只要
// 读开头十几 KB，就能写出一份 fMP4 的 HLS 媒体播放列表：
//   EXT-X-MAP 指向 init 段的字节范围，每个分片一条 EXT-X-BYTERANGE，
//   分片地址就是原始的 https 地址，AVPlayer 自己发 Range 请求去 CDN 拿。
// 播放列表本身由 AVAssetResourceLoader 以自定义 scheme 提供（不起本地 HTTP 服务）。
// 视频、音频两份媒体列表 + 一份 master（EXT-X-MEDIA 音频组），一个 AVPlayer 放完，
// 音画同步交给 AVPlayer。

static NSString *hls_from_sidx(NSData *head, NSString *url, double *dur_out) {
    const uint8_t *b = head.bytes;
    NSUInteger n = head.length, o = 0;
    #define BE32(p) ((uint32_t)(p)[0] << 24 | (uint32_t)(p)[1] << 16 | (uint32_t)(p)[2] << 8 | (p)[3])
    while (o + 8 <= n) {
        uint32_t size = BE32(b + o);
        if (memcmp(b + o + 4, "sidx", 4) == 0) {
            if (o + size > n) return nil;  // 调用方读得不够
            const uint8_t *p = b + o + 8;
            int version = p[0];
            p += 4;
            uint32_t timescale = BE32(p + 4);
            p += 8;
            uint64_t first_offset;
            if (version == 0) { first_offset = BE32(p + 4); p += 8; }
            else { first_offset = (uint64_t)BE32(p + 8) << 32 | BE32(p + 12); p += 16; }
            uint16_t count = (uint16_t)(p[2] << 8 | p[3]);
            p += 4;
            uint64_t offset = o + size + first_offset;
            NSMutableString *segs = [NSMutableString string];
            double maxd = 0, total = 0;
            for (int i = 0; i < count; i++, p += 12) {
                uint32_t len = BE32(p) & 0x7fffffff;
                double d = (double)BE32(p + 4) / timescale;
                maxd = fmax(maxd, d); total += d;
                [segs appendFormat:@"#EXTINF:%.3f,\n#EXT-X-BYTERANGE:%u@%llu\n%@\n", d, len, offset, url];
                offset += len;
            }
            *dur_out = total;
            return [NSString stringWithFormat:@"#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:%d\n#EXT-X-PLAYLIST-TYPE:VOD\n"
                                              @"#EXT-X-MAP:URI=\"%@\",BYTERANGE=\"%lu@0\"\n%@#EXT-X-ENDLIST\n",
                                              (int)ceil(maxd), url, (unsigned long)o, segs];
        }
        if (size < 8) break;
        o += size;
    }
    return nil;
}

@interface SpikeHlsLoader : NSObject <AVAssetResourceLoaderDelegate>
@property NSMutableDictionary<NSString *, NSData *> *playlists;
@end
@implementation SpikeHlsLoader
- (BOOL)resourceLoader:(AVAssetResourceLoader *)loader shouldWaitForLoadingOfRequestedResource:(AVAssetResourceLoadingRequest *)req {
    NSData *d = self.playlists[req.request.URL.host];
    if (!d) return NO;
    [req.dataRequest respondWithData:d];
    [req finishLoading];
    return YES;
}
@end

static SpikeHlsLoader *g_hls_loader;

/// 读一路 m4s 的开头（Range 请求），解析 sidx 得到媒体播放列表。
static void hls_fetch_head(NSString *url, NSDictionary *headers, void (^done)(NSString *, double)) {
    NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:url]];
    [headers enumerateKeysAndObjectsUsingBlock:^(id k, id v, BOOL *stop) { [r setValue:v forHTTPHeaderField:k]; }];
    // 32KB 够装下 ftyp + moov + sidx（实测 sidx 约 14KB，对应约一千个分片）。
    [r setValue:@"bytes=0-32767" forHTTPHeaderField:@"Range"];
    [[NSURLSession.sharedSession dataTaskWithRequest:r completionHandler:^(NSData *data, NSURLResponse *resp, NSError *e) {
        double dur = 0;
        NSString *pl = data ? hls_from_sidx(data, url, &dur) : nil;
        dispatch_async(dispatch_get_main_queue(), ^{ done(pl, dur); });
    }] resume];
}

static int avp_start_hls(NSString *video, NSString *audio, NSDictionary *headers, NSDictionary *pbattrs, CFAbsoluteTime t0) {
    g_hls_loader = [SpikeHlsLoader new];
    g_hls_loader.playlists = [NSMutableDictionary dictionary];
    __block int pending = audio ? 2 : 1;
    void (^ready)(void) = ^{
        if (--pending > 0) return;
        if (!g_hls_loader.playlists[@"video"]) { avp_status(@"hls: 视频 sidx 解析失败"); return; }
        const char *vc = getenv("SPIKE_VCODEC"), *ac = getenv("SPIKE_ACODEC");
        NSString *codecs = [NSString stringWithFormat:@"%s%s%s", vc ?: "avc1.64001F", audio ? "," : "", audio ? (ac ?: "mp4a.40.2") : ""];
        NSMutableString *master = [@"#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-INDEPENDENT-SEGMENTS\n" mutableCopy];
        if (audio) [master appendString:@"#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"main\",DEFAULT=YES,AUTOSELECT=YES,URI=\"kkhls://audio/a.m3u8\"\n"];
        [master appendFormat:@"#EXT-X-STREAM-INF:BANDWIDTH=2000000,CODECS=\"%@\"%@\nkkhls://video/v.m3u8\n", codecs, audio ? @",AUDIO=\"a\"" : @""];
        g_hls_loader.playlists[@"master"] = [master dataUsingEncoding:NSUTF8StringEncoding];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL URLWithString:@"kkhls://master/m.m3u8"]
                                                options:@{@"AVURLAssetHTTPHeaderFieldsKey": headers}];
        [asset.resourceLoader setDelegate:g_hls_loader queue:dispatch_get_main_queue()];
        AVPlayerItem *item = [AVPlayerItem playerItemWithAsset:asset];
        g_output = [[AVPlayerItemVideoOutput alloc] initWithPixelBufferAttributes:pbattrs];
        [item addOutput:g_output];
        g_player = [AVPlayer playerWithPlayerItem:item];
        [g_player play];
        avp_status([NSString stringWithFormat:@"hls playlists ready in %.0f ms", (CFAbsoluteTimeGetCurrent() - t0) * 1000]);
        avp_watch_ready(t0);
    };
    hls_fetch_head(video, headers, ^(NSString *pl, double dur) {
        if (pl) g_hls_loader.playlists[@"video"] = [pl dataUsingEncoding:NSUTF8StringEncoding];
        spike_log([NSString stringWithFormat:@"hls video playlist: %lu bytes, %.0fs", (unsigned long)pl.length, dur]);
        ready();
    });
    if (audio) hls_fetch_head(audio, headers, ^(NSString *pl, double dur) {
        if (pl) g_hls_loader.playlists[@"audio"] = [pl dataUsingEncoding:NSUTF8StringEncoding];
        spike_log([NSString stringWithFormat:@"hls audio playlist: %lu bytes, %.0fs", (unsigned long)pl.length, dur]);
        ready();
    });
    return 0;
}

/// 两路远程 m4s（fMP4）拼成一个 composition 播放。B 站要 Referer，
/// AVURLAsset 没有公开的设头参数，只能用社区通用的私有键
/// AVURLAssetHTTPHeaderFieldsKey——这本身就是结论的一部分（上架有风险）。
int spike_avp_start(const char *video, const char *audio, const char *ua, const char *referer) {
    NSMutableDictionary *headers = [NSMutableDictionary dictionary];
    if (ua && *ua) headers[@"User-Agent"] = @(ua);
    if (referer && *referer) headers[@"Referer"] = @(referer);
    // B 站 CDN 对 .m4s 回 `Content-Type: application/octet-stream`，内容其实是标准
    // fMP4（ftyp iso5 … dash）。AVFoundation 靠扩展名 + MIME 判类型，两样都认
    // 不出来就报 -11828「This media format is not supported」。iOS 17 起有公开的
    // AVURLAssetOverrideMIMETypeKey 可以强行指定。
    NSMutableDictionary *vopts = [@{@"AVURLAssetHTTPHeaderFieldsKey": headers} mutableCopy];
    NSMutableDictionary *aopts = [vopts mutableCopy];
    if (@available(iOS 17.0, *)) if (getenv("SPIKE_AVP_NO_MIME") == NULL) {
        vopts[AVURLAssetOverrideMIMETypeKey] = @"video/mp4";
        aopts[AVURLAssetOverrideMIMETypeKey] = @"audio/mp4";
    }
    AVURLAsset *va = [AVURLAsset URLAssetWithURL:[NSURL URLWithString:@(video)] options:vopts];
    AVURLAsset *aa = audio && *audio ? [AVURLAsset URLAssetWithURL:[NSURL URLWithString:@(audio)] options:aopts] : nil;
    // mpv 自己会把会话设成 Playback；AVPlayer 不会，默认的 SoloAmbient 下
    // isPictureInPicturePossible 恒为 NO、后台也不出声。
    NSError *se = nil;
    [AVAudioSession.sharedInstance setCategory:AVAudioSessionCategoryPlayback mode:AVAudioSessionModeMoviePlayback options:0 error:&se];
    [AVAudioSession.sharedInstance setActive:YES error:&se];
    if (se) spike_log([NSString stringWithFormat:@"audio session: %@", se]);
    CFAbsoluteTime t0 = CFAbsoluteTimeGetCurrent();
    NSDictionary *pbattrs = @{
        (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA),
        (id)kCVPixelBufferMetalCompatibilityKey: @YES,
        (id)kCVPixelBufferIOSurfacePropertiesKey: @{},
    };
    const char *variant = getenv("SPIKE_AVP") ?: "hls";
    if (strcmp(variant, "hls") == 0)
        return avp_start_hls(@(video), aa ? @(audio) : nil, headers, pbattrs, t0);
    if (strcmp(variant, "two") == 0) {
        // SPIKE_AVP=two：不拼 composition，视频一个 AVPlayer、音频另一个 AVPlayer。
        // composition 要先 loadTracks，而对这种 300MB 的 fMP4，loadTracks 实测
        // 半分钟以上都不返回（见 README）。两个播放器各自起播，音画同步要自己做
        // （setRate:time:atHostTime: 对齐），这里只看能不能放、开销多少。
        AVPlayerItem *item = [AVPlayerItem playerItemWithAsset:va];
        g_output = [[AVPlayerItemVideoOutput alloc] initWithPixelBufferAttributes:pbattrs];
        [item addOutput:g_output];
        g_player = [AVPlayer playerWithPlayerItem:item];
        g_player.muted = aa != nil;
        [g_player play];
        if (aa) { g_audio_player = [AVPlayer playerWithPlayerItem:[AVPlayerItem playerItemWithAsset:aa]]; [g_audio_player play]; }
        avp_status(@"two players (video + audio), waiting for readyToPlay");
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 100 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{ avp_watch_ready(t0); });
        return 0;
    }
    // SPIKE_AVP=composition：两路拼成一个 AVMutableComposition。
    avp_status(@"loading tracks");
    [va loadTracksWithMediaType:AVMediaTypeVideo completionHandler:^(NSArray<AVAssetTrack *> *vtracks, NSError *verr) {
        void (^build)(NSArray<AVAssetTrack *> *) = ^(NSArray<AVAssetTrack *> *atracks) {
            dispatch_async(dispatch_get_main_queue(), ^{
                if (verr || vtracks.count == 0) { avp_status([NSString stringWithFormat:@"video track failed: %@", verr]); return; }
                AVMutableComposition *comp = [AVMutableComposition composition];
                CMTime dur = vtracks[0].timeRange.duration;
                NSError *e = nil;
                AVMutableCompositionTrack *vt = [comp addMutableTrackWithMediaType:AVMediaTypeVideo preferredTrackID:kCMPersistentTrackID_Invalid];
                [vt insertTimeRange:vtracks[0].timeRange ofTrack:vtracks[0] atTime:kCMTimeZero error:&e];
                if (e) { avp_status([NSString stringWithFormat:@"insert video failed: %@", e]); return; }
                if (atracks.count) {
                    AVMutableCompositionTrack *at = [comp addMutableTrackWithMediaType:AVMediaTypeAudio preferredTrackID:kCMPersistentTrackID_Invalid];
                    CMTimeRange r = CMTimeRangeMake(kCMTimeZero, CMTimeMinimum(dur, atracks[0].timeRange.duration));
                    [at insertTimeRange:r ofTrack:atracks[0] atTime:kCMTimeZero error:&e];
                    if (e) { avp_status([NSString stringWithFormat:@"insert audio failed: %@", e]); return; }
                }
                AVPlayerItem *item = [AVPlayerItem playerItemWithAsset:comp];
                g_output = [[AVPlayerItemVideoOutput alloc] initWithPixelBufferAttributes:pbattrs];
                [item addOutput:g_output];
                g_player = [AVPlayer playerWithPlayerItem:item];
                [g_player play];
                avp_status([NSString stringWithFormat:@"playing composition (tracks ready in %.0f ms, audio=%lu, dur=%.1fs)",
                                                      (CFAbsoluteTimeGetCurrent() - t0) * 1000, (unsigned long)atracks.count,
                                                      CMTimeGetSeconds(dur)]);
            });
        };
        if (aa) [aa loadTracksWithMediaType:AVMediaTypeAudio completionHandler:^(NSArray<AVAssetTrack *> *a, NSError *aerr) {
            if (aerr) spike_log([NSString stringWithFormat:@"avplayer audio track failed: %@", aerr]);
            build(a ?: @[]);
        }];
        else build(@[]);
    }];
    return 0;
}

/// 有新帧就返回 +1 的 CVPixelBufferRef，否则 NULL。
void *spike_avp_copy_frame(void) {
    if (!g_output) return NULL;
    CMTime t = [g_output itemTimeForHostTime:CACurrentMediaTime()];
    if (![g_output hasNewPixelBufferForItemTime:t]) return NULL;
    return [g_output copyPixelBufferForItemTime:t itemTimeForDisplay:NULL];
}

/// 位置、时长、播放器状态、item 状态与错误，给 HUD 用。
void spike_avp_info(double *pos, double *dur, int *item_status) {
    AVPlayerItem *it = g_player.currentItem;
    *pos = it ? CMTimeGetSeconds(it.currentTime) : 0;
    *dur = it ? CMTimeGetSeconds(it.duration) : 0;
    *item_status = it ? (int)it.status : -1;
    static int reported;
    if (it.status == AVPlayerItemStatusFailed && !reported) {
        reported = 1;
        avp_status([NSString stringWithFormat:@"item failed: %@", it.error]);
    }
}

void spike_avp_set_playing(int playing) {
    if (playing) { [g_player play]; [g_audio_player play]; } else { [g_player pause]; [g_audio_player pause]; }
}

/// 音轨播放器相对视频播放器的位置差（秒），看两个播放器各自起播后漂多少。
double spike_avp_av_offset(void) {
    if (!g_audio_player) return 0;
    return CMTimeGetSeconds(g_audio_player.currentTime) - CMTimeGetSeconds(g_player.currentTime);
}

// ---------------------------------------------------------------------------
// 4. #219 插帧：显示刷新驱动、刷新率请求、温控
// ---------------------------------------------------------------------------
//
// #218 的 iOS 实验台用 8 ms 的 NSTimer 出帧：每秒算 120 张，而 Slint 在
// iPhone 上只以 60 fps 重绘（没开 ProMotion），一半白算，也不和 vsync 对齐。
// 这里改成 CADisplayLink：每次显示刷新回调一次，按插帧档位请求帧率
// （preferredFrameRateRange），回调里带上系统给的实际帧间隔。
// ProMotion 还要 Info.plist 的 CADisableMinimumFrameDurationOnPhone（见 project.yml）。

typedef void (*spike_tick_fn)(double timestamp, double target_timestamp);

@interface SpikeTicker : NSObject
@property(nonatomic) spike_tick_fn fn;
@property(nonatomic, strong) CADisplayLink *link;
@end

@implementation SpikeTicker
- (void)tick:(CADisplayLink *)link {
    if (self.fn) self.fn(link.timestamp, link.targetTimestamp);
}
@end

static SpikeTicker *g_ticker;

/// 在主 run loop 上起一个 CADisplayLink，每次刷新调 `fn(timestamp, targetTimestamp)`
/// （CACurrentMediaTime 时基，秒）。
void spike_displaylink_start(spike_tick_fn fn) {
    if (g_ticker) return;
    g_ticker = [SpikeTicker new];
    g_ticker.fn = fn;
    g_ticker.link = [CADisplayLink displayLinkWithTarget:g_ticker selector:@selector(tick:)];
    [g_ticker.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
}

/// 请求帧率。`preferred <= 0` 表示交还系统默认。
void spike_displaylink_rate(float preferred) {
    if (!g_ticker) return;
    if (preferred <= 0) {
        g_ticker.link.preferredFrameRateRange = CAFrameRateRangeDefault;
    } else {
        // 上下限都钉在 preferred。第一版给的是 (preferred/2, preferred)，iPhone 16 Pro Max
        // 上系统时不时挑 40 Hz：刷新率在 40/60 之间来回跳，回退控制器把 40 Hz 的
        // 间隔当成掉帧，几秒后就降到了直通。
        g_ticker.link.preferredFrameRateRange = CAFrameRateRangeMake(preferred, preferred, preferred);
    }
    spike_log([NSString stringWithFormat:@"displaylink preferredFrameRateRange → %.0f", preferred]);
}

/// 屏幕的最高刷新率（ProMotion 机型 120；Info.plist 没开 ProMotion 时系统也报 120，
/// 但 display link 会被限在 60——以回调里的实际间隔为准）。
int spike_screen_max_fps(void) { return (int)UIScreen.mainScreen.maximumFramesPerSecond; }

/// NSProcessInfo.thermalState：0 nominal、1 fair、2 serious、3 critical。
int spike_thermal_state(void) { return (int)NSProcessInfo.processInfo.thermalState; }
