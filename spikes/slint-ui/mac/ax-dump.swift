// 抓一个进程窗口里的 macOS 无障碍树（AX），对应 Windows 的 `win/uia-dump.ps1`。
//
//   swiftc -O ax-dump.swift -o ax-dump
//   ./ax-dump <pid> [轮数]              # 人看的缩进树
//   ./ax-dump <pid> --json <输出文件>   # 给 src/drive.rs 读，格式同 win/uia-dump.ps1
//
// JSON 里的 control 写成「AX 角色.对应的 UIA 控件类型」，例如
// `AXTextField.Edit`、`AXStaticText.ListItem`：实验台的判定按 UIA 的名字找
// （`ends_with("ListItem")`），而 AX 原名留在前面，不把 macOS 的真实角色
// 抹掉。列表的**直接子元素**一律记成 `.ListItem`——Slint 在 macOS 上给行的
// 角色是 AXStaticText，但旁白把它们当列表项念（「1/7」），判定按旁白的行为。
// offscreen 一律 false：AX 没有对应属性。
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
let jsonOut: String? = args.count > 3 && args[2] == "--json" ? args[3] : nil
let rounds = jsonOut == nil && args.count > 2 ? Int(args[2]) ?? 2 : 2

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

let app = AXUIElementCreateApplication(pid)

func uiaName(_ role: String, parentRole: String) -> String {
    if parentRole == "AXList" { return "ListItem" }
    switch role {
    case "AXList", "AXTable", "AXOutline": return "List"
    case "AXButton": return "Button"
    case "AXCheckBox": return "CheckBox"
    case "AXSlider": return "Slider"
    case "AXTextField", "AXTextArea": return "Edit"
    case "AXStaticText": return "Text"
    case "AXWindow": return "Window"
    case "AXGroup": return "Group"
    default: return "Custom"
    }
}

if let jsonOut {
    var nodes: [[String: Any]] = []
    var truncated = false
    func collect(_ e: AXUIElement, _ depth: Int, _ parentRole: String) {
        if nodes.count >= 2000 { truncated = true; return }
        let role = attr(e, "AXRole") ?? ""
        let value = attr(e, "AXValue") ?? ""
        let name = attr(e, "AXTitle") ?? attr(e, "AXDescription")
            ?? (role == "AXStaticText" ? value : "")
        nodes.append([
            "depth": depth,
            "control": "\(role).\(uiaName(role, parentRole: parentRole))",
            "name": name,
            "automationId": "",
            "value": role == "AXStaticText" ? "" : value,
            "offscreen": false,
        ])
        var kids: CFTypeRef?
        if AXUIElementCopyAttributeValue(e, kAXChildrenAttribute as CFString, &kids) == .success,
           let children = kids as? [AXUIElement] {
            for child in children { collect(child, depth + 1, role) }
        }
    }
    func windows() -> [AXUIElement] {
        var value: CFTypeRef?
        AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value)
        return value as? [AXUIElement] ?? []
    }
    // 先碰一下让 AccessKit 把真树建起来，再抓——见文件开头「懒初始化」。
    for window in windows() { collect(window, 0, "") }
    nodes.removeAll()
    Thread.sleep(forTimeInterval: 1.5)
    let started = Date()
    let found = !windows().isEmpty
    for window in windows() { collect(window, 0, "") }
    let result: [String: Any] = [
        "found": found,
        "truncated": truncated,
        "node_count": nodes.count,
        "elapsed_ms": Int(Date().timeIntervalSince(started) * 1000),
        "error": AXIsProcessTrusted() ? "" : "终端没有辅助功能权限（AXIsProcessTrusted=false）",
        "nodes": nodes,
    ]
    let data = try! JSONSerialization.data(withJSONObject: result, options: [])
    FileManager.default.createFile(atPath: jsonOut, contents: data)
    print("nodes=\(nodes.count) found=\(found) truncated=\(truncated)")
    exit(0)
}

print("trusted=\(AXIsProcessTrusted())")
for round in 0..<rounds {
    if round > 0 { Thread.sleep(forTimeInterval: 1.5) }
    count = 0
    print("--- round \(round)")
    var windows: CFTypeRef?
    AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &windows)
    for window in (windows as? [AXUIElement]) ?? [] { walk(window, 0) }
    print("nodes=\(count)")
}
