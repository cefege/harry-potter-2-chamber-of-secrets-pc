#!/usr/bin/env python3
"""Exercise the retail resolution dialog's unattended 20-second rollback.

The disposable copy and isolated home are mandatory. Captured frames require
visual review. Replay time is distinct from wall-clock time.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import sys

import InputScriptSmoke as replay
import RetailStockSettingsSmoke as stock

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / 'Tests/Fixtures'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--data-root', type=Path, required=True)
    p.add_argument('--inventory', type=Path, required=True)
    p.add_argument('--engine-bin', type=Path, required=True)
    p.add_argument('--engine-sha256', required=True)
    p.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    p.add_argument('--allow-window', action='store_true')
    args = p.parse_args()
    if not args.allow_window or not args.artifact_dir:
        p.error('--allow-window and --artifact-dir (or HP2_ARTIFACT_DIR) are required')
    data, inventory, engine = (x.resolve(strict=True) for x in (args.data_root, args.inventory, args.engine_bin))
    artifacts = Path(args.artifact_dir).resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts):
        p.error('data and artifacts must be separate trees')
    if data.is_relative_to(Path.home().resolve() / 'Library/Application Support/Harry Potter 2'):
        p.error('use a disposable retail copy')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()):
        p.error('artifact directory must be empty')
    engine_before = stock.sha256(engine)
    if engine_before != args.engine_sha256.lower():
        print('blocked: binary hash does not match requested binary', file=sys.stderr)
        return 3
    before = stock.identity(data, inventory)
    (artifacts / 'inventory-before.json').write_text(json.dumps(before, indent=2))
    if not all(row['matches'] for row in before):
        print('blocked: inventory mismatch', file=sys.stderr)
        return 3
    fixture = (FIXTURES / 'retail_stock_resolution_timeout.txt').read_text()
    tick, frames = replay.parse_fixture(fixture)
    replay.validate_launch_safe_frames(frames)
    home = artifacts / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, frames))
    ini = user / 'Game.ini'
    text = ini.read_text()
    for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
                                ('SDLDrv.SDLClient', 'WindowedViewportX', '800'),
                                ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
                                ('SDLDrv.SDLClient', 'StartupFullscreen', 'False'),
                                ('Core.System', 'SavePath', str(user / 'Save')),
                                ('Core.System', 'CachePath', str(user / 'Cache'))]:
        text = stock.set_ini(text, section, key, value)
    ini.write_text(text)
    os.environ.update(HP2_ARTIFACT_DIR=str(artifacts), SDL_MAC_BACKGROUND_APP='1',
                      XDG_DATA_HOME=str(home / '.local/share'))
    checkpoints = {'confirmation': 300, 'before_timeout': 600, 'after_timeout': 930, 'settled': 1000}
    def run(name, fixture_text, keep):
        delta, events = replay.parse_fixture(fixture_text)
        replay.validate_launch_safe_frames(events)
        (user / 'InputScriptSmoke.rep').write_bytes(replay.compile_replay('PrivetDr.unr', delta, events))
        command = replay.build_launch_command(engine, data, len(events) + 80)
        command.append(f'-INI="{ini}"')
        (artifacts / f'{name}-fixture.txt').write_text(fixture_text)
        (artifacts / f'{name}-command.json').write_text(json.dumps(command, indent=2))
        directory = artifacts / f'{name}-frames'
        os.environ['HP2_CAPTURE_FRAMES'] = str(directory)
        (output, status, timed_out, error), discarded, prune_errors = stock.supervised_checkpoints(
            command, home=home, cwd=ROOT, timeout_seconds=180, directory=directory, keep=set(keep.values()))
        (artifacts / f'{name}.log').write_bytes(output)
        (artifacts / f'{name}-discarded.json').write_text(json.dumps(discarded))
        sizes = {}
        for label, index in keep.items():
            path = directory / f'frame_{index:06}.png'
            try:
                sizes[label] = stock.png_dimensions(path)
            except (ValueError, OSError):
                sizes[label] = None
        return {'exit': status, 'timed_out': timed_out, 'error': error, 'sizes': sizes,
                'discarded_count': len(discarded), 'prune_errors': prune_errors,
                'failure_markers': replay.game_test._failure_markers(output, ROOT, home)}, output.decode('utf-8', errors='replace')
    first, log = run('timeout', fixture, checkpoints)
    width = stock.read_value(ini, 'SDLDrv.SDLClient', 'WindowedViewportX')
    height = stock.read_value(ini, 'SDLDrv.SDLClient', 'WindowedViewportY')
    engine_between = stock.sha256(engine)
    second, restart_log = None, ''
    if first['exit'] == 0 and (width, height) in [('800', '600'), ('640', '480')] and engine_between == engine_before:
        restart_fixture = (FIXTURES / 'retail_stock_resolution_timeout_restart.txt').read_text()
        if (width, height) == ('640', '480'):
            restart_fixture = restart_fixture.replace('mouse 680 455', 'mouse 544 364')
        second, restart_log = run('restart', restart_fixture, {'restart_video': 240})
    after_width = stock.read_value(ini, 'SDLDrv.SDLClient', 'WindowedViewportX')
    after_height = stock.read_value(ini, 'SDLDrv.SDLClient', 'WindowedViewportY')
    engine_after = stock.sha256(engine)
    after = stock.identity(data, inventory)
    (artifacts / 'inventory-after.json').write_text(json.dumps(after, indent=2))
    failures = []
    if first['exit'] != 0 or first['timed_out'] or first['error'] or first['failure_markers'] or first['prune_errors']:
        failures.append('timeout process or capture failed')
    if first['sizes'] != {'confirmation': (640, 480), 'before_timeout': (640, 480),
                          'after_timeout': (800, 600), 'settled': (800, 600)}:
        failures.append('timeout frame dimensions do not demonstrate expected rollback timing')
    if (width, height) != ('800', '600') or (after_width, after_height) != ('800', '600'):
        failures.append('original resolution not persisted across restart')
    if second is None or second['exit'] != 0 or second['timed_out'] or second['error'] or second['failure_markers'] or second['prune_errors'] or second['sizes'].get('restart_video') != (800, 600):
        failures.append('restart did not confirm original resolution')
    if not engine_before == engine_between == engine_after:
        failures.append('executable changed during test')
    if before != after:
        failures.append('retail copy changed')
    cut_errors = [line for line in (log + '\n' + restart_log).splitlines() if 'CutError:' in line and not line.startswith('Destroying ')]
    if cut_errors:
        failures.append('stock cutscene errors observed; not waived')
    report = {'status': 'fail' if failures else 'pass', 'data_profile': 'data-retail',
              'inventory_count': len(before), 'inventory_matches_after': all(row['matches'] for row in after),
              'engine_sha256_before': engine_before, 'engine_sha256_between': engine_between,
              'engine_sha256_after': engine_after, 'timeout': first, 'restart': second,
              'resolution_before_restart': [width, height], 'resolution_after_restart': [after_width, after_height],
              'replay_tick_seconds': tick, 'selection_frame': 261,
              'dialog_setup_lines': [line for line in log.splitlines() if 'HPMessageBox Setup' in line],
              'cut_errors': cut_errors,
              'failures': failures, 'limitations': ['screenshots require visual review', 'replay does not verify physical input',
              'replay time is not wall-clock timeout validation', 'inventory does not prove original-media authenticity']}
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
