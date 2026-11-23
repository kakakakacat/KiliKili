//! 从窗口外面驱动实验：按键、中文输入法、最小化/还原、抓 UIA 树。
//!
//! 三件事只能从外面做：
//!
//! 1. **IME 组字**——程序自己 `set_text` 等于把被测的那一段跳过去了；
//! 2. **前后台切换**——最小化是窗口管理器的事，应用改不了自己的状态；
//! 3. **无障碍树**——要的就是「别的进程能读到什么」。
//!
//! 实现方式是起 `powershell.exe` 跑 `win/*.ps1`（用 Windows PowerShell 而
//! 不是 pwsh：UIAutomationClient 是 WPF 的程序集，pwsh 7 不一定加载得到）。
//! macOS 上起 `sh mac/drive.sh`，动作一一对应（System Events 发键、
//! Carbon 切输入源、AX 抓树），抓出来的树写成同一份 JSON 格式。
//! macOS 的 `ime` 切的是**全局**输入源，所以 [`Driver::drain`] 收尾时会
//! 切回实验之前的那个。
//!
//! **`win/*.ps1` 必须存成带 BOM 的 UTF-8**，别让编辑器把 BOM 去掉。
//! Windows PowerShell 5.1 对没有 BOM 的文件按本机 ANSI 代码页读（这台机器
//! 是 GBK），中文注释会被拆成半个字符，整份脚本在**解析阶段**就失败。
//! 失败的样子很有迷惑性：报的是 `[string]$Keys = '',` 这类正常行「缺少
//! 表达式」，看着像脚本写错了；而 spike 这边只是收到非零退出码，记录里
//! 所有按键/IME/UIA 项一起变 FAIL，**像是 Slint 收不到输入**。
//! 2026-11-22 的第一轮就是这么跑废的。
//!
//! **一律异步起、下一帧再收**：起一个 PowerShell 进程就要几百毫秒，抓一次
//! UIA 树可能要几秒。在 UI 线程里同步等于自己给自己制造一个长帧，然后把它
//! 记成「Slint 卡了」。SPIKE-001 在截图上踩过同一个坑。

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Keys,
    Ime,
    Window,
    Uia,
}

/// 一次外部动作。两个平台的脚本参数长得不一样，在 [`Driver::spawn`] 里各自拼。
enum Op<'a> {
    /// 空串表示只切前台。
    Keys(&'a str),
    Ime(&'a str),
    Window(&'static str),
    Uia(&'a Path),
}

struct Pending {
    kind: Kind,
    label: String,
    child: Child,
    out: Option<PathBuf>,
    started: Instant,
}

/// UIA 树里的一个节点。
#[derive(Debug, Clone)]
pub struct UiaNode {
    pub depth: i64,
    pub control: String,
    pub name: String,
    pub automation_id: String,
    pub value: String,
    pub offscreen: bool,
}

#[derive(Debug, Clone)]
pub struct UiaDump {
    pub label: String,
    pub found: bool,
    pub truncated: bool,
    pub node_count: usize,
    pub elapsed_ms: i64,
    pub error: String,
    pub nodes: Vec<UiaNode>,
}

impl UiaDump {
    /// 控件类型以 `ControlType.X` 的形式出现，这里只比最后那一段。
    #[must_use]
    pub fn count_of(&self, control: &str) -> usize {
        self.nodes
            .iter()
            .filter(|n| n.control.rsplit('.').next() == Some(control))
            .count()
    }

    #[must_use]
    pub fn any_name_containing(&self, needle: &str) -> Option<&UiaNode> {
        self.nodes.iter().find(|n| n.name.contains(needle))
    }
}

pub struct Driver {
    enabled: bool,
    dir: PathBuf,
    out_dir: PathBuf,
    pid: u32,
    /// 抓树的次数，只用来给 JSON 起不撞名的文件名。
    uia_seq: u32,
    pending: Vec<Pending>,
    /// 按时间顺序记下发生过什么，直接进记录的「备注」。
    pub log: Vec<String>,
    pub dumps: Vec<UiaDump>,
}

impl Driver {
    /// `dir` 是 `win/` 或 `mac/` 目录，`out_dir` 放 UIA 的 JSON（macOS 上还放
    /// 「实验前的输入源」）。
    #[must_use]
    pub fn new(enabled: bool, dir: PathBuf, out_dir: PathBuf, pid: u32) -> Self {
        // 开跑之前先确认两份脚本还带着 BOM。少了 BOM 的失败**是静默的**：
        // 每一步都只是退出码非零，记录里按键/IME/UIA 一起变 FAIL，读的人
        // 会去怀疑 Slint。宁可在这里先喊一声。
        let mut log = Vec::new();
        if enabled && cfg!(windows) {
            for name in ["drive.ps1", "uia-dump.ps1"] {
                match std::fs::read(dir.join(name)) {
                    Ok(bytes) if bytes.starts_with(b"\xef\xbb\xbf") => {}
                    Ok(_) => log.push(format!(
                        "win/{name} 没有 UTF-8 BOM：powershell.exe 5.1 会按 ANSI 代码页读它，\
                         中文注释被拆开后整份脚本解析失败，于是按键/IME/UIA 全部测不到。\
                         把文件存成带 BOM 的 UTF-8 再跑。"
                    )),
                    Err(err) => log.push(format!("读不了 win/{name}：{err}")),
                }
            }
        }
        // macOS 的两个 Swift 小工具在这里**同步**编好：第一次编要好几秒，
        // 放到异步那条路上的话，第一次抓树会晚到列表场景已经切走之后。
        if enabled && cfg!(target_os = "macos") {
            match Command::new("sh")
                .arg(dir.join("drive.sh"))
                .arg("build")
                .output()
            {
                Ok(output) if output.status.success() => {}
                Ok(output) => log.push(format!(
                    "mac/drive.sh build 失败：{}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )),
                Err(err) => log.push(format!("起不了 mac/drive.sh：{err}")),
            }
        }
        Self {
            enabled: enabled && (cfg!(windows) || cfg!(target_os = "macos")),
            dir,
            out_dir,
            pid,
            uia_seq: 0,
            pending: Vec::new(),
            log,
            dumps: Vec::new(),
        }
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// 只切前台。两边的 `keys` 动作在按键为空时都不发按键。
    pub fn activate(&mut self) {
        self.spawn(Kind::Keys, "切到前台".to_owned(), &Op::Keys(""), None);
    }

    pub fn keys(&mut self, keys: &str) {
        self.spawn(Kind::Keys, keys.to_owned(), &Op::Keys(keys), None);
    }

    /// `klid` 是键盘布局 id：`00000409` 美式英文、`00000804` 简体中文。
    /// macOS 上映射成 ABC / 简体拼音两个输入源（见 `mac/drive.sh`）。
    pub fn ime(&mut self, klid: &str, label: &str) {
        self.spawn(
            Kind::Ime,
            format!("{label}（{klid}）"),
            &Op::Ime(klid),
            None,
        );
    }

    pub fn window(&mut self, action: &'static str) {
        self.spawn(Kind::Window, action.to_owned(), &Op::Window(action), None);
    }

    pub fn uia(&mut self, label: &str) {
        // 文件名**必须带序号**：标签是中文，`sanitize` 之后「列表」与
        // 「控件」都变成 `__`，两次抓树会写到同一个文件、互相覆盖，而
        // 覆盖是静默的——记录里只会少一行，看不出来是撞名。
        self.uia_seq += 1;
        let out = self
            .out_dir
            .join(format!("uia-{}-{}.json", self.uia_seq, sanitize(label)));
        self.spawn(
            Kind::Uia,
            label.to_owned(),
            &Op::Uia(&out),
            Some(out.clone()),
        );
    }

    fn spawn(&mut self, kind: Kind, label: String, op: &Op<'_>, out: Option<PathBuf>) {
        if !self.enabled {
            self.log
                .push(format!("跳过外部驱动（未启用）：{kind:?} {label}"));
            return;
        }
        if let Some(parent) = out.as_ref().and_then(|p| p.parent()) {
            std::fs::create_dir_all(parent).ok();
        }
        let mut command = if cfg!(target_os = "macos") {
            self.mac_command(op)
        } else {
            self.win_command(op)
        };
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        match command.spawn() {
            Ok(child) => self.pending.push(Pending {
                kind,
                label,
                child,
                out,
                started: Instant::now(),
            }),
            Err(err) => self
                .log
                .push(format!("起不了外部驱动（{kind:?} {label}）：{err}")),
        }
    }

    fn mac_command(&self, op: &Op<'_>) -> Command {
        let mut command = Command::new("sh");
        command
            .arg(self.dir.join("drive.sh"))
            .arg(self.pid.to_string());
        match op {
            Op::Keys(keys) => command.arg("keys").arg(keys),
            Op::Ime(klid) => command.arg("ime").arg(klid).arg(&self.out_dir),
            Op::Window(action) => command.arg(action),
            Op::Uia(out) => command.arg("uia").arg(out),
        };
        command
    }

    fn win_command(&self, op: &Op<'_>) -> Command {
        let script = match op {
            Op::Uia(_) => self.dir.join("uia-dump.ps1"),
            _ => self.dir.join("drive.ps1"),
        };
        let mut command = Command::new("powershell");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW：**不给 PowerShell 开控制台窗口**。
            // 开了的话每次驱动都有一个黑窗口弹到被测窗口前面，Windows 把
            // 被遮住的窗口的呈现降到几帧每秒，于是量到的帧间隔是遮挡造成
            // 的，不是 Slint 的。实验工具不能自己污染被测对象。
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&script)
            .arg("-ProcessId")
            .arg(self.pid.to_string());
        match op {
            Op::Keys("") => command.args(["-Action", "keys"]),
            Op::Keys(keys) => command.args(["-Action", "keys", "-Keys", keys]),
            Op::Ime(klid) => command.args(["-Action", "ime", "-Klid", klid]),
            Op::Window(action) => command.args(["-Action", action]),
            Op::Uia(out) => command.arg("-Out").arg(out),
        };
        command
    }

    /// 每帧调一次，收走已经结束的外部进程。
    pub fn poll(&mut self) {
        let mut finished = Vec::new();
        for (index, pending) in self.pending.iter_mut().enumerate() {
            match pending.child.try_wait() {
                Ok(Some(_)) => finished.push(index),
                Ok(None) => {}
                Err(err) => {
                    self.log.push(format!("等不到外部进程：{err}"));
                    finished.push(index);
                }
            }
        }
        for index in finished.into_iter().rev() {
            let pending = self.pending.remove(index);
            self.collect(pending);
        }
    }

    /// 实验结束时还有没收完的就等一下——UIA 那次可能正好压在退出前。
    /// macOS 上还要把全局输入源切回实验之前的那个。
    pub fn drain(&mut self) {
        self.drain_pending();
        if self.enabled && cfg!(target_os = "macos") {
            match Command::new("sh")
                .arg(self.dir.join("drive.sh"))
                .arg(self.pid.to_string())
                .arg("ime-restore")
                .arg("-")
                .arg(&self.out_dir)
                .output()
            {
                Ok(output) => self.log.push(format!(
                    "收尾切回输入源：{}",
                    String::from_utf8_lossy(&output.stdout).trim()
                )),
                Err(err) => self.log.push(format!("切不回输入源：{err}")),
            }
        }
    }

    fn drain_pending(&mut self) {
        while !self.pending.is_empty() {
            let pending = self.pending.remove(0);
            let _ = pending.child.wait_with_output().map(|output| {
                self.log.push(format!(
                    "{:?} {}：收尾 {}",
                    pending.kind,
                    pending.label,
                    String::from_utf8_lossy(&output.stdout).trim()
                ));
                if let Some(path) = pending.out.as_ref()
                    && let Ok(dump) = read_dump(path, &pending.label)
                {
                    self.dumps.push(dump);
                }
            });
        }
    }

    fn collect(&mut self, pending: Pending) {
        let elapsed = pending.started.elapsed().as_millis();
        let Pending {
            kind,
            label,
            child,
            out,
            ..
        } = pending;
        let output = child.wait_with_output();
        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                self.log.push(format!(
                    "{kind:?} 「{label}」{elapsed}ms：{stdout}{}",
                    if stderr.is_empty() {
                        String::new()
                    } else {
                        format!("（stderr: {stderr}）")
                    }
                ));
            }
            Err(err) => self
                .log
                .push(format!("{kind:?}「{label}」收不到输出：{err}")),
        }
        if let Some(path) = out {
            match read_dump(&path, &label) {
                Ok(dump) => self.dumps.push(dump),
                Err(err) => self.log.push(format!("读不了 UIA 树：{err:#}")),
            }
        }
    }

    #[must_use]
    pub fn dump(&self, label: &str) -> Option<&UiaDump> {
        self.dumps.iter().find(|d| d.label == label)
    }
}

fn sanitize(label: &str) -> String {
    label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn read_dump(path: &Path, label: &str) -> Result<UiaDump> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("读不了 {}", path.display()))?;
    // 容忍 BOM：写它的那一侧已经改成不带 BOM 了，但换一个 PowerShell
    // 版本、换一个 `Out-File` 写法就会带上，而 serde_json 报的
    // `expected value at line 1 column 1` 完全看不出是 BOM。
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let value: serde_json::Value = serde_json::from_str(text).context("UIA 树不是合法 JSON")?;
    let nodes = value
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(|array| {
            array
                .iter()
                .map(|node| UiaNode {
                    depth: node
                        .get("depth")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(-1),
                    control: string_at(node, "control"),
                    name: string_at(node, "name"),
                    automation_id: string_at(node, "automationId"),
                    value: string_at(node, "value"),
                    offscreen: node
                        .get("offscreen")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(UiaDump {
        label: label.to_owned(),
        found: value
            .get("found")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        truncated: value
            .get("truncated")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        node_count: value
            .get("node_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize,
        elapsed_ms: value
            .get("elapsed_ms")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(-1),
        error: string_at(&value, "error"),
        nodes,
    })
}

fn string_at(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
