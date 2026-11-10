#!/usr/bin/env python3
"""Opt-in retail visual replay for resolution discovery, scrolling and rollback.

The overlay manifest verifies the disposable copy, not original media. The
retained screenshots require visual review; automatic checks alone cannot
prove legibility, desktop presentation, or physical input.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile

import InputScriptSmoke as replay
import RetailStockSettingsSmoke as stock

ROOT = Path(__file__).resolve().parent.parent


def fixture_and_checkpoints():
    source = (ROOT / 'Tests/Fixtures/retail_stock_settings_replay.txt').read_text()
    marker = '# Select 640x480, reject, acknowledge restore.'
    if source.count(marker) != 1:
        raise ValueError('stock fixture boundary changed')
    chunks = [source.split(marker)[0],
              '\nmouse 320 178\npress LeftMouse\nidle 20\n',
              'mouse 413 283\npress LeftMouse\nidle 5\npress LeftMouse\nidle 5\npress LeftMouse\nidle 20\n',
              'mouse 413 202\npress LeftMouse\nidle 5\npress LeftMouse\nidle 5\npress LeftMouse\nidle 20\n',
              'mouse 250 262\npress LeftMouse\nidle 50\n',
              'mouse 730 418\npress LeftMouse\nidle 45\n']
    text = ''
    checkpoints = {}
    for label, chunk in zip(('prefix', 'open', 'scrolled', 'top', 'applied', 'reverted'), chunks):
        text += chunk
        _, frames = replay.parse_fixture(text)
        checkpoints[label] = len(frames) - 5
    return text, checkpoints


def verify_manifest(data: Path):
    manifest = json.loads((data / 'overlay-manifest.json').read_text())
    entries = manifest['entries']
    if not entries:
        raise ValueError('empty overlay manifest')
    results = []
    seen = set()
    for entry in entries:
        rel = Path(entry['path'])
        path = (data / rel).resolve(strict=True)
        if rel.is_absolute() or '..' in rel.parts or not path.is_relative_to(data.resolve()) or str(rel) in seen:
            raise ValueError('unsafe or duplicate overlay entry')
        seen.add(str(rel))
        results.append({'path': str(rel), 'matches': path.is_file() and path.stat().st_size == entry['size'] and stock.sha256(path) == entry['sha256']})
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-root', type=Path, required=True)
    parser.add_argument('--engine-bin', type=Path, required=True)
    parser.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    parser.add_argument('--allow-window', action='store_true')
    args = parser.parse_args()
    if not args.allow_window or not args.artifact_dir:
        parser.error('--allow-window and --artifact-dir are required')
    source = args.data_root.resolve(strict=True)
    engine = args.engine_bin.resolve(strict=True)
    artifacts = args.artifact_dir.resolve()
    if artifacts == source or artifacts.is_relative_to(source) or source.is_relative_to(artifacts):
        parser.error('data and artifact trees must be separate')
    artifacts.mkdir(parents=True, exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix='retail-modern-resolution-', dir=artifacts))
    data = run / 'retail-copy'
    shutil.copytree(source, data, symlinks=False)
    before = verify_manifest(data)
    (run / 'manifest-before.json').write_text(json.dumps(before, indent=2))
    if not all(row['matches'] for row in before):
        raise RuntimeError('retail overlay manifest mismatch before replay')
    fixture, checkpoints = fixture_and_checkpoints()
    delta, frames = replay.parse_fixture(fixture)
    replay.validate_launch_safe_frames(frames)
    (run / 'fixture.txt').write_text(fixture)
    home = run / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', delta, frames))
    ini = user / 'Game.ini'
    text = ini.read_text()
    for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
        ('SDLDrv.SDLClient', 'WindowedViewportX', '800'), ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
        ('SDLDrv.SDLClient', 'StartupFullscreen', 'False'), ('Core.System', 'SavePath', str(user / 'Save')),
        ('Core.System', 'CachePath', str(user / 'Cache'))]:
        text = stock.set_ini(text, section, key, value)
    ini.write_text(text)
    before_binary = stock.sha256(engine)
    command = replay.build_launch_command(engine, data, len(frames) + 50)
    command.append(f'-INI="{ini}"')
    (run / 'command.json').write_text(json.dumps(command, indent=2))
    directory = run / 'frames'
    os.environ.update(HP2_ARTIFACT_DIR=str(run), HP2_CAPTURE_FRAMES=str(directory), SDL_MAC_BACKGROUND_APP='1', XDG_DATA_HOME=str(home / '.local/share'))
    (output, status, timed_out, error), discarded, prune_errors = stock.supervised_checkpoints(
        command, home=home, cwd=ROOT, timeout_seconds=180, directory=directory, keep=set(checkpoints.values()))
    (run / 'runtime.log').write_bytes(output)
    dimensions = {}
    for label, frame in checkpoints.items():
        try:
            dimensions[label] = stock.png_dimensions(directory / f'frame_{frame:06}.png')
        except (ValueError, OSError):
            dimensions[label] = None
    after = verify_manifest(data)
    (run / 'manifest-after.json').write_text(json.dumps(after, indent=2))
    failures = replay.game_test._failure_markers(output, ROOT, home)
    problems = []
    if status != 0 or timed_out or error or failures or prune_errors:
        problems.append('runtime or capture failed')
    if dimensions['open'] != (800, 600) or dimensions['scrolled'] != (800, 600) or dimensions['top'] != (800, 600):
        problems.append('missing 800x600 list checkpoints')
    if dimensions['applied'] != (1280, 720) or dimensions['reverted'] != (800, 600):
        problems.append('1280x720 selection or rollback not observed')
    if before != after or stock.sha256(engine) != before_binary:
        problems.append('retail copy or executable changed')
    # Existing replay treats CutError as a finding, not a waived success.
    cut_errors = [line for line in output.decode('utf-8', errors='replace').splitlines() if 'CutError:' in line and not line.startswith('Destroying ')]
    if cut_errors:
        problems.append('stock cutscene errors observed')
    report = {'status': 'fail' if problems else 'visual-review-required', 'data_profile': 'data-retail',
              'manifest_entries': len(before), 'checkpoints': checkpoints, 'dimensions': dimensions,
              'failure_markers': failures, 'cut_errors': cut_errors, 'problems': problems,
              'visual_review': 'Compare open, scrolled, and top captures for readable entries and scrollbar movement.'}
    (run / 'report.json').write_text(json.dumps(report, indent=2))
    print(f"{report['status']}: {run / 'report.json'}")
    return 1 if problems else 0


if __name__ == '__main__':
    raise SystemExit(main())
