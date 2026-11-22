# 抓一个进程的 UI Automation 树，写成 JSON。
#
# 为什么是 UIA 而不是「打开 NVDA 听一遍」：屏幕阅读器读到的东西**就是**
# 这棵树。树里没有的，任何读屏软件都读不出来；树里有的，差别只在各家
# 怎么念。于是无障碍这一项可以脚本化，跑两次能逐行对照——这正是
# SPIKE-PLAN 对实验记录的要求。
#
# 用 Windows PowerShell（powershell.exe）跑：UIAutomationClient 是 WPF 的
# 程序集，pwsh 7 不一定加载得到。
#
# 这个文件**存成带 BOM 的 UTF-8**，别去掉 BOM：powershell.exe 5.1 对无 BOM
# 的文件按本机 ANSI 代码页（GBK）读，上面这些中文注释会被拆成半个字符，
# 整份脚本解析失败——报的却是下面那些正常行「缺少表达式」。
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$MaxNodes = 600,
    [int]$MaxDepth = 14,
    [int]$TimeoutSeconds = 20
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$result = [ordered]@{
    found      = $false
    truncated  = $false
    node_count = 0
    elapsed_ms = 0
    error      = ''
    nodes      = @()
}

function Get-Value($element) {
    try {
        $pattern = $null
        if ($element.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
            return $pattern.Current.Value
        }
    } catch {}
    # 滑块这类**连续取值**的控件走的是 RangeValuePattern，不是 ValuePattern。
    # 只试前者的话滑块的取值永远是空，看起来像「Slint 没把值暴露出去」，
    # 其实是这里没问对——无障碍结论最容易这样被工具自己做坏。
    try {
        $range = $null
        if ($element.TryGetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern, [ref]$range)) {
            $current = $range.Current
            return "$($current.Value)（$($current.Minimum)..$($current.Maximum)）"
        }
    } catch {}
    return ''
}

try {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
    $window = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $condition)

    if ($null -eq $window) {
        $result.error = "没有找到进程 $ProcessId 的顶层窗口"
    } else {
        $result.found = $true
        $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
        # 广度优先：先把浅层看全。深层主要是列表项，截断了也不影响结论。
        $queue = New-Object System.Collections.Queue
        $queue.Enqueue(@{ element = $window; depth = 0 })
        $nodes = New-Object System.Collections.ArrayList

        while ($queue.Count -gt 0) {
            if ($nodes.Count -ge $MaxNodes -or $sw.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
                $result.truncated = $true
                break
            }
            $item = $queue.Dequeue()
            $element = $item.element
            $depth = $item.depth

            $null = $nodes.Add([ordered]@{
                depth        = $depth
                control      = $element.Current.ControlType.ProgrammaticName
                name         = $element.Current.Name
                automationId = $element.Current.AutomationId
                value        = (Get-Value $element)
                keyboard     = $element.Current.HasKeyboardFocus
                offscreen    = $element.Current.IsOffscreen
            })

            if ($depth -lt $MaxDepth) {
                $child = $walker.GetFirstChild($element)
                while ($null -ne $child) {
                    $queue.Enqueue(@{ element = $child; depth = $depth + 1 })
                    $child = $walker.GetNextSibling($child)
                }
            }
        }
        $result.nodes = $nodes.ToArray()
        $result.node_count = $nodes.Count
    }
} catch {
    $result.error = $_.Exception.Message
}

$result.elapsed_ms = [int]$sw.Elapsed.TotalMilliseconds
$json = $result | ConvertTo-Json -Depth 6
# 写出去的 JSON **不要 BOM**：`[Text.Encoding]::UTF8` 这个现成实例是带 BOM 的，
# serde_json 读到就报 `expected value at line 1 column 1`，看着像 JSON 坏了。
# 和这个文件自己必须带 BOM 正好相反——源码给 powershell.exe 读，JSON 给
# Rust 读，两边的规矩不是一回事。
[System.IO.File]::WriteAllText($Out, $json, (New-Object System.Text.UTF8Encoding($false)))
Write-Output "nodes=$($result.node_count) found=$($result.found) truncated=$($result.truncated)"
