#!/usr/bin/env python3
"""Generate README activity charts from KiliKili's first-parent history."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from html import escape
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "assets" / "stats"
FORK_BASE = "f52160af0a026adfefaa4f9f99f0702b2cab0200"
RECENT_COUNT = 8
CODE_EXTENSIONS = {
    ".c", ".cc", ".cpp", ".dart", ".h", ".hpp", ".java",
    ".kt", ".kts", ".m", ".mm", ".py", ".rs", ".sh",
    ".swift",
}
CODE_DIRS = ("android/", "ios/", "lib/", "linux/", "macos/", "rust/", "test/", "tool/", "windows/")
GENERATED_NAMES = {"frb_generated.dart", "frb_generated.io.dart", "frb_generated.rs"}
BG = "#0d1117"
PANEL = "#161b22"
TEXT = "#e6edf3"
MUTED = "#8b949e"
GREEN = "#3fb950"
RED = "#f85149"
BLUE = "#58a6ff"


@dataclass
class Commit:
    sha: str
    date: str
    subject: str
    added: int = 0
    deleted: int = 0


def is_code(path: str) -> bool:
    if not path.startswith(CODE_DIRS):
        return False
    name = Path(path).name
    if name in GENERATED_NAMES or name.endswith((".g.dart", ".freezed.dart", ".pb.dart", ".pbenum.dart", ".pbjson.dart")):
        return False
    return Path(path).suffix in CODE_EXTENSIONS


def commits() -> list[Commit]:
    log = subprocess.check_output(
        [
            "git", "log", "--first-parent", "-m", "--no-renames",
            "--format=@@COMMIT@@%H%x09%cI%x09%s", "--numstat",
            f"{FORK_BASE}..HEAD",
        ],
        cwd=ROOT,
        text=True,
    )
    result: list[Commit] = []
    current: Commit | None = None
    for line in log.splitlines():
        if line.startswith("@@COMMIT@@"):
            sha, timestamp, subject = line[len("@@COMMIT@@"):].split("\t", 2)
            # Upstream syncs are not KiliKili development. Generated chart
            # commits must not count themselves or change the next chart.
            if subject.startswith("chore(stats):") or "bggRGjQaUbCoE:main" in subject:
                current = None
                continue
            if subject.startswith("Merge pull request #"):
                title = subprocess.check_output(
                    ["git", "show", "-s", "--format=%b", sha], cwd=ROOT, text=True
                ).strip().splitlines()
                if title:
                    subject = f"{subject.split()[3]} {title[0]}"
            current = Commit(sha, datetime.fromisoformat(timestamp).strftime("%Y-%m-%d"), subject)
            result.append(current)
        elif current is not None and "\t" in line:
            added, deleted, path = line.split("\t", 2)
            if added.isdigit() and deleted.isdigit() and is_code(path):
                current.added += int(added)
                current.deleted += int(deleted)
    return result


def short(text: str, limit: int) -> str:
    return text if len(text) <= limit else text[: limit - 1] + "…"


def svg_start(title: str, description: str, height: int) -> list[str]:
    return [
        f'<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-label="{escape(title)}" viewBox="0 0 920 {height}">',
        f"<title>{escape(title)}</title>",
        f"<desc>{escape(description)}</desc>",
        f'<rect width="920" height="{height}" rx="16" fill="{BG}"/>',
        f'<rect x="1" y="1" width="918" height="{height - 2}" rx="15" fill="none" stroke="#30363d"/>',
    ]


def label(x: int, y: int, value: str, size: int = 16, color: str = TEXT, weight: int = 400) -> str:
    return f'<text x="{x}" y="{y}" fill="{color}" font-family="-apple-system, BlinkMacSystemFont, Segoe UI, Noto Sans, sans-serif" font-size="{size}" font-weight="{weight}">{escape(value)}</text>'


def render_commits(history: list[Commit]) -> str:
    recent = history[:RECENT_COUNT]
    height = 114 + 43 * max(len(recent), 1)
    parts = svg_start("KiliKili recent commits", "Recent main-branch commits since the PiliPlus fork", height)
    parts += [label(28, 42, "最近提交 · Recent commits", 22, TEXT, 700),
              label(28, 72, f"自 fork 起 {len(history)} 次主线提交 · 更新于 main 每次推送后", 14, MUTED)]
    if not recent:
        parts.append(label(28, 126, "暂无提交 · No commits yet", 16, MUTED))
    else:
        parts.append(f'<line x1="43" y1="102" x2="43" y2="{104 + 43 * (len(recent) - 1)}" stroke="#30363d" stroke-width="2"/>')
        for index, commit in enumerate(recent):
            y = 108 + index * 43
            parts.append(f'<circle cx="43" cy="{y - 5}" r="5" fill="{BLUE}"/>')
            parts.append(label(62, y, commit.date, 13, MUTED))
            parts.append(label(178, y, commit.sha[:8], 14, BLUE, 600))
            parts.append(label(266, y, short(commit.subject, 72), 14))
    parts.append("</svg>")
    return "\n".join(parts) + "\n"


def render_changes(history: list[Commit]) -> str:
    code_commits = [item for item in history if item.added or item.deleted]
    recent = code_commits[:RECENT_COUNT]
    height = 128 + 43 * max(len(recent), 1)
    total_added = sum(item.added for item in history)
    total_deleted = sum(item.deleted for item in history)
    parts = svg_start("KiliKili code changes", "Source-code lines added and deleted on the main branch since the PiliPlus fork", height)
    parts += [label(28, 42, "代码修改 · Code changes", 22, TEXT, 700),
              label(28, 72, f"自 fork 起 +{total_added:,} / −{total_deleted:,} 行 · 不含生成文件与上游同步", 14, MUTED),
              f'<rect x="28" y="88" width="11" height="11" rx="2" fill="{GREEN}"/>',
              label(45, 98, "新增", 12, MUTED),
              f'<rect x="91" y="88" width="11" height="11" rx="2" fill="{RED}"/>',
              label(108, 98, "删除", 12, MUTED)]
    if not recent:
        parts.append(label(28, 138, "暂无代码修改 · No code changes yet", 16, MUTED))
    else:
        maximum = max(item.added + item.deleted for item in recent)
        for index, commit in enumerate(recent):
            y = 132 + index * 43
            parts.append(label(28, y + 2, commit.sha[:8], 13, BLUE, 600))
            parts.append(label(112, y + 2, short(commit.subject, 35), 12, MUTED))
            green_width = round(300 * commit.added / maximum)
            red_width = round(300 * commit.deleted / maximum)
            parts.append(f'<rect x="440" y="{y - 11}" width="{green_width}" height="16" rx="3" fill="{GREEN}"/>')
            if red_width:
                parts.append(f'<rect x="{440 + green_width}" y="{y - 11}" width="{red_width}" height="16" rx="3" fill="{RED}"/>')
            parts.append(label(755, y + 2, f"+{commit.added:,} / −{commit.deleted:,}", 12, TEXT))
    parts.append("</svg>")
    return "\n".join(parts) + "\n"


def main() -> None:
    history = commits()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    (OUTPUT / "commits.svg").write_text(render_commits(history), encoding="utf-8")
    (OUTPUT / "code-changes.svg").write_text(render_changes(history), encoding="utf-8")
    print(f"Generated README charts from {len(history)} main-branch commits")


if __name__ == "__main__":
    main()
