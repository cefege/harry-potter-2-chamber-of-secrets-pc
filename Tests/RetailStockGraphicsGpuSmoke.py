#!/usr/bin/env python3
"""Opt-in retail VSync/anisotropy UI replay; requires calibrated input scripts.

No fixture coordinates are assumed. Calibrate and visually inspect both replay
scripts on the target display before use. This test requires the engine's
StockGraphicsGPU probe to report actual effective GPU state, not just config.
Never use a personal retail data tree. See Fixtures/retail_stock_graphics_gpu.md.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import sys

import InputScriptSmoke as replay
from RetailStockSettingsSmoke import identity, read_value, set_ini
from RetailStockFpsSmoke import sha256

ROOT = Path(__file__).resolve().parent.parent
PROBE = re.compile(r'StockGraphicsGPU event=(created|applied) setting=(vsync|anisotropy|msaa) requested=(on|off|adaptive|0|2|4|8|16) actual=(on|off|adaptive|0|2|4|8|16)')

def probes(log: str) -> list[dict[str, str]]:
    return [dict(zip(('event', 'setting', 'requested', 'actual'), fields)) for fields in PROBE.findall(log)]

def matching(rows: list[dict[str, str]], event: str, setting: str, value: str) -> bool:
    return any(row == dict(event=event, setting=setting, requested=value, actual=value) for row in rows)

def run_phase(name: str, fixture_path: Path, checkpoints: list[int], engine: Path, expected_hash: str,
              data: Path, home: Path, user: Path, artifacts: Path) -> dict:
    phase = artifacts / name
    phase.mkdir()
    if sha256(engine) != expected_hash:
        raise RuntimeError(f'{name}: engine hash changed before launch')
    fixture = fixture_path.read_text(encoding='utf-8')
    tick, frames = replay.parse_fixture(fixture)
    replay.validate_launch_safe_frames(frames)
    if not checkpoints or max(checkpoints) >= len(frames):
        raise ValueError(f'{name}: checkpoint outside fixture')
    (user / f'{replay.REPLAY_STEM}.rep').write_bytes(replay.compile_replay('PrivetDr.unr', tick, frames))
    command = replay.build_launch_command(engine, data, len(frames) + 40)
    command.append(f'-INI="{user / "Game.ini"}"')
    (phase / 'fixture.txt').write_text(fixture, encoding='utf-8')
    (phase / 'command.json').write_text(json.dumps(command, indent=2) + '\n', encoding='utf-8')
    os.environ['HP2_CAPTURE_FRAMES'] = str(phase / 'frames')
    os.environ['HP2_CAPTURE_FRAME_INDICES'] = ','.join(map(str, checkpoints))
    os.environ['HP2_STOCK_GRAPHICS_GPU_TRACE'] = '1'
    os.environ['HP2_ARTIFACT_DIR'] = str(artifacts)
    os.environ['SDL_MAC_BACKGROUND_APP'] = '1'
    os.environ['XDG_DATA_HOME'] = str(home / '.local/share')
    output, status, timeout, error = replay._supervised_run(command, home=home, cwd=ROOT, timeout_seconds=180)
    (phase / 'run.log').write_bytes(output)
    log = output.decode('utf-8', errors='replace')
    shots = [phase / 'frames' / f'frame_{i:06}.png' for i in checkpoints]
    result = {'exit': status, 'timed_out': timeout, 'launch_error': error,
              'engine_sha256_after': sha256(engine), 'probe': probes(log),
              'vsync': read_value(user / 'Game.ini', 'WgpuDrv.WgpuRenderDevice', 'UseVSync'),
              'anisotropy': read_value(user / 'Game.ini', 'WgpuDrv.WgpuRenderDevice', 'MaxAnisotropy'),
              'adapter_errors': [line for line in log.splitlines() if any(marker in line for marker in
                  ('Graphics setting failed:', 'Stock FPS menu:', 'Cannot apply persisted', 'Saved anti-aliasing unavailable:', 'Graphics menu tick:'))
                  or ('CutError:' in line and not line.startswith('Destroying '))],
              'failure_markers': replay.game_test._failure_markers(output, ROOT, home),
              'shots': [str(p) for p in shots if p.is_file() and p.stat().st_size]}
    (phase / 'result.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    return result

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('data-root', 'inventory', 'engine-bin', 'set-fixture', 'restart-fixture'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--engine-sha256', required=True)
    parser.add_argument('--set-checkpoints', required=True, help='comma-separated tick indices')
    parser.add_argument('--restart-checkpoints', required=True, help='comma-separated tick indices')
    parser.add_argument('--vsync', choices=('on', 'off'), required=True)
    parser.add_argument('--anisotropy', choices=('0', '4', '8', '16'), required=True)
    parser.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    parser.add_argument('--allow-window', action='store_true')
    args = parser.parse_args()
    if not args.allow_window or args.artifact_dir is None:
        parser.error('--allow-window and --artifact-dir are required')
    try:
        set_checkpoints = [int(v) for v in args.set_checkpoints.split(',')]
        restart_checkpoints = [int(v) for v in args.restart_checkpoints.split(',')]
        if min(set_checkpoints + restart_checkpoints) < 0: raise ValueError()
    except ValueError:
        parser.error('checkpoints must be nonnegative comma-separated integers')
    data, engine, inventory, fixture, restart = (p.resolve(strict=True) for p in
        (args.data_root, args.engine_bin, args.inventory, args.set_fixture, args.restart_fixture))
    artifacts = Path(args.artifact_dir).resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts):
        parser.error('data and artifacts must be separate trees')
    if data.is_relative_to(Path.home().resolve() / 'Library/Application Support/Harry Potter 2'):
        parser.error('use a disposable retail copy')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()): parser.error('artifact directory must be empty')
    expected = args.engine_sha256.lower()
    if not re.fullmatch(r'[0-9a-f]{64}', expected): parser.error('invalid engine hash')
    if sha256(engine) != expected:
        print('blocked: engine hash mismatch', file=sys.stderr); return 3
    before = identity(data, inventory)
    (artifacts / 'inventory-before.json').write_text(json.dumps(before, indent=2), encoding='utf-8')
    if not all(row['matches'] for row in before):
        print('blocked: retail inventory mismatch', file=sys.stderr); return 3
    # Validate both scripts before starting either game process.
    for path, checkpoints in ((fixture, set_checkpoints), (restart, restart_checkpoints)):
        _, frames = replay.parse_fixture(path.read_text(encoding='utf-8'))
        replay.validate_launch_safe_frames(frames)
        if max(checkpoints) >= len(frames): parser.error(f'checkpoint outside {path}')
    tick, frames = replay.parse_fixture(fixture.read_text(encoding='utf-8'))
    home = artifacts / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, frames))
    ini = user / 'Game.ini'
    text = ini.read_text(encoding='utf-8')
    # Distinct starting values make a persisted result meaningful.
    initial_vsync = 'Off' if args.vsync == 'on' else 'On'
    initial_anisotropy = '4' if args.anisotropy == '0' else '0'
    for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
            ('SDLDrv.SDLClient', 'WindowedViewportX', '800'), ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
            ('WgpuDrv.WgpuRenderDevice', 'UseVSync', initial_vsync), ('WgpuDrv.WgpuRenderDevice', 'MaxAnisotropy', initial_anisotropy),
            ('Core.System', 'SavePath', str(user / 'Save')), ('Core.System', 'CachePath', str(user / 'Cache'))]:
        text = set_ini(text, section, key, value)
    ini.write_text(text, encoding='utf-8')
    phases = {'set': run_phase('set', fixture, set_checkpoints, engine, expected, data, home, user, artifacts)}
    failures = []
    first = phases['set']
    if first['exit'] == 0 and not first['timed_out'] and not first['launch_error']:
        phases['restart'] = run_phase('restart', restart, restart_checkpoints, engine, expected, data, home, user, artifacts)
    else:
        failures.append('set phase failed; restart was not run')
    for name, result in phases.items():
        if result['exit'] != 0 or result['timed_out'] or result['launch_error']: failures.append(f'{name}: process failed')
        if result['engine_sha256_after'] != expected: failures.append(f'{name}: engine changed')
        if (result['vsync'] or '').lower() != args.vsync: failures.append(f'{name}: VSync config mismatch')
        try:
            if float(result['anisotropy']) != int(args.anisotropy): failures.append(f'{name}: anisotropy config mismatch')
        except (TypeError, ValueError): failures.append(f'{name}: invalid anisotropy config')
        if result['failure_markers'] or result['adapter_errors']: failures.append(f'{name}: engine errors')
        checks = set_checkpoints if name == 'set' else restart_checkpoints
        if len(result['shots']) != len(checks): failures.append(f'{name}: screenshots missing')
        event = 'applied' if name == 'set' else 'created'
        for setting, value in (('vsync', args.vsync), ('anisotropy', args.anisotropy)):
            if not matching(result['probe'], event, setting, value): failures.append(f'{name}: missing effective {setting} GPU probe')
    after = identity(data, inventory)
    (artifacts / 'inventory-after.json').write_text(json.dumps(after, indent=2), encoding='utf-8')
    if before != after or not all(row['matches'] for row in after): failures.append('retail copy changed')
    report = {'status': 'fail' if failures else 'pass', 'data_profile': 'data-retail', 'phases': phases,
              'engine_sha256': expected, 'inventory_unchanged': before == after, 'failures': failures,
              'limitations': ['screenshots require visual review', 'synthetic input is not physical input',
                              'inventory does not prove original-media authenticity', 'audio not tested']}
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    return 1 if failures else 0

if __name__ == '__main__':
    raise SystemExit(main())
