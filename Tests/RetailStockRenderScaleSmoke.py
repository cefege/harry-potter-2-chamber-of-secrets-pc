#!/usr/bin/env python3
"""Windowed retail stock-slider replay, with live and restart GPU evidence.

Requires an explicit immutable retail copy, TSV size/SHA-256 inventory, stable
private engine binary and a fresh artifact directory. Input is synthetic and
PNG frames require visual review. This does not prove physical input, audio,
desktop presentation, or original-media authenticity.
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
FIXTURES = {
    'set': ROOT / 'Tests/Fixtures/retail_stock_render_scale_set.txt',
    'restart': ROOT / 'Tests/Fixtures/retail_stock_render_scale_restart.txt',
}
CHECKPOINTS = {'set': [220, 315, 550], 'restart': [120, 260]}
SCALE = re.compile(r'StockScaleProbe event=(\w+) percent=(\d+) unavailable=(true|false)')
GPU = re.compile(r'world-scale trace scopes=(\d+) prior_color=(\d+) scaled=(\d+) unavailable=(\d+) requested=(\d+)%')


def probe(log: str) -> list[dict]:
    return [dict(event=e, percent=int(p), unavailable=u == 'true') for e, p, u in SCALE.findall(log)]


def gpu_trace(log: str) -> list[dict]:
    return [dict(scopes=int(s), prior_color=int(p), scaled=int(c), unavailable=int(u), requested=int(r))
            for s, p, c, u, r in GPU.findall(log)]


def run_phase(name: str, engine: Path, expected_hash: str, data: Path, home: Path, user: Path, artifacts: Path) -> dict:
    phase = artifacts / name
    phase.mkdir()
    if sha256(engine) != expected_hash:
        raise RuntimeError(f'{name}: engine hash changed before launch')
    fixture = FIXTURES[name].read_text(encoding='utf-8')
    tick, frames = replay.parse_fixture(fixture)
    replay.validate_launch_safe_frames(frames)
    (user / f'{replay.REPLAY_STEM}.rep').write_bytes(replay.compile_replay('PrivetDr.unr', tick, frames))
    command = replay.build_launch_command(engine, data, len(frames) + 40)
    command.append(f'-INI="{user / "Game.ini"}"')
    (phase / 'fixture.txt').write_text(fixture, encoding='utf-8')
    (phase / 'command.json').write_text(json.dumps(command, indent=2) + '\n', encoding='utf-8')
    os.environ['HP2_CAPTURE_FRAMES'] = str(phase / 'frames')
    os.environ['HP2_CAPTURE_FRAME_INDICES'] = ','.join(map(str, CHECKPOINTS[name]))
    os.environ['HP2_STOCK_SCALE_TRACE'] = '1'
    os.environ['HP2_WORLD_SCALE_TRACE'] = '1'
    os.environ['HP2_ARTIFACT_DIR'] = str(artifacts)
    os.environ['SDL_MAC_BACKGROUND_APP'] = '1'
    os.environ['XDG_DATA_HOME'] = str(home / '.local/share')
    output, status, timeout, error = replay._supervised_run(command, home=home, cwd=ROOT, timeout_seconds=180)
    (phase / 'run.log').write_bytes(output)
    log = output.decode('utf-8', errors='replace')
    shots = [phase / 'frames' / f'frame_{i:06}.png' for i in CHECKPOINTS[name]]
    result = {'exit': status, 'timed_out': timeout, 'launch_error': error,
              'engine_sha256_after': sha256(engine),
              'stored_scale': read_value(user / 'Game.ini', 'WgpuDrv.WgpuRenderDevice', 'RenderScale'),
              'slider': probe(log), 'gpu': gpu_trace(log),
              'adapter_errors': [line for line in log.splitlines() if 'Stock FPS menu:' in line or 'Render scale menu tick:' in line],
              'cut_errors': [line for line in log.splitlines() if 'CutError:' in line and not line.startswith('Destroying ')],
              'failure_markers': replay.game_test._failure_markers(output, ROOT, home),
              'shots': [str(p) for p in shots if p.is_file() and p.stat().st_size]}
    (phase / 'result.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-root', type=Path, required=True)
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--engine-bin', type=Path, required=True)
    parser.add_argument('--engine-sha256', required=True)
    parser.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    parser.add_argument('--allow-window', action='store_true')
    args = parser.parse_args()
    if not args.allow_window or args.artifact_dir is None:
        parser.error('--allow-window and --artifact-dir (or HP2_ARTIFACT_DIR) are required')
    data, engine, inventory = (p.resolve(strict=True) for p in (args.data_root, args.engine_bin, args.inventory))
    artifacts = Path(args.artifact_dir).resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts):
        parser.error('data and artifacts must be separate trees')
    if data.is_relative_to(Path.home().resolve() / 'Library/Application Support/Harry Potter 2'):
        parser.error('use a disposable retail copy')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()): parser.error('artifact directory must be empty')
    expected = args.engine_sha256.lower()
    if not re.fullmatch(r'[0-9a-f]{64}', expected): parser.error('--engine-sha256 must be 64 hex digits')
    if sha256(engine) != expected:
        print('blocked: engine hash does not match', file=sys.stderr)
        return 3
    before = identity(data, inventory)
    (artifacts / 'inventory-before.json').write_text(json.dumps(before, indent=2), encoding='utf-8')
    if not all(row['matches'] for row in before):
        print('blocked: retail inventory mismatch', file=sys.stderr)
        return 3
    tick, frames = replay.parse_fixture(FIXTURES['set'].read_text(encoding='utf-8'))
    home = artifacts / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, frames))
    ini = user / 'Game.ini'
    text = ini.read_text(encoding='utf-8')
    for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
                                ('SDLDrv.SDLClient', 'WindowedViewportX', '800'),
                                ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
                                ('WgpuDrv.WgpuRenderDevice', 'RenderScale', '1.0'),
                                ('Core.System', 'SavePath', str(user / 'Save')),
                                ('Core.System', 'CachePath', str(user / 'Cache'))]:
        text = set_ini(text, section, key, value)
    ini.write_text(text, encoding='utf-8')
    phases = {'set': run_phase('set', engine, expected, data, home, user, artifacts)}
    if phases['set']['exit'] == 0 and phases['set']['stored_scale'] == '0.5':
        phases['restart'] = run_phase('restart', engine, expected, data, home, user, artifacts)
    failures = []
    if 'restart' not in phases: failures.append('50% not persisted; dependent restart was not run')
    for name, result in phases.items():
        if result['exit'] != 0 or result['timed_out'] or result['launch_error']: failures.append(f'{name}: process failed')
        if result['engine_sha256_after'] != expected: failures.append(f'{name}: executable changed')
        if result['stored_scale'] != '0.5': failures.append(f'{name}: config is not 50%')
        if result['failure_markers'] or result['cut_errors'] or result['adapter_errors']: failures.append(f'{name}: engine errors observed')
        if len(result['shots']) != len(CHECKPOINTS[name]): failures.append(f'{name}: screenshot checkpoints missing')
        if not any(row['requested'] == 50 and row['scaled'] > 0 for row in result['gpu']): failures.append(f'{name}: GPU trace lacks applied 50% world scope')
    if not any(row['event'] == 'changed' and row['percent'] == 50 for row in phases['set']['slider']):
        failures.append('live slider did not report a 50% change')
    if 'restart' in phases:
        restart = phases['restart']
        if not any(row['event'] == 'created' and row['percent'] == 50 for row in restart['slider']): failures.append('reopened slider did not show 50%')
        if not restart['gpu'] or restart['gpu'][0]['requested'] != 50: failures.append('first startup GPU scope did not request persisted 50%')
    after = identity(data, inventory)
    (artifacts / 'inventory-after.json').write_text(json.dumps(after, indent=2), encoding='utf-8')
    if before != after or not all(row['matches'] for row in after): failures.append('retail copy changed')
    report = {'status': 'fail' if failures else 'pass', 'data_profile': 'data-retail',
              'engine_bin': str(engine), 'engine_sha256': expected, 'inventory_count': len(before),
              'inventory_unchanged': before == after, 'phases': phases, 'failures': failures,
              'limitations': ['screenshots require visual review', 'replay is not physical input',
                              'audio output not tested', 'inventory does not prove original-media authenticity']}
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, indent=2))
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
