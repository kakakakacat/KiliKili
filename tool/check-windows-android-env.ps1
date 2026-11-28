param(
    [Parameter(Mandatory = $true)][string]$TargetPlatform,
    [Parameter(Mandatory = $true)][string]$RepoPath
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $TargetPlatform -ne 'android') { exit 0 }

$longPaths = git -C $RepoPath config --get core.longpaths 2>$null
if ($LASTEXITCODE -ne 0 -or $longPaths -ne 'true') {
    [Console]::Error.WriteLine('Windows Android 构建前置检查失败：git core.longpaths 未开启。pub 的 git 依赖可能静默缺文件，随后 javac 报「找不到符号」。运行 git config --global core.longpaths true，再删除已损坏的 pub cache/git 检出并运行 flutter pub get。')
    exit 1
}

. (Join-Path $RepoPath 'lib/scripts/pub-cache.ps1')
$pubCachePath = (Resolve-PubCache).Path
$repoDrive = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($RepoPath))
$cacheDrive = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($pubCachePath))
if ($repoDrive -ne $cacheDrive) {
    [Console]::Error.WriteLine("Windows Android 构建警告：项目在 $repoDrive，pub cache 在 $cacheDrive。Kotlin 增量编译可能报 Could not close incremental caches（根因是 different roots）。可在本机 ~/.gradle/gradle.properties 设置 kotlin.incremental=false，或把 PUB_CACHE 移到项目所在盘。")
}

$gradleFile = Join-Path $RepoPath 'android/app/build.gradle.kts'
$ndkMatch = [regex]::Match((Get-Content -Raw $gradleFile), 'ndkVersion\s*=\s*"([^"]+)"')
if (-not $ndkMatch.Success) {
    [Console]::Error.WriteLine("Windows Android 构建前置检查失败：无法从 $gradleFile 读取 ndkVersion。")
    exit 1
}
$ndkVersion = $ndkMatch.Groups[1].Value

# Gradle 优先读 local.properties；其余依次采用 Android SDK 的环境变量与默认位置。
$sdkPath = $null
$localProperties = Join-Path $RepoPath 'android/local.properties'
if (Test-Path $localProperties) {
    $sdkLine = Get-Content $localProperties | Where-Object { $_ -match '^sdk\.dir=' } | Select-Object -First 1
    if ($sdkLine) {
        $sdkPath = ($sdkLine -replace '^sdk\.dir=', '' -replace '\\([\\:])', '$1')
    }
}
if (-not $sdkPath) { $sdkPath = $env:ANDROID_HOME }
if (-not $sdkPath) { $sdkPath = $env:ANDROID_SDK_ROOT }
if (-not $sdkPath -and $env:LOCALAPPDATA) { $sdkPath = Join-Path $env:LOCALAPPDATA 'Android/Sdk' }
if (-not $sdkPath) {
    [Console]::Error.WriteLine('Windows Android 构建前置检查失败：无法确定 Android SDK 位置；请设置 android/local.properties 的 sdk.dir 或 ANDROID_HOME。')
    exit 1
}
$packageXml = Join-Path $sdkPath "ndk/$ndkVersion/package.xml"
if (-not (Test-Path $packageXml -PathType Leaf)) {
    [Console]::Error.WriteLine("Windows Android 构建前置检查失败：缺少 $packageXml。手工解压安装 NDK 时还要补 package.xml；否则 Cargokit 可能只记 SEVERE，Gradle 仍成功产出缺少 libbridge_flutter.so 的 APK。请用 sdkmanager 安装该版本 NDK，或补齐与该版本匹配的 package.xml。")
    exit 1
}
