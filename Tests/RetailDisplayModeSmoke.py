#!/usr/bin/env python3
"""Replay the stock retail display-mode workflow against a disposable overlay.

The caller supplies the interaction fixtures after reviewing captures for the
actual display and scale. See Fixtures/retail_display_mode_workflow.md. This
uses synthetic input and is not evidence of physical mouse or keyboard input.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import sys

import InputScriptSmoke as replay
from RetailStockSettingsSmoke import identity, read_value, set_ini, sha256

ROOT = Path(__file__).resolve().parent.parent
OPEN = ROOT / 'Tests/Fixtures/retail_display_mode_open.txt'
PHASES = ('initial', 'keep', 'restart', 'revert', 'timeout', 'exclusive')
START = re.compile(r'StockDisplayProbe startup requested=(\w+) actual=(\w+) startup=(true|false) desktop=(true|false)')


def captures(fixture: str) -> list[int]:
    match = re.search(r'^\s*#\s*capture:\s*([\d, ]+)\s*$', fixture, re.M)
    if not match:
        raise ValueError('each fixture needs a # capture: frame,frame annotation')
    values = [int(item.strip()) for item in match.group(1).split(',')]
    if not values or len(set(values)) != len(values):
        raise ValueError('capture indices must be nonempty and unique')
    return values


def validate_fixture(name: str, fixture: str):
    tick, frames = replay.parse_fixture(fixture)
    replay.validate_launch_safe_frames(frames)
    selected = captures(fixture)
    if any(i >= len(frames) + 40 for i in selected):
        raise ValueError(f'{name}: capture index exceeds launch tick budget')
    if name == 'timeout':
        last = max((i for i, frame in enumerate(frames) if frame), default=-1)
        # No event, including pointer motion, after the final selection. The
        # stock modal uses game time; 650 frames at 30 Hz exceed 20 seconds.
        if tick * (len(frames) - 1 - last) < 21.0:
            raise ValueError('timeout fixture must end with 21 seconds of event-free frames')
    return tick, frames, selected


def phase_checks(name: str, log: str, stored: str | None) -> list[str]:
    failures = []
    def require(condition, message):
        if not condition:
            failures.append(message)
    startup = START.search(log)
    require(startup is not None, 'missing startup SDL mode probe')
    require('ChangePage Transient.FESoundVideoPage' in log, 'stock sound/video page was not opened')
    if name == 'initial' and startup:
        require(startup.groups() == ('Windowed', 'Windowed', 'false', 'true'),
                f'false startup with desktop sizing did not remain windowed: {startup.groups()}')
        require('StockDisplayProbe created' in log and 'mode=Windowed' in log, 'windowed control not observed')
        require(stored is None, 'initial phase unexpectedly created a display preference')
    if name == 'keep':
        require('StockDisplayProbe applied mode=BorderlessFullscreen' in log, 'borderless apply not observed')
        require('StockDisplayProbe confirmed' in log, 'confirmation not observed')
        require(stored == 'BorderlessFullscreen', f'borderless preference not saved: {stored!r}')
    if name == 'restart':
        if startup:
            require(startup.group(1) == 'BorderlessFullscreen' and startup.group(2) == 'BorderlessFullscreen',
                    f'borderless startup not restored: {startup.groups()}')
        require(stored == 'BorderlessFullscreen', 'borderless preference changed on restart')
    if name in ('revert', 'timeout'):
        require('StockDisplayProbe applied mode=Windowed' in log, 'windowed apply not observed')
        marker = 'StockDisplayProbe cancelled' if name == 'revert' else None
        rolled_back = (marker in log) if marker else ('StockDisplayProbe timeout restored' in log or 'StockDisplayProbe dialog timeout restored' in log)
        require(rolled_back, 'expected rollback reason not observed')
        require(stored == 'BorderlessFullscreen', f'rollback changed saved preference: {stored!r}')
    if name == 'exclusive':
        applied = 'StockDisplayProbe applied mode=ExclusiveFullscreen' in log
        rejected = 'Display mode transition failed:' in log and 'StockDisplayProbe applied mode=ExclusiveFullscreen' not in log
        require(applied or rejected, 'exclusive attempt was neither applied nor explicitly rejected')
        if applied:
            require('StockDisplayProbe cancelled' in log or 'StockDisplayProbe timeout restored' in log or 'StockDisplayProbe dialog timeout restored' in log,
                    'successful exclusive attempt was not reverted')
        require(stored == 'BorderlessFullscreen', f'exclusive attempt changed saved preference: {stored!r}')
    return failures


def remove_ini_value(text: str, section: str, key: str) -> str:
    match = re.search(r'(?im)^\[' + re.escape(section) + r'\][ \t]*$', text)
    if not match:
        return text
    following = re.search(r'(?m)^\[', text[match.end():])
    end = match.end() + following.start() if following else len(text)
    block = re.sub(r'(?im)^' + re.escape(key) + r'[ \t]*=.*(?:\n|$)', '', text[match.end():end])
    return text[:match.end()] + block + text[end:]


def optional_value(path: Path, section: str, key: str) -> str | None:
    import configparser
    try:
        return read_value(path, section, key)
    except (configparser.NoOptionError, configparser.NoSectionError):
        return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-root', type=Path, required=True, help='disposable retail copy')
    parser.add_argument('--inventory', type=Path, required=True, help='TSV retail size/SHA-256 inventory')
    parser.add_argument('--engine-bin', type=Path, required=True)
    parser.add_argument('--engine-sha256', required=True)
    parser.add_argument('--fixture-dir', type=Path, help='keep.txt, restart.txt, revert.txt, timeout.txt, exclusive.txt; see workflow document')
    parser.add_argument('--initial-only', action='store_true', help='capture and check startup only; result is partial, not workflow acceptance')
    parser.add_argument('--through', choices=PHASES, help='run through this phase only; result is partial unless exclusive is reached')
    parser.add_argument('--initial-fixture', type=Path, help='optional exploratory fixture; only valid with --initial-only')
    parser.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    parser.add_argument('--allow-window', action='store_true')
    parser.add_argument('--foreground', action='store_true', help='allow SDL to activate the app; required for full presentation evidence')
    parser.add_argument('--diagnose-occluded', action='store_true', help='continue after zero presented frames, but classify the result blocked')
    args = parser.parse_args()
    if not args.allow_window or args.artifact_dir is None:
        parser.error('--allow-window and --artifact-dir or HP2_ARTIFACT_DIR are required')
    if args.initial_fixture and not args.initial_only:
        parser.error('--initial-fixture requires --initial-only')
    if args.initial_only and args.through:
        parser.error('--initial-only and --through are mutually exclusive')
    selected_phase = 'initial' if args.initial_only else (args.through or 'exclusive')
    if selected_phase != 'initial' and not args.foreground:
        parser.error('--foreground is required for mode workflow; background-only captures cannot prove presentation')
    if selected_phase != 'initial' and args.fixture_dir is None:
        parser.error('--fixture-dir is required for mode workflow')
    data, inventory, engine = (p.resolve(strict=True) for p in (args.data_root, args.inventory, args.engine_bin))
    fixture_dir = args.fixture_dir.resolve(strict=True) if args.fixture_dir else None
    phases = PHASES[:PHASES.index(selected_phase) + 1]
    artifacts = Path(args.artifact_dir).resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts):
        parser.error('data and artifacts must be separate trees')
    if data.is_relative_to(Path.home().resolve() / 'Library/Application Support/Harry Potter 2'):
        parser.error('use a disposable retail copy, not the live retail data tree')
    if not re.fullmatch(r'[0-9a-fA-F]{64}', args.engine_sha256):
        parser.error('--engine-sha256 must contain 64 hexadecimal digits')
    if sha256(engine) != args.engine_sha256.lower():
        parser.error('engine SHA-256 does not match')
    initial = args.initial_fixture.resolve(strict=True) if args.initial_fixture else OPEN
    fixtures = {name: (initial if name == 'initial' else fixture_dir / f'{name}.txt').read_text() for name in phases}
    compiled = {name: validate_fixture(name, text) for name, text in fixtures.items()}
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()):
        parser.error('artifact directory must be empty')
    before = identity(data, inventory)
    (artifacts / 'inventory-before.json').write_text(json.dumps(before, indent=2))
    if not all(row['matches'] for row in before):
        print('blocked: retail inventory mismatch', file=sys.stderr)
        return 3
    tick, frames, _ = compiled['initial']
    home = artifacts / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, frames))
    ini = user / 'Game.ini'
    text = ini.read_text(encoding='utf-8')
    # Remove only this preference from the disposable user INI; leave retail
    # files and all unrelated INI sections untouched.
    text = remove_ini_value(text, 'SDLDrv.SDLClient', 'HP2DisplayMode')
    for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
                                ('SDLDrv.SDLClient', 'WindowedViewportX', '800'),
                                ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
                                ('SDLDrv.SDLClient', 'StartupFullscreen', 'False'),
                                ('SDLDrv.SDLClient', 'UseDesktopResolution', 'True'),
                                ('Core.System', 'SavePath', str(user / 'Save')),
                                ('Core.System', 'CachePath', str(user / 'Cache'))]:
        text = set_ini(text, section, key, value)
    ini.write_text(text, encoding='utf-8')
    results = {}; failures = []; blocked = []
    for name in phases:
        tick, frames, selected = compiled[name]
        phase = artifacts / name; phase.mkdir()
        (phase / 'fixture.txt').write_text(fixtures[name])
        (user / f'{replay.REPLAY_STEM}.rep').write_bytes(replay.compile_replay('PrivetDr.unr', tick, frames))
        command = replay.build_launch_command(engine, data, len(frames) + 40)
        command.append(f'-INI="{ini}"')
        (phase / 'command.json').write_text(json.dumps(command, indent=2))
        os.environ['HP2_CAPTURE_FRAMES'] = str(phase / 'frames')
        os.environ['HP2_CAPTURE_FRAME_INDICES'] = ','.join(map(str, selected))
        os.environ['HP2_STOCK_DISPLAY_TRACE'] = '1'
        os.environ['HP2_ARTIFACT_DIR'] = str(artifacts)
        if args.foreground:
            os.environ.pop('SDL_MAC_BACKGROUND_APP', None)
        else:
            os.environ['SDL_MAC_BACKGROUND_APP'] = '1'
        os.environ['XDG_DATA_HOME'] = str(home / '.local/share')
        output, status, timed_out, error = replay._supervised_run(command, home=home, cwd=ROOT, timeout_seconds=240)
        (phase / 'run.log').write_bytes(output)
        log = output.decode('utf-8', errors='replace')
        stored = optional_value(ini, 'SDLDrv.SDLClient', 'HP2DisplayMode')
        issues = phase_checks(name, log, stored)
        if status != 0 or timed_out or error: issues.append(f'process failure: status={status}, timeout={timed_out}, error={error}')
        tally = re.search(r'WgpuDrv: frames presented=(\d+) occluded=(\d+) outdated=(\d+) other_failure=(\d+)', log)
        if args.foreground and (not tally or int(tally.group(1)) == 0):
            if args.diagnose_occluded:
                blocked.append(f'{name}: foreground run did not report any presented WGPU frames')
            else:
                issues.append('foreground run did not report any presented WGPU frames')
        markers = replay.game_test._failure_markers(output, ROOT, home)
        if markers: issues.append(f'failure markers: {markers}')
        if sha256(engine) != args.engine_sha256.lower(): issues.append('engine executable changed during workflow')
        shots = [phase / 'frames' / f'frame_{index:06}.png' for index in selected]
        missing = [str(p) for p in shots if not p.is_file() or not p.stat().st_size]
        if missing: issues.append(f'missing captures: {missing}')
        (phase / 'Game.ini').write_bytes(ini.read_bytes())
        results[name] = {'process_exit': status, 'timeout': timed_out, 'stored': stored, 'captures': list(map(str, shots)), 'wgpu_tally': tally.groups() if tally else None, 'failures': issues}
        (phase / 'result.json').write_text(json.dumps(results[name], indent=2) + '\n')
        failures.extend(f'{name}: {item}' for item in issues)
        if issues: break  # Dependent steps must not run against an unknown mode.
    after = identity(data, inventory)
    (artifacts / 'inventory-after.json').write_text(json.dumps(after, indent=2))
    if before != after: failures.append('retail inventory changed')
    report = {'status': 'fail' if failures else ('blocked' if blocked else 'partial' if selected_phase != 'exclusive' else 'pass'), 'profile': 'data-retail', 'binary_sha256': args.engine_sha256.lower(), 'phases': results, 'failures': failures, 'blocked': blocked,
              'limitations': ['synthetic input', 'screenshots require visual review', 'a process-wide presentation tally does not prove that fullscreen frames were visible', 'exclusive support is reported by SDL, not independently verified by the OS']}
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 1 if failures else 2 if blocked else 0


if __name__ == '__main__':
    raise SystemExit(main())
