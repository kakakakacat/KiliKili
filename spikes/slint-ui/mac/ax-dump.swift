// 抓一个进程窗口里的 macOS 无障碍树（AX），对应 Windows 的 `win/uia-dump.ps1`。
//
//   swiftc -O ax-dump.swift -o ax-dump
//   ./ax-dump <pid> [轮数]
//
// 要求：运行它的终端在「系统设置 → 隐私与安全性 → 辅助功能」里打开。
// 没打开时 AXIsProcessTrusted() 是 false，树是空的，**不会报错**——所以
// 第一行先把它打出来。
//
// 为什么默认抓两轮、间隔 1.5 秒：Slint（AccessKit）在 macOS 上是懒初始化的，
// 第一次查询只给一个没有角色、没有子节点的占位，第二轮才是真树。只抓一次
// 会得出「Slint 什么都没暴露」这个错误结论。
//
// 只遍历窗口，不碰菜单栏：菜单栏底下挂着「最近使用的项目」，会把本机的文件名
// 带进输出。
import ApplicationServices
import Foundation

let args = CommandLine.arguments
guard args.count >= 2, let pid = pid_t(args[1]) else {
    FileHandle.standardError.write("用法：ax-dump <pid> [轮数]\n".data(using: .utf8)!)
    exit(2)
}
let rounds = args.count > 2 ? Int(args[2]) ?? 2 : 2

func attr(_ e: AXUIElement, _ name: String) -> String? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(e, name as CFString, &value) == .success,
          let value else { return nil }
    if let s = value as? String { return s.isEmpty ? nil : s }
    if let n = value as? NSNumber { return n.stringValue }
    return nil
}

let fields = [
    ("role", "AXRole"), ("sub", "AXSubrole"), ("title", "AXTitle"),
    ("desc", "AXDescription"), ("value", "AXValue"), ("ph", "AXPlaceholderValue"),
]
var count = 0

func walk(_ e: AXUIElement, _ depth: Int) {
    count += 1
    if depth > 30 || count > 2000 { return }
    let parts = fields.compactMap { key, name in attr(e, name).map { "\(key)=\($0)" } }
    print(String(repeating: "  ", count: depth) + parts.joined(separator: " "))
    var kids: CFTypeRef?
    if AXUIElementCopyAttributeValue(e, kAXChildrenAttribute as CFString, &kids) == .success,
       let children = kids as? [AXUIElement] {
        for child in children { walk(child, depth + 1) }
    }
}

print("trusted=\(AXIsProcessTrusted())")
let app = AXUIElementCreateApplication(pid)
for round in 0..<rounds {
    if round > 0 { Thread.sleep(forTimeInterval: 1.5) }
    count = 0
    print("--- round \(round)")
    var windows: CFTypeRef?
    AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &windows)
    for window in (windows as? [AXUIElement]) ?? [] { walk(window, 0) }
    print("nodes=\(count)")
}
