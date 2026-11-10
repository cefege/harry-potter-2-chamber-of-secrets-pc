#!/usr/bin/env python3
"""Opt-in retail replay for bounded advanced popups and single-step keyboard input.

The set fixture must select 120 FPS with the mouse, press Down then Up on
that value, and select 75% UI scale. The restart fixture must reopen the page
at 75%, select frame limit, and press Up then Down. Calibrate coordinates and
review the screenshots manually; synthetic input does not prove physical input.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import sys

import InputScriptSmoke as replay
import RetailStockAdvancedGraphicsSmoke as advanced
import RetailStockGraphicsGpuSmoke as gpu
from RetailStockSettingsSmoke import identity, read_value, set_ini
from RetailStockFpsSmoke import sha256


def exact_changes(rows: list[dict[str, str]], setting: str, values: list[str]) -> bool:
    """Reject missing, extra, duplicated, or ineffective apply events."""
    applied = [row for row in rows if row['event'] == 'applied' and row['setting'] == setting]
    return [row['actual'] for row in applied] == values and all(row['requested'] == row['actual'] for row in applied)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('data-root', 'inventory', 'engine-bin', 'set-fixture', 'restart-fixture'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--engine-sha256', required=True)
    p.add_argument('--set-checkpoints', required=True)
    p.add_argument('--restart-checkpoints', required=True)
    p.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    p.add_argument('--allow-window', action='store_true')
    args = p.parse_args()
    if not args.allow_window or args.artifact_dir is None:
        p.error('--allow-window and --artifact-dir are required')
    try:
        checkpoints = [[int(v) for v in s.split(',')] for s in (args.set_checkpoints, args.restart_checkpoints)]
        if min(checkpoints[0] + checkpoints[1]) < 0: raise ValueError()
    except ValueError:
        p.error('checkpoints must be nonnegative comma-separated integers')
    data, inventory, engine, first, restart = (v.resolve(strict=True) for v in
        (args.data_root, args.inventory, args.engine_bin, args.set_fixture, args.restart_fixture))
    artifacts = Path(args.artifact_dir).resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts):
        p.error('data and artifacts must be separate')
    if data.is_relative_to(Path.home().resolve() / 'Library/Application Support/Harry Potter 2'):
        p.error('use a disposable retail copy')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()): p.error('artifact directory must be empty')
    expected = args.engine_sha256.lower()
    if not re.fullmatch(r'[0-9a-f]{64}', expected): p.error('invalid engine hash')
    if sha256(engine) != expected:
        print('blocked: engine hash mismatch', file=sys.stderr); return 3
    before = identity(data, inventory)
    (artifacts / 'inventory-before.json').write_text(json.dumps(before, indent=2) + '\n')
    if len(before) != 488 or not all(row['matches'] for row in before):
        print('blocked: retail inventory must contain 488 matching entries', file=sys.stderr); return 3
    for fixture, indices in zip((first, restart), checkpoints):
        _, frames = replay.parse_fixture(fixture.read_text(encoding='utf-8'))
        replay.validate_launch_safe_frames(frames)
        if max(indices) >= len(frames): p.error(f'checkpoint outside {fixture}')
    tick, frames = replay.parse_fixture(first.read_text(encoding='utf-8'))
    home = artifacts / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, frames))
    ini = user / 'Game.ini'
    text = ini.read_text(encoding='utf-8')
    for section, key, value in [
        ('Engine.GameEngine', 'FrameRateLimit', '60'),
        ('SDLDrv.SDLClient', 'UIScale', '1.0'),
        ('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
        ('SDLDrv.SDLClient', 'WindowedViewportX', '800'),
        ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
        ('Core.System', 'SavePath', str(user / 'Save')),
        ('Core.System', 'CachePath', str(user / 'Cache')),
    ]:
        text = set_ini(text, section, key, value)
    ini.write_text(text, encoding='utf-8')
    failures = []
    phases = {}
    for name, fixture, indices, expected_frames in [
        ('set', first, checkpoints[0], ['120', '144', '120']),
        ('restart', restart, checkpoints[1], ['60', '120']),
    ]:
        result = gpu.run_phase(name, fixture, indices, engine, expected, data, home, user, artifacts)
        phases[name] = result
        if result['exit'] != 0 or result['timed_out'] or result['launch_error']: failures.append(f'{name}: process failed')
        if result['engine_sha256_after'] != expected: failures.append(f'{name}: engine changed')
        if result['failure_markers'] or result['adapter_errors']: failures.append(f'{name}: engine errors')
        if len(result['shots']) != len(indices): failures.append(f'{name}: screenshots missing')
        log = (artifacts / name / 'run.log').read_text(encoding='utf-8', errors='replace')
        if 'Graphics VM observer:' in log: failures.append(f'{name}: graphics VM observer error')
        rows = advanced.probes(log)
        result['advanced_probe'] = rows
        created = {(r['setting'], r['actual']) for r in rows if r['event'] == 'created'}
        required = {('frame_limit', '60'), ('ui_scale', '100')} if name == 'set' else {('frame_limit', '120'), ('ui_scale', '75')}
        if not required.issubset(created): failures.append(f'{name}: missing initial live probes {required - created}')
        if not exact_changes(rows, 'frame_limit', expected_frames):
            failures.append(f'{name}: frame changes are not the exact mouse/keyboard sequence {expected_frames}')
        scales = [r for r in rows if r['event'] == 'applied' and r['setting'] == 'ui_scale']
        if name == 'set' and not exact_changes(rows, 'ui_scale', ['75']): failures.append('set: UI scale did not change exactly once to 75')
        if name == 'restart' and scales: failures.append('restart: UI scale changed unexpectedly')
        stored_frame = read_value(ini, 'Engine.GameEngine', 'FrameRateLimit')
        stored_scale = read_value(ini, 'SDLDrv.SDLClient', 'UIScale')
        if not advanced.config_matches('frame_limit', stored_frame, '120'): failures.append(f'{name}: frame limit not persisted')
        if not advanced.config_matches('ui_scale', stored_scale, '75'): failures.append(f'{name}: UI scale not persisted')
        result['stored_frame_limit'], result['stored_ui_scale'] = stored_frame, stored_scale
        (artifacts / name / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
        if result['exit'] != 0 or result['timed_out'] or result['launch_error']:
            if name == 'set': failures.append('set phase failed; restart was not run')
            break
    after = identity(data, inventory)
    (artifacts / 'inventory-after.json').write_text(json.dumps(after, indent=2) + '\n')
    if len(after) != 488 or before != after or not all(row['matches'] for row in after): failures.append('retail copy changed')
    report = {'status': 'fail' if failures else 'needs-visual-review', 'data_profile': 'data-retail',
              'engine_sha256': expected, 'inventory_unchanged': before == after, 'phases': phases,
              'failures': failures, 'limitations': ['screenshots require visual review',
              'synthetic input is not physical input', 'inventory does not prove original-media authenticity', 'audio not tested']}
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 1 if failures else 0

if __name__ == '__main__': raise SystemExit(main())
