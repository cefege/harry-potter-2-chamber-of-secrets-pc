#!/usr/bin/env python3
"""Exercise stock retail FPS control across page changes and process restarts.

Uses synthetic input and an isolated user home. Requires a disposable retail
copy, a TSV inventory, and permission to open SDL windows. Frames still need
visual review; the replay is not physical input.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys

import InputScriptSmoke as replay
from RetailStockSettingsSmoke import identity, read_value, set_ini

ROOT = Path(__file__).resolve().parent.parent
OPEN = """tick 0.033333335
idle 60
press Enter
idle 40
press Escape
idle 80
mouse 680 455
idle 4
press LeftMouse
idle 65
idle 2
idle 3
"""
ENABLE_REOPEN = OPEN + """mouse 483 428
idle 4
press LeftMouse
idle 15
idle 2
idle 3
press Tab
idle 20
mouse 755 555
idle 4
press LeftMouse
idle 50
mouse 680 455
idle 4
press LeftMouse
idle 55
idle 2
idle 3
"""
DISABLE = OPEN + """mouse 483 428
idle 4
press LeftMouse
idle 65
idle 2
idle 3
"""
CHECK_OFF = OPEN + "idle 40\n"
PROBE = re.compile(r'StockFPSProbe tick=(\d+) page=(\d+) control=(\d+) visible=(true|false) checked=(true|false) focus=(true|false) fps=(true|false) count=(\d+)')


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def observations(log: str) -> list[dict]:
    rows = []
    for m in PROBE.finditer(log):
        tick, page, control, visible, checked, focus, fps, count = m.groups()
        rows.append({'tick': int(tick), 'page': int(page), 'control': int(control), 'count': int(count),
                     'visible': visible == 'true', 'checked': checked == 'true',
                     'focus': focus == 'true', 'fps': fps == 'true'})
    return rows


def sequence(rows: list[dict], checks: list[dict]) -> bool:
    """Require matching states in order, without assuming particular ticks."""
    remaining = iter(rows)
    return all(any(all(row[key] == value for key, value in check.items()) for row in remaining) for check in checks)


def run_phase(name: str, fixture: str, engine: Path, data: Path, home: Path,
              user: Path, artifacts: Path, expected_hash: str) -> dict:
    phase = artifacts / name
    phase.mkdir()
    binary_before = sha256(engine)
    if binary_before != expected_hash:
        raise RuntimeError(f'{name}: executable changed before launch: {binary_before}')
    tick, frames = replay.parse_fixture(fixture)
    replay.validate_launch_safe_frames(frames)
    (user / f'{replay.REPLAY_STEM}.rep').write_bytes(replay.compile_replay('PrivetDr.unr', tick, frames))
    command = replay.build_launch_command(engine, data, len(frames) + 40)
    command.append(f'-INI="{user / "Game.ini"}"')
    (phase / 'fixture.txt').write_text(fixture)
    (phase / 'command.json').write_text(json.dumps(command, indent=2))
    # The renderer skips GPU readback for all other presented frames.
    checkpoints = {'enable_reopen': [245, 280, 300, 410], 'restart_disable': [245, 300], 'restart_off': [245]}[name]
    os.environ['HP2_CAPTURE_FRAMES'] = str(phase / 'frames')
    os.environ['HP2_CAPTURE_FRAME_INDICES'] = ','.join(map(str, checkpoints))
    os.environ['HP2_STOCK_FPS_TRACE'] = '1'
    os.environ['HP2_ARTIFACT_DIR'] = str(artifacts)
    os.environ['SDL_MAC_BACKGROUND_APP'] = '1'
    os.environ['XDG_DATA_HOME'] = str(home / '.local/share')
    output, status, timed_out, error = replay._supervised_run(command, home=home, cwd=ROOT, timeout_seconds=180)
    (phase / 'run.log').write_bytes(output)
    log = output.decode('utf-8', errors='replace')
    shots = [phase / 'frames' / f'frame_{index:06}.png' for index in checkpoints]
    result = {'process_exit': status, 'timed_out': timed_out, 'launch_error': error,
              'binary_sha256_before': binary_before, 'binary_sha256_after': sha256(engine),
              'fps_stored': read_value(user / 'Game.ini', 'SDLDrv.SDLClient', 'ShowFPS'),
              'page_opens': log.count('ChangePage Transient.FESoundVideoPage'),
              'observations': observations(log),
              'adapter_errors': [line for line in log.splitlines() if 'Stock FPS menu:' in line],
              'cut_errors': [line for line in log.splitlines() if 'CutError:' in line and not line.startswith('Destroying ')],
              'failure_markers': replay.game_test._failure_markers(output, ROOT, home),
              'shots': [str(shot) for shot in shots if shot.is_file() and shot.stat().st_size]}
    (phase / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-root', type=Path, required=True)
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--engine-bin', type=Path, required=True)
    parser.add_argument('--engine-sha256', required=True, help='expected stable executable SHA-256')
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
        parser.error('use a disposable retail copy, not the user data tree')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()): parser.error('artifact directory must be empty')
    expected_hash = args.engine_sha256.lower()
    if not re.fullmatch(r'[0-9a-f]{64}', expected_hash): parser.error('--engine-sha256 must be 64 hexadecimal characters')
    initial_hash = sha256(engine)
    if initial_hash != expected_hash:
        print(f'blocked: executable SHA-256 mismatch: {initial_hash}', file=sys.stderr)
        return 3
    before = identity(data, inventory)
    (artifacts / 'inventory-before.json').write_text(json.dumps(before, indent=2))
    if not all(row['matches'] for row in before):
        print('blocked: retail copy does not match inventory', file=sys.stderr)
        return 3
    tick, frames = replay.parse_fixture(ENABLE_REOPEN)
    home = artifacts / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, frames))
    game_ini = user / 'Game.ini'
    text = game_ini.read_text(encoding='utf-8')
    for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
                                ('SDLDrv.SDLClient', 'WindowedViewportX', '800'),
                                ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
                                ('SDLDrv.SDLClient', 'ShowFPS', 'False'),
                                ('Core.System', 'SavePath', str(user / 'Save')),
                                ('Core.System', 'CachePath', str(user / 'Cache'))]:
        text = set_ini(text, section, key, value)
    game_ini.write_text(text, encoding='utf-8')
    phases = {}
    failures = []
    phases['enable_reopen'] = run_phase('enable_reopen', ENABLE_REOPEN, engine, data, home, user, artifacts, expected_hash)
    enabled = phases['enable_reopen']
    if enabled['process_exit'] == 0 and enabled['fps_stored'].lower() == 'true':
        phases['restart_disable'] = run_phase('restart_disable', DISABLE, engine, data, home, user, artifacts, expected_hash)
        disabled = phases['restart_disable']
        if disabled['process_exit'] == 0 and disabled['fps_stored'].lower() == 'false':
            phases['restart_off'] = run_phase('restart_off', CHECK_OFF, engine, data, home, user, artifacts, expected_hash)
    if len(phases) != 3: failures.append('dependent restart phases not reached')
    if enabled['page_opens'] < 2: failures.append('sound/video page was not reopened')
    if not sequence(enabled['observations'], [
        {'visible': True, 'checked': False, 'fps': False},
        {'visible': True, 'checked': True, 'fps': True, 'focus': True},
        {'visible': True, 'checked': True, 'fps': True, 'focus': False},
        {'visible': False, 'checked': True, 'fps': True},
        {'visible': True, 'checked': True, 'fps': True},
    ]): failures.append('enable, focus/tab, or reopen state sequence missing')
    if 'restart_disable' in phases:
        disabled = phases['restart_disable']
        if not sequence(disabled['observations'], [
            {'visible': True, 'checked': True, 'fps': True},
            {'visible': True, 'checked': False, 'fps': False},
        ]): failures.append('restart-on or immediate disable state missing')
    if 'restart_off' in phases and not sequence(phases['restart_off']['observations'], [
        {'visible': True, 'checked': False, 'fps': False},
    ]): failures.append('restart-off state missing')
    for name, result in phases.items():
        if result['process_exit'] != 0 or result['timed_out'] or result['launch_error']: failures.append(f'{name}: process failed or timed out')
        if result['binary_sha256_after'] != expected_hash: failures.append(f'{name}: executable changed during run')
        if result['page_opens'] < 1: failures.append(f'{name}: sound/video page not opened')
        if result['failure_markers']: failures.append(f'{name}: engine failure markers observed')
        if result['cut_errors']: failures.append(f'{name}: stock cutscene errors observed; not waived')
        if result['adapter_errors']: failures.append(f'{name}: FPS adapter errors observed')
        if any(row['count'] != 1 for row in result['observations']): failures.append(f'{name}: duplicate FPS controls observed')
        expected_shots = {'enable_reopen': 4, 'restart_disable': 2, 'restart_off': 1}[name]
        if len(result['shots']) < expected_shots: failures.append(f'{name}: renderer checkpoints missing')
    after = identity(data, inventory)
    (artifacts / 'inventory-after.json').write_text(json.dumps(after, indent=2))
    if before != after or not all(row['matches'] for row in after): failures.append('retail copy changed')
    report = {'status': 'fail' if failures else 'pass', 'data_profile': 'data-retail', 'engine_bin': str(engine), 'engine_sha256': initial_hash,
              'inventory_count': len(before), 'inventory_unchanged': before == after,
              'phases': phases, 'failures': failures,
              'limitations': ['frames require visual review', 'replay is not physical input',
                              'nosound; audio output not tested', 'inventory does not prove original-media authenticity']}
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
