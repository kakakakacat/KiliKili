// SPIKE-002（issue #9）的 Media3 回退对照。自成一个 Gradle 构建，**不挂进
// 仓库根**：它跟 `android/` 那个 Flutter 应用没有任何共享代码，挂进去只会让
// 每次 `flutter build` 多编一个用不上的模块。
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}

plugins {
    // 版本跟 `android/settings.gradle.kts` 对齐：同一台机器上两套 AGP 会各自
    // 拉一份 Gradle 分发与依赖缓存，白等几分钟。
    id("com.android.application") version "9.1.0" apply false
}

rootProject.name = "media3-probe"

include(":app")
