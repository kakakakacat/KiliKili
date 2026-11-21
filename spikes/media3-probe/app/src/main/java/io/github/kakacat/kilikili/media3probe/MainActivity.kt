package io.github.kakacat.kilikili.media3probe

import android.app.Activity
import android.app.PictureInPictureParams
import android.graphics.PixelFormat
import android.opengl.GLES20
import android.opengl.GLSurfaceView
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.util.Rational
import android.view.Gravity
import android.view.SurfaceView
import android.view.View
import android.widget.Button
import android.widget.FrameLayout
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.source.MergingMediaSource
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import java.io.File
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10

/**
 * SPIKE-002（issue #9）里「Media3 SurfaceView/TextureView 回退对照」那一项。
 *
 * 它**不是**要证明 Media3 能放视频——那没有悬念。要量的是：如果 Slint + libmpv
 * 那条路走不通，回退到 Media3 之后，#9 关心的那几件事各自变成什么样子：
 *
 * 1. **UI 层还能不能压在视频上？** 这边的视频在它自己的 surface 上（SurfaceView
 *    是在窗口上挖洞、由 SurfaceFlinger 单独合成的），UI 不可能和它共用一个
 *    framebuffer。所以这里摆了两层：底下 SurfaceView 放视频，上面一层
 *    **透明的 GLSurfaceView**（`setZOrderMediaOverlay(true)`）画 UI，再上面
 *    是普通的 View（按钮）。三层叠得住，回退路线才成立。
 * 2. **硬解是不是零拷贝。** ExoPlayer 把 MediaCodec 直接配到 Surface 上输出，
 *    天然不经 CPU；`onVideoDecoderInitialized` 给出的解码器名字就是证据
 *    （`c2.qti.*` / `OMX.qcom.*` 是硬件，`c2.android.*` 是软件）。
 * 3. **后台音频 / 旋转 / PiP 各要写多少代码。** 这才是对照的真正目的：同一组
 *    问题，两条路各自的成本。结论写在 docs/m0/spikes/SPIKE-002-android.md。
 *
 * 日志标签 `kk-media3`，前缀跟 Slint 探针对齐（STATUS / TOUCH / PIP / EV），
 * 这样两边的日志能用同一套眼睛看。
 */
@UnstableApi
class MainActivity : Activity() {

    private var player: ExoPlayer? = null
    private lateinit var videoView: SurfaceView
    private lateinit var overlay: GLSurfaceView
    private val handler = Handler(Looper.getMainLooper())

    /** 命令通道：跟 Slint 探针用同一套文件协议，驱动脚本才能共用。 */
    private val cmdFile: File? by lazy { getExternalFilesDir(null)?.let { File(it, "cmd.txt") } }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Log.i(TAG, "EV onCreate")

        val root = FrameLayout(this)

        videoView = SurfaceView(this)
        root.addView(
            videoView,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )

        // 第二层：透明的 GL 层，代表「UI 层」。
        // setZOrderMediaOverlay(true) + TRANSLUCENT 是让它落在视频 surface 之上
        // 又不挡住它的**唯一**办法；少一个，要么整块黑、要么压在视频下面。
        overlay = GLSurfaceView(this).apply {
            setEGLContextClientVersion(2)
            setEGLConfigChooser(8, 8, 8, 8, 0, 0)
            holder.setFormat(PixelFormat.TRANSLUCENT)
            setZOrderMediaOverlay(true)
            setRenderer(OverlayRenderer())
            renderMode = GLSurfaceView.RENDERMODE_CONTINUOUSLY
        }
        root.addView(
            overlay,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )

        // 第三层：普通视图。SurfaceView 在场时，普通 View 到底还盖不盖得住，
        // 是回退路线能不能放控制条的关键，所以摆一个按钮当探针。
        val pipButton = Button(this).apply {
            text = "PiP"
            setOnClickListener {
                Log.i(TAG, "TOUCH click 目标=PiP 按钮")
                enterPip()
            }
        }
        root.addView(
            pipButton,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM or Gravity.START,
            ),
        )

        // 画面层自己的触摸：对照 Slint 那边「视频不吃触摸」那条结论。
        root.setOnTouchListener { _, event ->
            Log.i(TAG, "TOUCH ${event.actionMasked} x=${event.x.toInt()} y=${event.y.toInt()} 目标=画面层")
            true
        }

        setContentView(root)
        ensureCmdFile()
        startPlayback()
        pollCommands()
        logStatus()
    }

    private fun startPlayback() {
        val media = getExternalFilesDir(null)?.let { File(it, "probe.mp4") }
        if (media == null || !media.exists()) {
            Log.e(TAG, "没有 probe.mp4，对照只能验 UI 叠加那一半")
            return
        }
        val exo = ExoPlayer.Builder(this).build()
        exo.setVideoSurfaceView(videoView)
        // probe.mp4 自己没有音频轨，后台音频那一项要有声才测得出来。外挂一条
        // WAV，用 MergingMediaSource 合成——这跟 B 站的 DASH 分轨是同一个形状，
        // 也跟 Slint 那边走的 `MediaSource::Dash` 对得上。
        val audio = getExternalFilesDir(null)?.let { File(it, "probe-audio.wav") }
        if (audio != null && audio.exists()) {
            val factory = DefaultMediaSourceFactory(this)
            exo.setMediaSource(
                MergingMediaSource(
                    factory.createMediaSource(MediaItem.fromUri(media.toURI().toString())),
                    factory.createMediaSource(MediaItem.fromUri(audio.toURI().toString())),
                ),
            )
        } else {
            Log.e(TAG, "没有 probe-audio.wav：这一轮是默片，后台音频测不出东西")
            exo.setMediaItem(MediaItem.fromUri(media.toURI().toString()))
        }
        exo.repeatMode = Player.REPEAT_MODE_ALL
        exo.addListener(object : Player.Listener {
            // 没有这一条的话，失败只表现为 state 回到 IDLE，原因一个字都没有——
            // 第一轮就这么白跑了一次。
            override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                Log.e(TAG, "PLAYER-ERROR ${error.errorCodeName}: ${error.message}", error)
            }
        })
        exo.addAnalyticsListener(object : AnalyticsListener {
            override fun onVideoDecoderInitialized(
                eventTime: AnalyticsListener.EventTime,
                decoderName: String,
                initializedTimestampMs: Long,
                initializationDurationMs: Long,
            ) {
                // `c2.android.*` 是 Google 的软件解码器，`c2.qti.*` / `OMX.qcom.*`
                // 才是高通的硬件解码器——这一行就是「硬解有没有真开」的证据。
                Log.i(TAG, "DECODER $decoderName")
            }

            override fun onDroppedVideoFrames(
                eventTime: AnalyticsListener.EventTime,
                droppedFrames: Int,
                elapsedMs: Long,
            ) {
                Log.i(TAG, "DROP $droppedFrames 帧 / ${elapsedMs}ms")
            }
        })
        exo.prepare()
        exo.play()
        player = exo
    }

    /** 每秒一条，字段跟 Slint 探针的 STATUS 对齐。 */
    private fun logStatus() {
        handler.postDelayed({
            player?.let {
                Log.i(
                    TAG,
                    "STATUS t=${it.currentPosition / 1000.0} playing=${it.isPlaying} " +
                        "state=${it.playbackState} videoSize=${it.videoSize.width}x${it.videoSize.height}",
                )
            }
            logStatus()
        }, 1000)
    }

    /** 命令文件**必须由 app 自己建**：驱动脚本先建的话文件属 shell，app 读不到
     *  （第一轮就是这样，命令一条都没执行，日志里却什么异常都没有）。 */
    private fun ensureCmdFile() {
        val file = cmdFile ?: return
        if (!file.exists()) {
            runCatching { file.writeText("") }
                .onFailure { Log.e(TAG, "建不了 cmd.txt: $it") }
        }
    }

    private fun pollCommands() {
        handler.postDelayed({
            val file = cmdFile
            if (file != null && file.exists()) {
                val line = runCatching { file.readText().trim() }.getOrDefault("")
                if (line.isNotEmpty()) {
                    runCatching { file.writeText("") }
                    runCommand(line)
                }
            }
            pollCommands()
        }, 250)
    }

    private fun runCommand(line: String) {
        val parts = line.split(" ")
        when (parts[0]) {
            "note" -> Log.i(TAG, "CMD note ${parts.drop(1).joinToString(" ")}")
            "pip" -> enterPip()
            "pip-auto" -> setAutoEnter(parts.getOrNull(1) != "off")
            "quit" -> finish()
            else -> Log.i(TAG, "CMD 不认识 $line")
        }
        Log.i(TAG, "CMD done $line")
    }

    private fun enterPip() {
        val aspect = Rational(16, 9)
        val entered = enterPictureInPictureMode(
            PictureInPictureParams.Builder().setAspectRatio(aspect).build(),
        )
        Log.i(TAG, "PIP enter = $entered")
    }

    private fun setAutoEnter(on: Boolean) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
            Log.i(TAG, "PIP setAutoEnterEnabled 要 API 31")
            return
        }
        setPictureInPictureParams(
            PictureInPictureParams.Builder()
                .setAspectRatio(Rational(16, 9))
                .setAutoEnterEnabled(on)
                .build(),
        )
        Log.i(TAG, "PIP setAutoEnterEnabled($on)")
    }

    // Slint 那条路用的是系统的 NativeActivity，这两个回调**根本收不到**；
    // 这边是自己的 Activity，所以收得到。对照里这是一条实打实的差别。
    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode)
        Log.i(TAG, "PIP isInPictureInPictureMode=$isInPictureInPictureMode")
        // 小窗里放不下控制件，真实应用在这里把它们藏掉。
        overlay.visibility = if (isInPictureInPictureMode) View.GONE else View.VISIBLE
    }

    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        Log.i(TAG, "EV onUserLeaveHint")
    }

    override fun onPause() {
        super.onPause()
        Log.i(TAG, "EV onPause isInPip=$isInPictureInPictureMode")
        // **后台音频就靠这个判断**：进 PiP 时也会走 onPause，此时不能停；
        // 真正回到桌面时才按应用自己的策略决定停不停。这里一律不停，
        // 好让「Media3 后台还出不出声」这一问有个干净的观察。
    }

    override fun onStop() {
        super.onStop()
        Log.i(TAG, "EV onStop")
    }

    override fun onResume() {
        super.onResume()
        Log.i(TAG, "EV onResume")
    }

    override fun onDestroy() {
        super.onDestroy()
        Log.i(TAG, "EV onDestroy")
        handler.removeCallbacksAndMessages(null)
        player?.release()
        player = null
    }

    /**
     * UI 层的替身：每帧画一个半透明的横条，其余部分 alpha=0。
     *
     * 它存在的意义只有一个——**证明这一层确实压在视频之上，又没把视频盖住**。
     * 真实的 UI 会是 Slint（或别的）画在这同一个 GL 上下文里。
     */
    private class OverlayRenderer : GLSurfaceView.Renderer {
        private var frames = 0

        override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
            Log.i(TAG, "GL ${GLES20.glGetString(GLES20.GL_VENDOR)} | ${GLES20.glGetString(GLES20.GL_RENDERER)}")
        }

        override fun onSurfaceChanged(gl: GL10?, width: Int, height: Int) {
            Log.i(TAG, "SIZE 覆盖层 ${width}x$height")
            GLES20.glViewport(0, 0, width, height)
        }

        override fun onDrawFrame(gl: GL10?) {
            // 透明清屏：不透明的话视频就被这一层盖掉了，三层叠加当场失败。
            GLES20.glClearColor(0f, 0f, 0f, 0f)
            GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)
            // 用 scissor 画一条会动的半透明条，证明这一层在出帧。
            GLES20.glEnable(GLES20.GL_SCISSOR_TEST)
            val y = (frames * 4) % 1600
            GLES20.glScissor(0, y, 2000, 80)
            GLES20.glClearColor(0.1f, 0.8f, 0.4f, 0.55f)
            GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)
            GLES20.glDisable(GLES20.GL_SCISSOR_TEST)
            frames += 1
        }
    }

    companion object {
        private const val TAG = "kk-media3"
    }
}
