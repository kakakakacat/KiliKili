package io.github.kilikili.playerportprobe.kilikili

import android.app.PictureInPictureParams
import android.content.res.Configuration
import android.os.Build
import android.util.Rational
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

// 独立宿主只接 Android API；不复制播放器的业务或时序判断。
class MainActivity : FlutterActivity() {
    private lateinit var channel: MethodChannel
    override fun configureFlutterEngine(engine: FlutterEngine) {
        super.configureFlutterEngine(engine)
        channel = MethodChannel(engine.dartExecutor.binaryMessenger, "kilikili/player-port-probe")
        channel.setMethodCallHandler { call, result ->
            if (call.method == "enterPip" && Build.VERSION.SDK_INT >= 26) {
                result.success(enterPictureInPictureMode(PictureInPictureParams.Builder()
                    .setAspectRatio(Rational(16, 9)).build()))
            } else {
                result.notImplemented()
            }
        }
    }
    override fun onPictureInPictureModeChanged(active: Boolean, config: Configuration) {
        super.onPictureInPictureModeChanged(active, config)
        channel.invokeMethod("pipChanged", active)
    }
}
