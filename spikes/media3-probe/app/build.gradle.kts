plugins {
    id("com.android.application")
}

android {
    namespace = "io.github.kakacat.kilikili.media3probe"
    // 刻意用 36 而不是主应用那个 37：Google 仓库里 37 只有带小版本的
    // （android-37.0 / 37.1 / 37.2），而 AGP 9.1 找的是平版 `android-37`，
    // 本机踩过（见 #184 的记录）。对照实验不需要最新平台，少一个变量。
    compileSdk = 36

    defaultConfig {
        applicationId = "io.github.kakacat.kilikili.media3probe"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.0"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // 对照包不对外分发，用调试签名就够（跟 slint-mpv 那个 spike 一致）。
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

dependencies {
    // 只要 exoplayer 本体：对照要量的是**解码与合成**，不是 Media3 的 UI 组件。
    // 版本钉死，换版本要当成一次有记录的改动。
    implementation("androidx.media3:media3-exoplayer:1.9.0")
}
