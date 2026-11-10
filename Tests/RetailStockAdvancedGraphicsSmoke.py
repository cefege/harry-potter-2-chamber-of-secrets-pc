#!/usr/bin/env python3
"""Opt-in replay for all advanced stock graphics controls, including restart.

Requires calibrated synthetic-input fixtures and actual-value trace probes.
The screenshots must be reviewed for discoverability, clipping and overlap.
This script must not run while another test owns the display.
"""
from __future__ import annotations
import argparse
import configparser
import json
import os
from pathlib import Path
import re
import sys

import InputScriptSmoke as replay
import RetailStockGraphicsGpuSmoke as gpu
from RetailStockSettingsSmoke import identity, read_value, set_ini
from RetailStockFpsSmoke import sha256

TRACE = re.compile(r'StockGraphicsProbe event=(created|applied) setting=(frame_limit|ui_scale|vertical_fov|native_text|screen_flashes) requested=(\d+|on|off) actual=(\d+|on|off)')
# Display percentages are integer values; INI scales are floating point.
SETTINGS = {
    'frame_limit': ('Engine.GameEngine', 'FrameRateLimit', '120', '60'),
    'ui_scale': ('SDLDrv.SDLClient', 'UIScale', '125', '1.0'),
    'vertical_fov': ('SDLDrv.SDLClient', 'MaintainVerticalFOV', 'off', 'True'),
    'native_text': ('SDLDrv.SDLClient', 'NativeText', 'off', 'True'),
    'screen_flashes': ('SDLDrv.SDLClient', 'ScreenFlashes', 'off', 'True'),
}

def probes(log: str) -> list[dict[str, str]]:
    return [dict(zip(('event', 'setting', 'requested', 'actual'), fields)) for fields in TRACE.findall(log)]

def config_matches(setting: str, actual: str | None, expected: str) -> bool:
    if actual is None: return False
    try:
        if setting == 'ui_scale': return float(actual) == int(expected) / 100
        if setting == 'frame_limit': return float(actual) == int(expected)
    except ValueError:
        return False
    return actual.strip().lower() in (('true', 'on', '1', 'yes') if expected == 'on' else ('false', 'off', '0', 'no'))

def changed(rows: list[dict[str, str]], setting: str, target: str) -> bool:
    """Require a distinct observed starting value before the matching apply."""
    initial = next((i for i, row in enumerate(rows) if row['event'] == 'created' and row['setting'] == setting and row['actual'] != target), None)
    return initial is not None and any(row == dict(event='applied', setting=setting, requested=target, actual=target) for row in rows[initial + 1:])

def safe_read(path: Path, section: str, key: str) -> str | None:
    try: return read_value(path, section, key)
    except (OSError, configparser.Error): return None

def check_phase(result: dict, root: Path, name: str, user: Path, expected_hash: str,
                checkpoints: list[int], vsync: str, anisotropy: str, msaa: str) -> list[str]:
    failures: list[str] = []
    if result['exit'] != 0 or result['timed_out'] or result['launch_error']: failures.append(f'{name}: process failed')
    if result['engine_sha256_after'] != expected_hash: failures.append(f'{name}: engine changed')
    if result['failure_markers'] or result['adapter_errors']: failures.append(f'{name}: engine errors')
    if len(result['shots']) != len(checkpoints): failures.append(f'{name}: screenshots missing')
    event = 'applied' if name == 'set' else 'created'
    log = (root / name / 'run.log').read_text(encoding='utf-8', errors='replace')
    rows = probes(log)
    result['advanced_probe'] = rows
    result['stored_advanced'] = {}
    for setting, (section, key, target, _) in SETTINGS.items():
        actual = safe_read(user / 'Game.ini', section, key)
        result['stored_advanced'][setting] = actual
        if not config_matches(setting, actual, target): failures.append(f'{name}: {setting} config mismatch')
        if not (changed(rows, setting, target) if name == 'set' else gpu.matching(rows, event, setting, target)):
            failures.append(f'{name}: missing distinct live change or restart {setting} trace')
    if (result['vsync'] or '').strip().lower() != vsync: failures.append(f'{name}: VSync config mismatch')
    try:
        if float(result['anisotropy']) != int(anisotropy): failures.append(f'{name}: anisotropy config mismatch')
    except (TypeError, ValueError): failures.append(f'{name}: invalid anisotropy config')
    aa = safe_read(user / 'Game.ini', 'WgpuDrv.WgpuRenderDevice', 'UseAA')
    samples = safe_read(user / 'Game.ini', 'WgpuDrv.WgpuRenderDevice', 'NumAASamples')
    result['msaa_enabled'], result['msaa_samples'] = aa, samples
    if not config_matches('msaa_enabled', aa, 'off' if msaa == '0' else 'on'): failures.append(f'{name}: MSAA enable config mismatch')
    try:
        if int(samples) != int(msaa): failures.append(f'{name}: MSAA sample config mismatch')
    except (TypeError, ValueError): failures.append(f'{name}: invalid MSAA sample config')
    for setting, target in (('vsync', vsync), ('anisotropy', anisotropy), ('msaa', msaa)):
        if not (changed(result['probe'], setting, target) if name == 'set' else gpu.matching(result['probe'], event, setting, target)):
            failures.append(f'{name}: missing distinct live change or restart {setting} GPU trace')
    (root / name / 'result.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    return failures

def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('data-root', 'inventory', 'engine-bin', 'set-fixture', 'restart-fixture'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--engine-sha256', required=True)
    p.add_argument('--set-checkpoints', required=True)
    p.add_argument('--restart-checkpoints', required=True)
    p.add_argument('--vsync', choices=('on', 'off'), required=True)
    p.add_argument('--anisotropy', choices=('0', '4', '8', '16'), required=True)
    p.add_argument('--msaa', choices=('0', '2', '4'), required=True)
    p.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    p.add_argument('--allow-window', action='store_true')
    args = p.parse_args()
    if not args.allow_window or args.artifact_dir is None: p.error('--allow-window and --artifact-dir are required')
    try:
        sc = [int(v) for v in args.set_checkpoints.split(',')]
        rc = [int(v) for v in args.restart_checkpoints.split(',')]
        if min(sc + rc) < 0: raise ValueError()
    except ValueError: p.error('checkpoints must be nonnegative comma-separated integers')
    data, engine, inventory, fixture, restart = (v.resolve(strict=True) for v in
        (args.data_root, args.engine_bin, args.inventory, args.set_fixture, args.restart_fixture))
    artifacts = Path(args.artifact_dir).resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts): p.error('data and artifacts must be separate')
    if data.is_relative_to(Path.home().resolve() / 'Library/Application Support/Harry Potter 2'): p.error('use a disposable retail copy')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()): p.error('artifact directory must be empty')
    expected = args.engine_sha256.lower()
    if not re.fullmatch(r'[0-9a-f]{64}', expected): p.error('invalid engine hash')
    if sha256(engine) != expected:
        print('blocked: engine hash mismatch', file=sys.stderr); return 3
    before = identity(data, inventory)
    (artifacts / 'inventory-before.json').write_text(json.dumps(before, indent=2), encoding='utf-8')
    if not all(row['matches'] for row in before):
        print('blocked: retail inventory mismatch', file=sys.stderr); return 3
    for path, checkpoints in ((fixture, sc), (restart, rc)):
        _, frames = replay.parse_fixture(path.read_text(encoding='utf-8'))
        replay.validate_launch_safe_frames(frames)
        if max(checkpoints) >= len(frames): p.error(f'checkpoint outside {path}')
    tick, frames = replay.parse_fixture(fixture.read_text(encoding='utf-8'))
    home = artifacts / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, frames))
    ini = user / 'Game.ini'; text = ini.read_text(encoding='utf-8')
    for _, (section, key, _, initial) in SETTINGS.items(): text = set_ini(text, section, key, initial)
    for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
            ('SDLDrv.SDLClient', 'WindowedViewportX', '800'), ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
            ('WgpuDrv.WgpuRenderDevice', 'UseVSync', 'Off' if args.vsync == 'on' else 'On'),
            ('WgpuDrv.WgpuRenderDevice', 'MaxAnisotropy', '4' if args.anisotropy == '0' else '0'),
            ('WgpuDrv.WgpuRenderDevice', 'UseAA', 'True' if args.msaa == '0' else 'False'),
            ('WgpuDrv.WgpuRenderDevice', 'NumAASamples', '4' if args.msaa == '0' else '0'),
            ('Core.System', 'SavePath', str(user / 'Save')), ('Core.System', 'CachePath', str(user / 'Cache'))]:
        text = set_ini(text, section, key, value)
    ini.write_text(text, encoding='utf-8')
    phases = {'set': gpu.run_phase('set', fixture, sc, engine, expected, data, home, user, artifacts)}
    failures = check_phase(phases['set'], artifacts, 'set', user, expected, sc, args.vsync, args.anisotropy, args.msaa)
    if phases['set']['exit'] == 0 and not phases['set']['timed_out'] and not phases['set']['launch_error']:
        phases['restart'] = gpu.run_phase('restart', restart, rc, engine, expected, data, home, user, artifacts)
        failures += check_phase(phases['restart'], artifacts, 'restart', user, expected, rc, args.vsync, args.anisotropy, args.msaa)
    else: failures.append('set phase failed; restart was not run')
    after = identity(data, inventory)
    (artifacts / 'inventory-after.json').write_text(json.dumps(after, indent=2), encoding='utf-8')
    if before != after or not all(row['matches'] for row in after): failures.append('retail copy changed')
    report = {'status': 'fail' if failures else 'needs-visual-review', 'data_profile': 'data-retail', 'phases': phases,
        'engine_sha256': expected, 'inventory_unchanged': before == after, 'failures': failures,
        'limitations': ['screenshots require visual review', 'synthetic input is not physical input',
                        'inventory does not prove original-media authenticity', 'audio not tested']}
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    return 1 if failures else 0

if __name__ == '__main__': raise SystemExit(main())
