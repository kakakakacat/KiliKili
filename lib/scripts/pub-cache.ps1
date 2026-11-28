# 解析本机 pub cache 的位置（#175）。
#
# 缓存在哪由**跑脚本的机器**决定，与要打给哪个平台无关。patch.ps1 原来把两者
# 混在一起：只有 `windows` 这一支才去读 %LOCALAPPDATA%，于是「在 Windows 上打
# Android 包」会按 Linux 的 ~/.pub-cache 去找，必然落空。prepare-build.sh 的
# 清理步骤同样写死 ~/.pub-cache，在 Windows 上是静默空操作。两边现在都调这里。
#
# 查找顺序与 Dart 官方一致：
#   1. $env:PUB_CACHE —— 官方的覆盖方式，优先级最高；
#   2. Windows：%LOCALAPPDATA%\Pub\Cache，其次旧版 Dart 用过的 %APPDATA%\Pub\Cache；
#   3. 其余平台：~/.pub-cache。
#
# 用法：
#   . "$PSScriptRoot/pub-cache.ps1"; $c = Resolve-PubCache; $c.Path; $c.Candidates
#   pwsh -NoProfile -File lib/scripts/pub-cache.ps1     # 直接打印路径，供 bash 取用

function Get-PubCacheCandidate {
    $candidates = @()

    if ($env:PUB_CACHE) {
        $candidates += $env:PUB_CACHE
    }

    $onWindows = $IsWindows -or ($env:OS -eq 'Windows_NT')
    if ($onWindows) {
        if ($env:LOCALAPPDATA) { $candidates += (Join-Path $env:LOCALAPPDATA 'Pub/Cache') }
        if ($env:APPDATA) { $candidates += (Join-Path $env:APPDATA 'Pub/Cache') }
    }

    $home_ = if ($env:HOME) { $env:HOME } elseif ($env:USERPROFILE) { $env:USERPROFILE } else { $null }
    if ($home_) { $candidates += (Join-Path $home_ '.pub-cache') }

    # 路径一律用正斜杠：Windows 自己认，传给 bash 侧时反斜杠会被当转义符吃掉。
    return $candidates | ForEach-Object { $_ -replace '\\', '/' } | Select-Object -Unique
}

function Resolve-PubCache {
    $candidates = @(Get-PubCacheCandidate)
    if ($candidates.Count -eq 0) {
        throw '无法判断 pub cache 位置：PUB_CACHE、LOCALAPPDATA、HOME、USERPROFILE 都是空的'
    }

    # PUB_CACHE 是用户的显式指定，dart 也只认它，不做存在性回退。
    if ($env:PUB_CACHE) {
        # $candidates[0] 就是它，已经统一成正斜杠。
        return [pscustomobject]@{ Path = $candidates[0]; Candidates = $candidates }
    }

    # 其余情况优先选真的装过包的那个；其次选存在的目录；都没有就用首选项，
    # 让后面的 `flutter pub get` 自己把它建出来。
    $chosen = $candidates | Where-Object { Test-Path (Join-Path $_ 'hosted/pub.dev') } | Select-Object -First 1
    if (-not $chosen) {
        $chosen = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    }
    if (-not $chosen) {
        $chosen = $candidates[0]
    }

    return [pscustomobject]@{
        Path       = $chosen
        Candidates = $candidates
    }
}

# 被 `. ` 点源时只定义函数；用 -File 直接跑时打印结果。
if ($MyInvocation.InvocationName -ne '.') {
    (Resolve-PubCache).Path
}
