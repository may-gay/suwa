#!/usr/bin/env python3
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import shutil
import sqlite3
import time

parser = argparse.ArgumentParser()
parser.add_argument('--profile', type=Path)
parser.add_argument('--enhance', action='store_true')
parser.add_argument('--size', help='float only this test window at WIDTHxHEIGHT')
parser.add_argument('--view', choices=['connection', 'library', 'library-filters', 'series', 'series-edit', 'series-details', 'reader', 'reader-options', 'webtoon', 'settings', 'upscaling', 'categories'], default='connection')
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
suffix = ('-enhanced' if args.enhance else '') + (f'-{args.size}' if args.size else '')
output = root / '.local' / f'wayland-{args.view}{suffix}.png'
output.parent.mkdir(exist_ok=True)
env = os.environ.copy()
env['SUWA_DATA_DIR'] = tempfile.mkdtemp(prefix='suwa-ui-')
if args.profile:
    shutil.copytree(args.profile, env['SUWA_DATA_DIR'], dirs_exist_ok=True)
if args.enhance:
    if not args.profile:
        raise ValueError('--enhance requires an offline fixture profile')
    with sqlite3.connect(Path(env['SUWA_DATA_DIR']) / 'suwa.sqlite3') as database:
        settings = json.loads(database.execute("SELECT value FROM kv WHERE scope='app' AND key='settings'").fetchone()[0])
        settings['upscale'] = True
        database.execute("UPDATE kv SET value=? WHERE scope='app' AND key='settings'", (json.dumps(settings),))
env['SUWA_CAPTURE_VIEW'] = args.view
capture = Path(env['SUWA_DATA_DIR']) / 'capture.png'
env['SUWA_CAPTURE'] = str(capture)
started = time.monotonic()
with (output.parent / 'wayland-smoke.log').open('w') as log:
    process = subprocess.Popen([str(root / 'scripts/run.sh')], cwd=root, env=env, stdout=log, stderr=log)
    try:
        client = None
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError('Suwa exited; inspect .local/wayland-smoke.log')
            clients = json.loads(subprocess.check_output(['hyprctl', 'clients', '-j']))
            client = next((c for c in clients if c['pid'] == process.pid and c['mapped']), None)
            if client:
                break
            time.sleep(0.1)
        if client is None:
            raise RuntimeError('Suwa did not map a Wayland window')
        elapsed = time.monotonic() - started
        if args.size:
            width, height = (int(value) for value in args.size.split('x'))
            address = f"address:{client['address']}"
            floated = subprocess.run(['hyprctl', 'dispatch', 'setfloating', address], capture_output=True)
            if floated.returncode == 0:
                subprocess.run(['hyprctl', 'dispatch', 'resizewindowpixel', f'exact {width} {height},{address}'], check=True, stdout=subprocess.DEVNULL)
            else:
                selector = json.dumps(address)
                code = (
                    f'hl.dispatch(hl.dsp.window.float({{action="set", window={selector}}}))\n'
                    f'hl.dispatch(hl.dsp.window.resize({{x={width}, y={height}, window={selector}}}))'
                )
                subprocess.run(['hyprctl', 'eval', code], check=True, stdout=subprocess.DEVNULL)

        for _ in range(300):
            if process.poll() is not None:
                raise RuntimeError('Suwa exited before capture')
            if capture.exists() and capture.stat().st_size > 0:
                time.sleep(0.2)
                break
            time.sleep(0.1)
        shutil.copyfile(capture, output)
        rss = next(line.split()[1] for line in Path(f'/proc/{process.pid}/status').read_text().splitlines() if line.startswith('VmRSS:'))
        memory = {line.split(':')[0]: int(line.split()[1]) for line in Path(f'/proc/{process.pid}/smaps_rollup').read_text().splitlines()[1:] if len(line.split()) == 3}
        metrics = {'pss_mib': round(memory.get('Pss',0)/1024,1), 'backend':env.get('WGPU_BACKEND','auto'), 'mapped_ms': round(elapsed * 1000), 'idle_rss_mib': round(int(rss) / 1024, 1), 'xwayland': client['xwayland'], 'screenshot': str(output)}
        output.with_suffix('.json').write_text(json.dumps(metrics, indent=2) + '\n')
        print(json.dumps(metrics))
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        shutil.rmtree(env['SUWA_DATA_DIR'])

