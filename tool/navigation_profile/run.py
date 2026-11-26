#!/usr/bin/env python3
"""自动冷进程采样；仅操作独立 profile 包，不清除数据或日志。"""
import argparse
import json
from pathlib import Path
import subprocess
import queue
import threading
import time

p = argparse.ArgumentParser()
p.add_argument('--output', type=Path, required=True)
p.add_argument('--rounds', type=int, default=5)
p.add_argument('--expect-login', choices=['yes', 'no'])
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
package = 'io.github.kakacat.kilikili.navprofile'
activity = package + '/io.github.kakacat.kilikili.MainActivity'
for target in ['dynamics', 'mine']:
    for round_number in range(a.rounds):
        subprocess.run(['adb', 'shell', 'am', 'force-stop', package], check=True, stdout=subprocess.DEVNULL)
        subprocess.run(['adb', 'shell', 'am', 'start', '-n', activity, '--es', 'nav_target', target], check=True, stdout=subprocess.DEVNULL)
        # logcat 包含启动前的环形缓存，首个 ready 的时间必须晚于本次进程。
        pid = ''
        for _ in range(50):
            pid = subprocess.run(['adb', 'shell', 'pidof', package], text=True, capture_output=True).stdout.strip()
            if pid:
                break
            time.sleep(0.1)
        if not pid:
            raise RuntimeError('独立 profile 进程没有启动')
        log = subprocess.Popen(['adb', 'logcat', f'--pid={pid}', '-v', 'raw', 'flutter:I', '*:S'], stdout=subprocess.PIPE, text=True)
        lines = queue.Queue()
        def read_lines():
            for line in log.stdout:
                lines.put(line)
            lines.put(None)
        threading.Thread(target=read_lines, daemon=True).start()
        records = []
        failure = None
        try:
            deadline = time.monotonic() + 100
            while time.monotonic() < deadline:
                try:
                    line = lines.get(timeout=1)
                except queue.Empty:
                    continue
                if line is None:
                    raise RuntimeError('logcat 提前退出')
                if not line.startswith('NAV_PROFILE '):
                    continue
                record = json.loads(line.removeprefix('NAV_PROFILE '))
                records.append(record)
                if record['event'] == 'ready' and a.expect_login is not None and record['login'] != (a.expect_login == 'yes'):
                    raise RuntimeError('实际登录态与本轮要求不一致，停止采样')
                if record['event'] in ['done', 'timeout', 'interrupted']:
                    break
        except Exception as error:
            failure = error
        finally:
            log.terminate()
            log.wait()
        path = a.output / f'{target}-{round_number + 1}.jsonl'
        path.write_text(''.join(json.dumps(r) + '\n' for r in records))
        if failure is not None:
            raise failure
        if not records or records[-1]['event'] != 'done':
            raise TimeoutError(f'{target}: 未观察到测量结束，不能报告成功，原始日志已保存')
        print(f'{target} {round_number + 1}/{a.rounds}: {len(records)} 条，{path}', flush=True)
        time.sleep(1)
