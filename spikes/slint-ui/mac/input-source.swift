// 查询 / 切换 macOS 的输入源，对应 Windows 那边的 LoadKeyboardLayout。
//
//   input-source current          # 打印当前输入源 id
//   input-source list             # 打印所有可选的输入源
//   input-source select <id>      # 切过去，成功打印 id，失败退出码非零
//
// 为什么不用 AppleScript：System Events 没有切输入源的接口，模拟
// Control+空格又依赖用户的快捷键设置，而且切到哪个不确定。Carbon 的
// TISSelectInputSource 是唯一确定的做法。
import Carbon
import Foundation

func prop(_ source: TISInputSource, _ key: CFString) -> String {
    guard let raw = TISGetInputSourceProperty(source, key) else { return "" }
    return Unmanaged<CFString>.fromOpaque(raw).takeUnretainedValue() as String
}

func selectable() -> [TISInputSource] {
    let filter = [kTISPropertyInputSourceIsSelectCapable: true] as CFDictionary
    return TISCreateInputSourceList(filter, false).takeRetainedValue() as? [TISInputSource] ?? []
}

let args = CommandLine.arguments
switch args.count > 1 ? args[1] : "" {
case "current":
    print(prop(TISCopyCurrentKeyboardInputSource().takeRetainedValue(), kTISPropertyInputSourceID))
case "list":
    for source in selectable() {
        print("\(prop(source, kTISPropertyInputSourceID))\t\(prop(source, kTISPropertyLocalizedName))")
    }
case "select" where args.count > 2:
    let wanted = args[2]
    guard let source = selectable().first(where: { prop($0, kTISPropertyInputSourceID) == wanted }) else {
        FileHandle.standardError.write("没有可选的输入源 \(wanted)（系统设置 → 键盘 → 输入法里加上）\n".data(using: .utf8)!)
        exit(3)
    }
    let status = TISSelectInputSource(source)
    guard status == noErr else {
        FileHandle.standardError.write("TISSelectInputSource 失败：\(status)\n".data(using: .utf8)!)
        exit(4)
    }
    print(wanted)
default:
    FileHandle.standardError.write("用法：input-source current | list | select <id>\n".data(using: .utf8)!)
    exit(2)
}
