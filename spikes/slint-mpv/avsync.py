#!/usr/bin/env python3
"""#219 音画同步实测：生成闪光 + 哔声素材，量拍屏录像里两者的偏差。

    python3 avsync.py make  <输出.mp4> [--fps 30] [--seconds 60]
    python3 avsync.py measure <录像.mp4|.mov>

素材：黑底，每秒第 0 帧整屏白（只亮一帧），同一时刻 1 kHz 哔声 40 ms。
插帧会在白帧前后插出半亮的过渡帧，所以「闪光时刻」取每次闪光的亮度质心，
而不是第一张变亮的帧。

量法：手机播素材（`--setting 60` 与 `off` 各录一段），用另一台设备拍屏
（≥120 fps 慢动作更准，声音要录进去），或 iPhone 自带录屏（画面与声音同一
时钟，最省事）。`measure` 打印每次闪光对应的声 − 画偏差（正数 = 声音晚），
以及中位数。**看两段的差**：拍摄链路自己的延迟在两段里相同，相减就抵消了；
插帧接得对的话，两段的中位数应该相差不到一帧（≤ 17 ms）。

只用标准库和 ffmpeg / ffprobe（PATH 上）。
"""

from __future__ import annotations

import argparse
import statistics
import subprocess
import sys


def make(out: str, fps: int, seconds: int) -> None:
    beep = "if(lt(mod(t\\,1)\\,0.04)\\,0.8*sin(2*PI*1000*t)\\,0)"
    flash = f"if(lt(mod(N\\,{fps})\\,1)\\,255\\,16)"
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"color=black:s=1280x720:r={fps}:d={seconds}",
            "-f", "lavfi", "-i", f"aevalsrc={beep}:s=48000:d={seconds}",
            "-filter_complex", f"[0]format=gray,geq=lum='{flash}',format=yuv420p[v]",
            "-map", "[v]", "-map", "1",
            "-c:v", "libx264", "-preset", "fast", "-g", str(fps),
            "-c:a", "aac", "-b:a", "128k", out,
        ],
        check=True,
    )
    print(f"写好了 {out}：{fps} fps，{seconds} 秒，每秒一次闪光 + 哔声")


def brightness(path: str) -> list[tuple[float, float]]:
    """每帧 (时间, 平均亮度 0–255)。"""
    out = subprocess.run(
        [
            "ffprobe", "-v", "error", "-f", "lavfi",
            "-i", f"movie={path},format=gray,signalstats",
            "-show_entries", "frame=pts_time:frame_tags=lavfi.signalstats.YAVG",
            "-of", "csv=p=0",
        ],
        check=True, capture_output=True, text=True,
    ).stdout
    rows = []
    for line in out.splitlines():
        parts = line.split(",")
        if len(parts) == 2 and parts[0] and parts[1]:
            rows.append((float(parts[0]), float(parts[1])))
    return rows


def flashes(rows: list[tuple[float, float]]) -> list[float]:
    """每次闪光的亮度质心时刻。

    不取「第一张变亮的帧」或越过阈值的时刻：插帧把一张白帧摊成前后几张
    半亮的过渡帧（对称的），阈值会让插帧那段系统性地早；质心不受影响。
    """
    lo = min(y for _, y in rows)
    hi = max(y for _, y in rows)
    gate = lo + (hi - lo) * 0.1
    out = []
    pulse: list[tuple[float, float]] = []
    for t, y in rows + [(float("inf"), lo)]:
        if y > gate:
            pulse.append((t, y - lo))
        elif pulse:
            weight = sum(w for _, w in pulse)
            out.append(sum(t * w for t, w in pulse) / weight)
            pulse = []
    return out


def beeps(path: str) -> list[float]:
    """哔声起点：silencedetect 的 silence_end。"""
    err = subprocess.run(
        [
            "ffmpeg", "-v", "info", "-i", path, "-vn",
            "-af", "highpass=f=600,lowpass=f=1500,silencedetect=n=-30dB:d=0.3",
            "-f", "null", "-",
        ],
        capture_output=True, text=True,
    ).stderr
    out = []
    for line in err.splitlines():
        if "silence_end:" in line:
            out.append(float(line.split("silence_end:")[1].split("|")[0]))
    return out


def measure(path: str) -> None:
    fl = flashes(brightness(path))
    bp = beeps(path)
    if not fl or not bp:
        sys.exit(f"闪光 {len(fl)} 次、哔声 {len(bp)} 次，量不了")
    diffs = []
    for f in fl:
        b = min(bp, key=lambda b: abs(b - f))
        if abs(b - f) < 0.4:
            diffs.append((b - f) * 1000)
    for i, d in enumerate(diffs):
        print(f"{i:3d}  声 − 画 {d:+7.1f} ms")
    if not diffs:
        sys.exit("闪光与哔声配不上对（相差都超过 400 ms）")
    print(
        f"n={len(diffs)} · 中位数 {statistics.median(diffs):+.1f} ms · "
        f"范围 {min(diffs):+.1f} … {max(diffs):+.1f} ms"
    )


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("make")
    m.add_argument("out")
    m.add_argument("--fps", type=int, default=30)
    m.add_argument("--seconds", type=int, default=60)
    q = sub.add_parser("measure")
    q.add_argument("video")
    a = ap.parse_args()
    if a.cmd == "make":
        make(a.out, a.fps, a.seconds)
    else:
        measure(a.video)


if __name__ == "__main__":
    main()
