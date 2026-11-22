# 从外面驱动实验台：切焦点、按键、切中文输入法、最小化/还原。
#
# 为什么要有它：IME 组字、Tab 轮转、最小化后再回来这几项，**只能从窗口
# 外面发真正的输入事件**才测得到。程序自己改属性等于绕过被测的那一段。
# 这条路子在 SPIKE-002 的 android-drive.sh 里已经用过一次，这里是
# Windows 版。
#
# 用 Windows PowerShell（powershell.exe）跑。
#
# 这个文件**存成带 BOM 的 UTF-8**，别去掉 BOM：powershell.exe 5.1 对无 BOM
# 的文件按本机 ANSI 代码页（GBK）读，上面这些中文注释会被拆成半个字符，
# 整份脚本解析失败——报的却是下面那些正常行「缺少表达式」。
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][ValidateSet('keys', 'ime', 'minimize', 'restore')][string]$Action,
    [string]$Keys = '',
    # 中文输入法的 KLID。0804 是简体中文，本机装的是微软拼音。
    [string]$Klid = '00000804'
)

$ErrorActionPreference = 'Stop'

Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class SpikeWin {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr LoadKeyboardLayout(string pwszKLID, uint Flags);

    [DllImport("user32.dll")]
    public static extern IntPtr PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@

$process = Get-Process -Id $ProcessId
$hwnd = $process.MainWindowHandle
if ($hwnd -eq [IntPtr]::Zero) {
    Write-Output 'hwnd=0'
    exit 2
}

switch ($Action) {
    'minimize' {
        # 6 = SW_MINIMIZE。前后台切换要走系统，程序自己 set 不出来。
        $null = [SpikeWin]::ShowWindow($hwnd, 6)
        Write-Output 'minimized'
    }
    'restore' {
        # 9 = SW_RESTORE
        $null = [SpikeWin]::ShowWindow($hwnd, 9)
        Write-Output 'restored'
    }
    default {
        $shell = New-Object -ComObject WScript.Shell
        # AppActivate 的返回值一定要看：它失败时 SendKeys 会静默打到别的
        # 窗口上，看起来就像「程序不响应按键」。
        $activated = $shell.AppActivate($ProcessId)
        if (-not $activated) {
            Write-Output 'activate=false'
            exit 3
        }
        Start-Sleep -Milliseconds 300

        if ($Action -eq 'ime') {
            $hkl = [SpikeWin]::LoadKeyboardLayout($Klid, 1)
            # 0x0050 = WM_INPUTLANGCHANGEREQUEST
            $null = [SpikeWin]::PostMessage($hwnd, 0x0050, [IntPtr]::Zero, $hkl)
            Start-Sleep -Milliseconds 500
            Write-Output "hkl=$hkl"
        }

        if ($Keys -ne '') {
            # 一次一个键，给 IME 留出候选窗口更新的时间；整串发过去时
            # 微软拼音经常只收到前两个字母。`{TAB}` 这类转义要整段发，
            # 拆成单字符的话 SendKeys 会把花括号本身打出来。
            foreach ($token in [regex]::Matches($Keys, '\{[^}]+\}|.')) {
                $shell.SendKeys($token.Value)
                Start-Sleep -Milliseconds 60
            }
        }
        Write-Output 'activate=true'
    }
}
