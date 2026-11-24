#!/usr/bin/env python3
"""CoreDevice 驱动独立 profile 探针；profile 的 Dart 日志从沙盒文件读取。"""
import json
import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parents[2]
device = os.environ['IOS_DEVICE']
pkg = 'io.github.kilikili.playerportprobe.kilikili'

def run(*args):
    return subprocess.run(['xcrun', 'devicectl', 'device', *args[:2], '--device', device, *args[2:]], check=True, capture_output=True).stdout

def files():
    data = run('info', 'files', '--domain-type', 'appDataContainer', '--domain-identifier', pkg,
               '--subdirectory', 'tmp', '--search', 'events.jsonl', '--json-output', '-')
    return json.loads(data)['result']['files']

old = {row['relativePath'] for row in files()}
run('process', 'launch', '--terminate-existing', pkg)
end = time.monotonic() + 120
rows = []
seen = set()
source = None
background_done = False
pip_done = False
while time.monotonic() < end:
    if source is None:
        choices = [row for row in files() if row['relativePath'] not in old]
        if not choices:
            time.sleep(.5)
            continue
        source = 'tmp/' + choices[-1]['relativePath']
    local = root / 'build/player-port-ios-events.jsonl'
    run('copy', 'from', '--domain-type', 'appDataContainer', '--domain-identifier', pkg,
        '--source', source, '--destination', str(local))
    for line in local.read_text().splitlines():
        if line in seen:
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        seen.add(line)
        rows.append(row)
        print(line, flush=True)
    actions = {r['action'] for r in rows}
    if 'WAIT_BACKGROUND' in actions and not background_done:
        background_done = True
        run('process', 'launch', 'com.apple.Preferences')
        time.sleep(2)
        run('process', 'launch', pkg)
    if 'PIP_PLAYING' in actions and not pip_done:
        pip_done = True
        run('capture', 'screenshot', '--destination', str(root / 'build/player-port-ios-pip.png'))
        run('process', 'launch', pkg)
    if 'PASS' in actions or 'FAIL' in actions:
        run('capture', 'screenshot', '--destination', str(root / 'build/player-port-ios.png'))
        break
    time.sleep(.5)
(root / 'build/player-port-ios.json').write_text(json.dumps(rows, ensure_ascii=False, indent=2))
actions = {row['action'] for row in rows}
raise SystemExit(0 if 'PASS' in actions and 'FAIL' not in actions else 1)
