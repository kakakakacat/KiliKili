#!/usr/bin/env python3
"""只驱动独立测试包，通过真实 Android 生命周期完成验收。"""
import json
import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parents[2]
adb = os.environ.get('ADB', 'adb')
device = os.environ['ANDROID_SERIAL']
pkg = 'io.github.kilikili.playerportprobe.kilikili'

def run(*args, check=True):
    return subprocess.run([adb, '-s', device, *args], capture_output=True, check=check).stdout

# 运行边界必须由驱动建立，不能把同一 PID 的旧 PASS 当成本轮证据。
run('shell', 'am', 'force-stop', pkg)
since = run('shell', 'date', '+%s').decode().strip() + '.000'
run('shell', 'am', 'start', '-n', pkg + '/.MainActivity')
end = time.monotonic() + 90
background_done = False
pip_done = False
seen = set()
rows = []
while time.monotonic() < end:
    pid = run('shell', 'pidof', pkg, check=False).decode().strip()
    if not pid:
        time.sleep(.5)
        continue
    log = run('logcat', '-d', '--pid=' + pid, '-T', since, '-s', 'flutter:I').decode(errors='replace')
    for line in log.splitlines():
        if 'PLAYER_PROBE {' not in line:
            continue
        raw = line.split('PLAYER_PROBE ', 1)[1]
        if raw in seen:
            continue
        seen.add(raw)
        rows.append(json.loads(raw))
        print(raw, flush=True)
    actions = {r['action'] for r in rows}
    if 'WAIT_BACKGROUND' in actions and not background_done:
        background_done = True
        run('shell', 'am', 'start', '-a', 'android.intent.action.MAIN', '-c', 'android.intent.category.HOME')
        time.sleep(2)
        run('shell', 'am', 'start', '-n', pkg + '/.MainActivity')
    if 'PIP_PLAYING' in actions and not pip_done:
        pip_done = True
        (root / 'build/player-port-android-pip.png').write_bytes(run('exec-out', 'screencap', '-p'))
        run('shell', 'am', 'start', '-n', pkg + '/.MainActivity')
    if 'PASS' in actions or 'FAIL' in actions:
        (root / 'build/player-port-android.png').write_bytes(run('exec-out', 'screencap', '-p'))
        break
    time.sleep(.5)
(root / 'build/player-port-android.json').write_text(json.dumps(rows, ensure_ascii=False, indent=2))
actions = {row['action'] for row in rows}
raise SystemExit(0 if 'PASS' in actions and 'FAIL' not in actions else 1)
