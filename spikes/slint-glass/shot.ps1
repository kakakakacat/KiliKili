# 桌面侧跑一轮并出一张图（issue #130）。
#
# **这个脚本不截屏。** 它只做两件事：挑一块空闲的屏幕、告诉程序开在那儿，
# 然后让程序用 Slint 的 take_snapshot 自己出图（`GLASS_SNAPSHOT`）。
#
# 为什么不截屏了——三条都是这台机器上实际撞到的：
#
# 1. **主屏有人在用。** 全屏游戏跑着的时候，GDI 的 CopyFromScreen 对**另一块
#    屏**也返回全黑；锁屏时窗口根本不渲染，同样是黑图（第二轮那张 wgpu 探针
#    截图就是这么丢的）。自己出图完全绕开这件事，连屏幕都不用看。
# 2. **抢焦点不可接受。** 原来用 WScript.Shell AppActivate 把窗口切到前台，
#    会把正在打游戏的人踢出来；而且前台被全屏程序占着时它**返回 True 但其实
#    没切过去**。现在窗口由程序自己用 `with_active(false)` 创建，从一开始就
#    不激活，见 src/lib.rs 的 place_window_without_stealing_focus()。
# 3. **DPI。** 缩放 1.5 时 GetWindowRect 与 CopyFromScreen 差 1.5 倍，截出来
#    是窗口旁边的桌面——而且脚本照常产出文件，看起来像成功了（第一轮栽过，
#    记在 docs/m0/VALIDATION-CAPABILITY.md）。自己出图没有这个坐标系问题。
#
# 用法（注意要用 `& .\shot.ps1`，`pwsh -File` 会把 -EnvVars 当字符串传）：
#   & .\shot.ps1 -Out shots/gl-p0.png -EnvVars @{GLASS_GL='1'; GLASS_COUNT='10'}

param(
    [Parameter(Mandatory = $true)][string]$Out,
    [hashtable]$EnvVars = @{},
    [string]$Exe = "target/debug/slint-glass-spike.exe",
    # 出图前让它跑多久。背景是动的，太短的话条带还没走开，几张图看着一样。
    [int]$SnapshotMs = 3000,
    [int]$TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32Place {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

# 必须在 AllScreens / GetWindowRect 之前，否则拿到的是逻辑坐标，而
# GLASS_WINDOW_POS 要的是物理像素。
[void][Win32Place]::SetProcessDPIAware()

# .NET 的「当前目录」和 PowerShell 的 Set-Location **不是一回事**：
# GetFullPath 用的是前者，而 Set-Location 只动后者。宿主进程的起始目录和
# 脚本里 cd 到的目录不一致时，相对路径会静默落到起始目录下（实测这样写出过
# shots/shots/*.png，而脚本照常打印「已写」）。手动同步一下。
[Environment]::CurrentDirectory = (Get-Location).Path
$outPath = [System.IO.Path]::GetFullPath($Out)
$outDir = [System.IO.Path]::GetDirectoryName($outPath)
if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }
$logPath = [System.IO.Path]::ChangeExtension($outPath, ".log")

# 前台窗口在哪块屏上，就避开那块——人正在用它。
$fg = [Win32Place]::GetForegroundWindow()
$busy = $null
if ($fg -ne [IntPtr]::Zero) {
    $fgr = New-Object Win32Place+RECT
    [void][Win32Place]::GetWindowRect($fg, [ref]$fgr)
    $center = New-Object System.Drawing.Point(
        [int](($fgr.Left + $fgr.Right) / 2), [int](($fgr.Top + $fgr.Bottom) / 2))
    $busy = [System.Windows.Forms.Screen]::FromPoint($center)
}
$screens = [System.Windows.Forms.Screen]::AllScreens
$target = $screens | Where-Object { $null -eq $busy -or $_.DeviceName -ne $busy.DeviceName } | Select-Object -First 1
if ($null -eq $target) {
    $target = if ($busy) { $busy } else { [System.Windows.Forms.Screen]::PrimaryScreen }
    Write-Output "注意：只有一块屏，窗口会开在有人用的那块上（但仍然不抢焦点）"
}
Write-Output "前台屏=$(if ($busy) { $busy.DeviceName } else { '无' })  实验台开在=$($target.DeviceName) $($target.Bounds)"

# 先把所有 GLASS_* 开关清掉。
#
# 不是洁癖：pwsh 同一个会话里 $env: 会留到下一次调用，于是连着跑几轮时
# 上一轮的档位会**静默带进下一轮**。2026-11-25 实测撞到：跑完
# GLASS_GL_MATERIAL=baseline 之后再跑 count=10，那一轮印出来的材质仍然是
# baseline——和第一轮 GLASS_COUNT=10 只画 6 条同一个形状的坑：
# 记录写的和实际跑的对不上，而两边都「看起来正常」。
#
# 画面上印着实际档位是第二道保险，这里是第一道。
Get-ChildItem Env: | Where-Object { $_.Name -like 'GLASS_*' } |
    ForEach-Object { Remove-Item "Env:$($_.Name)" }

$env:GLASS_WINDOW_POS = "$($target.Bounds.X + 40),$($target.Bounds.Y + 40)"
$env:GLASS_SNAPSHOT = $outPath
$env:GLASS_SNAPSHOT_MS = "$SnapshotMs"
Write-Output "env GLASS_WINDOW_POS=$($env:GLASS_WINDOW_POS)"
foreach ($k in $EnvVars.Keys) {
    Set-Item -Path "Env:$k" -Value $EnvVars[$k]
    Write-Output "env $k=$($EnvVars[$k])"
}

# 程序会在出完图之后自己退出（GLASS_SNAPSHOT 那条路 quit_event_loop）。
$proc = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $logPath
if (-not $proc.WaitForExit($TimeoutSeconds * 1000)) {
    Stop-Process -Id $proc.Id -Force
    throw "超过 $TimeoutSeconds 秒还没退出，已杀掉。看 $logPath"
}

Write-Output "---- 程序输出 ----"
if (Test-Path $logPath) { Get-Content $logPath }
