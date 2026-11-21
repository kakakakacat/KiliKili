"""给 SPIKE-002 的后台音频那一项造一条**带声音**的轨。

为什么要造：手上那份 probe.mp4 **没有音频轨**（mpv 日志里
`playback restart complete @ 0.000000, audio=eof`），拿它测「后台还出不出声」
等于拿一段默片去证明音响坏没坏。本机又没有 ffmpeg，所以用标准库写一个 WAV。

声音是故意设计过的：每秒换一个音高、每秒开头一声短促的「嘀」。这样**用耳朵
就能判断时间在不在走**——后台播放这一项最后总要有人真的听一次，日志里的
`audio-pts` 只是它的机器化替身。

    python make-probe-audio.py probe-audio.wav

然后跟 probe.mp4 一起推到设备上；探针看到它就按 DASH 分轨开
（`MediaSource::Dash`，跟真实的 B 站取流是同一条路径）。
"""

import math
import struct
import sys
import wave

RATE = 24000
SECONDS = 180  # 比 probe.mp4（约 30s）长得多：音轨先结束的话测的就是另一回事
AMPLITUDE = 11000


def samples():
    for index in range(RATE * SECONDS):
        second = index // RATE
        phase_in_second = index % RATE
        # 每秒换一个音高，十秒一循环：听一遍就知道走到哪儿了。
        freq = 320 + (second % 10) * 55
        value = math.sin(2 * math.pi * freq * index / RATE)
        # 每秒头 60ms 加一声「嘀」，给耳朵一个明确的节拍。
        if phase_in_second < RATE * 0.06:
            value = 0.6 * value + 0.4 * math.sin(2 * math.pi * 1760 * index / RATE)
        yield int(AMPLITUDE * value)


def main() -> None:
    out = sys.argv[1] if len(sys.argv) > 1 else "probe-audio.wav"
    with wave.open(out, "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(RATE)
        handle.writeframes(b"".join(struct.pack("<h", s) for s in samples()))
    print(f"写好了 {out}：{SECONDS}s / {RATE}Hz / 单声道")


if __name__ == "__main__":
    main()
