//! 实验记录产出。字段对齐 `docs/m0/SPIKE-PLAN.md` 的「Test artifact template」。
//!
//! 这是 spike 的**交付物本身**：没有这份记录，跑没跑过只有跑的人自己知道。
//! 所以它由程序写，不由人写——人写的记录漏掉工具链版本和 commit 的概率是
//! 100%，而那两样恰恰是「结论还算不算数」的前提。
//!
//! 机器判不了的项（层级对不对、画面糊不糊、音画同不同步）老实标成
//! `人工确认`，不许程序替人按 PASS。

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Blocked,
    NotTested,
    /// 机器只能给出证据，结论要人看屏幕或看截图。
    Manual,
}

impl Verdict {
    const fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "**FAIL**",
            Self::Blocked => "BLOCKED",
            Self::NotTested => "NOT TESTED",
            Self::Manual => "人工确认",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub verdict: Verdict,
}

impl Check {
    pub fn new(
        name: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
        verdict: Verdict,
    ) -> Self {
        Self {
            name: name.into(),
            expected: expected.into(),
            actual: actual.into(),
            verdict,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Evidence {
    pub title: String,
    pub command_line: String,
    pub env: Vec<(String, String)>,
    pub media: Vec<(String, String)>,
    pub metrics: Vec<(String, String)>,
    pub checks: Vec<Check>,
    pub shots: Vec<PathBuf>,
    pub notes: Vec<String>,
}

impl Evidence {
    pub fn push_env(&mut self, key: &str, value: impl Into<String>) {
        self.env.push((key.to_owned(), value.into()));
    }

    pub fn push_media(&mut self, key: &str, value: impl Into<String>) {
        self.media.push((key.to_owned(), value.into()));
    }

    pub fn push_metric(&mut self, key: &str, value: impl Into<String>) {
        self.metrics.push((key.to_owned(), value.into()));
    }

    pub fn push_check(&mut self, check: Check) {
        self.checks.push(check);
    }

    /// 整体结论：有 FAIL 就 FAIL，否则有 BLOCKED 就 BLOCKED，
    /// 都没有但还有人工项就 PASS（自动项）。
    #[must_use]
    pub fn overall(&self) -> Verdict {
        if self.checks.iter().any(|c| c.verdict == Verdict::Fail) {
            Verdict::Fail
        } else if self.checks.iter().any(|c| c.verdict == Verdict::Blocked) {
            Verdict::Blocked
        } else if self.checks.is_empty() {
            Verdict::NotTested
        } else {
            Verdict::Pass
        }
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# {}\n", self.title);
        let _ = writeln!(
            out,
            "自动产出于 `slint-mpv-spike`，不要手工编辑：重跑一次会整份覆盖。\n"
        );
        let _ = writeln!(out, "命令行：`{}`\n", self.command_line);
        let _ = writeln!(
            out,
            "**自动项结论：{}**（人工项见下表）\n",
            self.overall().label()
        );

        section(&mut out, "环境", &self.env);
        section(&mut out, "媒体与渲染", &self.media);
        section(&mut out, "测量", &self.metrics);

        let _ = writeln!(out, "## 验证项\n");
        let _ = writeln!(out, "| 项 | 预期 | 实际 | 结论 |");
        let _ = writeln!(out, "|---|---|---|---|");
        for check in &self.checks {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                escape_cell(&check.name),
                escape_cell(&check.expected),
                escape_cell(&check.actual),
                check.verdict.label()
            );
        }
        let _ = writeln!(out);

        if !self.shots.is_empty() {
            let _ = writeln!(out, "## 截图\n");
            for shot in &self.shots {
                let name = shot
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let _ = writeln!(out, "- `{name}`");
            }
            let _ = writeln!(out);
        }

        if !self.notes.is_empty() {
            let _ = writeln!(out, "## 备注\n");
            for note in &self.notes {
                let _ = writeln!(out, "- {note}");
            }
            let _ = writeln!(out);
        }
        redact_home(&out)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, self.to_markdown())
            .with_context(|| format!("写不了记录：{}", path.display()))
    }
}

/// 把用户主目录换成 `~`。
///
/// 记录是要**进仓库**的：命令行、媒体路径、日志路径里一路带着
/// `C:\Users\<名字>` 或 `/Users/<名字>`，等于把开发者的账号名提交上去。
/// 这和 `bili-domain::redact` 是同一条规矩——按来源处理，不靠人每次记得。
/// Windows 上路径大小写与分隔符都不固定，所以两种分隔符各比一次，
/// 并忽略大小写。
fn redact_home(text: &str) -> String {
    let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|h| h.to_string_lossy().into_owned())
    else {
        return text.to_owned();
    };
    if home.is_empty() {
        return text.to_owned();
    }
    let variants = [home.replace('\\', "/"), home.replace('/', "\\")];
    let mut out = text.to_owned();
    for variant in variants {
        out = replace_ignore_case(&out, &variant, "~");
    }
    out
}

fn replace_ignore_case(haystack: &str, needle: &str, with: &str) -> String {
    // 只折 ASCII 大小写：`to_lowercase` 对某些非 ASCII 字符会改变字节长度，
    // 折完再按字节位置切原串就会把字符切成两半（记录里全是中文，这不是
    // 假想情况）。`to_ascii_lowercase` 不改长度，位置永远对得上。
    let lower_haystack = haystack.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    if lower_needle.is_empty() {
        return haystack.to_owned();
    }
    let mut out = String::with_capacity(haystack.len());
    let mut cursor = 0;
    while let Some(found) = lower_haystack[cursor..].find(&lower_needle) {
        let start = cursor + found;
        out.push_str(&haystack[cursor..start]);
        out.push_str(with);
        cursor = start + needle.len();
    }
    out.push_str(&haystack[cursor..]);
    out
}

/// 竖线必须转义：GL 的 renderer 串本身就带 `|`（"厂商 | 型号 | 版本"），
/// 原样写进表格会把那一行劈成四列，证据读起来就全乱了。
fn escape_cell(value: &str) -> String {
    value.replace('|', r"\|")
}

fn section(out: &mut String, title: &str, rows: &[(String, String)]) {
    if rows.is_empty() {
        return;
    }
    let _ = writeln!(out, "## {title}\n");
    let _ = writeln!(out, "| 项 | 值 |");
    let _ = writeln!(out, "|---|---|");
    for (key, value) in rows {
        let _ = writeln!(out, "| {} | {} |", escape_cell(key), escape_cell(value));
    }
    let _ = writeln!(out);
}

/// UTC 时间戳，形如 `2026-11-21 08:30:11 UTC`。
///
/// 自己算而不是引 chrono：只为一行时间戳加一棵依赖树不划算。
#[must_use]
pub fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant 的 civil_from_days。
const fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// 跑一条命令取版本号之类的单行输出。取不到就说取不到，不要留空。
#[must_use]
pub fn capture(program: &str, args: &[&str]) -> String {
    std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(
            || format!("<取不到：{program}>"),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
        // 闰日。
        assert_eq!(civil_from_days(18_321), (2020, 2, 29));
        assert_eq!(civil_from_days(18_322), (2020, 3, 1));
    }

    #[test]
    fn a_single_failure_decides_the_overall_verdict() {
        let mut evidence = Evidence::default();
        evidence.push_check(Check::new("甲", "x", "x", Verdict::Pass));
        evidence.push_check(Check::new("乙", "x", "y", Verdict::Fail));
        evidence.push_check(Check::new("丙", "x", "?", Verdict::Manual));
        assert_eq!(evidence.overall(), Verdict::Fail);
    }

    #[test]
    fn nothing_measured_is_not_a_pass() {
        assert_eq!(Evidence::default().overall(), Verdict::NotTested);
    }

    #[test]
    fn the_home_directory_never_reaches_the_record() {
        // 记录会被提交，里面出现用户名就是把它提交到了公开仓库。
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .expect("开发机上总有其中之一");
        let mut evidence = Evidence {
            command_line: format!("{home}\\tool.exe --video {home}/a.mp4"),
            ..Evidence::default()
        };
        evidence.push_env("日志", format!("{home}/mpv.log"));
        let md = evidence.to_markdown();
        assert!(!md.contains(&home), "主目录原样出现在记录里：{md}");
        assert!(md.contains("~"));
    }

    #[test]
    fn replacing_ignores_case_but_keeps_the_rest() {
        assert_eq!(
            replace_ignore_case("C:/Users/X/a and c:/users/x/b", "C:/Users/X", "~"),
            "~/a and ~/b"
        );
        // 中文在场时位置不能切错（记录正文全是中文）。
        assert_eq!(
            replace_ignore_case("前缀 C:/Users/X/a 后缀", "c:/users/x", "~"),
            "前缀 ~/a 后缀"
        );
    }

    #[test]
    fn the_markdown_carries_every_check() {
        let mut evidence = Evidence {
            title: "标题".to_owned(),
            ..Evidence::default()
        };
        evidence.push_env("平台", "windows");
        evidence.push_check(Check::new("硬解", "不是 no", "d3d11va", Verdict::Pass));
        let md = evidence.to_markdown();
        assert!(md.contains("# 标题"));
        assert!(md.contains("d3d11va"));
        assert!(md.contains("PASS"));
    }
}
