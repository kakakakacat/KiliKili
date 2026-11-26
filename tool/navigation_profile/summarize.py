#!/usr/bin/env python3
"""按 Flutter frameNumber 关联首次绘制和 Raster 完成，不把采样回调延迟算成渲染。"""
import argparse
import json
from pathlib import Path
import statistics

p = argparse.ArgumentParser()
p.add_argument('directory', type=Path)
p.add_argument('--rounds', type=int, default=5)
a = p.parse_args()
rows = []
for path in sorted(a.directory.glob('*.jsonl')):
    events = [json.loads(line) for line in path.read_text().splitlines()]
    if not events or events[-1]['event'] != 'done' or any(r['event'] in ['timeout', 'interrupted'] for r in events):
        raise ValueError(f'{path}: 测量未成功完成')
    samples = [r for r in events if r['event'] == 'sample']
    if len(samples) != 3:
        raise ValueError(f'{path}: 样本数不是3')
    frames = {r['frame']: r for r in events if r['event'] == 'frame'}
    for sample in [r for r in events if r['event'] == 'sample']:
        terminal = [r for r in events if r['sample'] == sample['sample'] and r['event'] == 'data_terminal']
        if len(terminal) != 1:
            raise ValueError(f'{path}: 数据终态数量不是1')
        subset = [r for r in events if r['sample'] == sample['sample'] and sample['us'] <= r['us'] <= terminal[0]['us']]
        def event(name):
            matches = [r for r in subset if r['event'] == name]
            if len(matches) != 1:
                raise ValueError(f'{path}: {name} 数量不是1')
            return matches[0]
        start = event('callback')['us']
        first = event('first_paint')
        full = event('full_paint')
        frame = frames[first['frame']]
        full_frame = frames[full['frame']]
        row = {
            'file': path.name, 'sample': sample['sample'], 'page': sample['name'],
            'login': sample['login'], 'first': sample['first'],
            'callback_us': start, 'first_frame': first['frame'],
            'first_frame_raster_finish_us': frame['raster_finish_us'],
            'full_frame_raster_finish_us': full_frame['raster_finish_us'],
            'selected_ms': (event('selected')['us'] - start) / 1000,
            'paint_ms': (first['us'] - start) / 1000,
            'raster_finish_ms': (frame['raster_finish_us'] - start) / 1000,
            'full_raster_ms': (full_frame['raster_finish_us'] - start) / 1000,
            'frame_build_ms': frame['build_us'] / 1000,
            'frame_raster_ms': frame['raster_us'] / 1000,
            'data_terminal_ms': (event('data_terminal')['us'] - start) / 1000,
            'data_state': event('data_terminal')['state'],
        }
        if row['raster_finish_ms'] < 0:
            raise ValueError('时间基准不一致，停止汇总')
        if row['data_state'].startswith('Loading'):
            raise ValueError(f'{path}: 数据仍在加载')
        row['requests'] = [r for r in subset if r['event'] == 'request_result']
        rows.append(row)
for page in ['dynamics', 'mine']:
    for first, count in [(True, a.rounds), (False, a.rounds * 2)]:
        actual = sum(r['page'] == page and r['first'] == first for r in rows)
        if actual != count:
            raise ValueError(f'{page}: 应有{count}条，实际{actual}条')
(a.directory / 'samples.json').write_text(json.dumps(rows, indent=2))
for page in ['dynamics', 'mine']:
    for first in [True, False]:
        selected = [r for r in rows if r['page'] == page and r['first'] == first]
        if not selected:
            continue
        print(page, '首次' if first else '回访', f'n={len(selected)}')
        for field in ['selected_ms', 'paint_ms', 'raster_finish_ms', 'full_raster_ms', 'frame_build_ms', 'frame_raster_ms', 'data_terminal_ms']:
            values = [r[field] for r in selected]
            print(f'  {field}: p50={statistics.median(values):.2f}, min={min(values):.2f}, max={max(values):.2f}')
