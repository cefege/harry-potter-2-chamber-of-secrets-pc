#!/usr/bin/env python3
"""Compare stock retail Object Detail extremes using isolated replay homes.

Requires Pillow, an explicit retail overlay, and an explicit executable. Opens an
SDL window. Captures are renderer output, not proof of desktop presentation.
Only System/HGame.u is verified against the supplied overlay manifest.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys

import InputScriptSmoke as replay
from RetailStockSettingsSmoke import set_ini, supervised_checkpoints, read_value

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-root', type=Path, required=True)
    parser.add_argument('--engine-bin', type=Path, required=True)
    parser.add_argument('--artifact-dir', type=Path, required=True)
    parser.add_argument('--allow-window', action='store_true')
    args = parser.parse_args()
    if not args.allow_window:
        parser.error('--allow-window is required')
    try:
        from PIL import Image, ImageChops
    except ImportError:
        print('blocked: Pillow is required', file=sys.stderr)
        return 3
    data = args.data_root.resolve(strict=True)
    engine = args.engine_bin.resolve(strict=True)
    artifacts = args.artifact_dir.resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts):
        parser.error('artifacts and data must be separate')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()):
        parser.error('artifact directory must be empty')
    manifest = json.loads((data / 'overlay-manifest.json').read_text())
    entries = [entry for entry in manifest['entries'] if entry['path'] == 'System/HGame.u']
    package = data / 'System/HGame.u'
    digest = hashlib.sha256(package.read_bytes()).hexdigest()
    if len(entries) != 1 or entries[0]['sha256'] != digest or entries[0]['size'] != package.stat().st_size:
        print('blocked: HGame.u does not match overlay manifest', file=sys.stderr)
        return 3
    results = {'hgame_sha256': digest, 'runs': {}}
    for label, x in [('low', 210), ('high', 350)]:
        out = artifacts / label
        out.mkdir()
        home = out / 'home'
        fixture = f'''tick 0.033333335\nidle 60\npress Enter\nidle 40\npress Escape\nidle 80\nmouse 680 455\npress LeftMouse\nidle 40\nmouse {x} 340\npress LeftMouse\nidle 5\npress LeftMouse\nidle 5\npress LeftMouse\nidle 5\npress LeftMouse\nidle 15\nmouse 755 555\npress LeftMouse\nidle 40\npress Escape\nidle 90\n'''
        tick, frames = replay.parse_fixture(fixture)
        replay.validate_launch_safe_frames(frames)
        user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, frames))
        ini = user / 'Game.ini'
        text = ini.read_text(encoding='utf-8')
        for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
                                    ('SDLDrv.SDLClient', 'WindowedViewportX', '800'),
                                    ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
                                    ('Core.System', 'SavePath', str(user / 'Save')),
                                    ('Core.System', 'CachePath', str(user / 'Cache'))]:
            text = set_ini(text, section, key, value)
        ini.write_text(text, encoding='utf-8')
        command = replay.build_launch_command(engine, data, len(frames) + 40)
        command.append(f'-INI="{ini}"')
        os.environ.update(HP2_CAPTURE_FRAMES=str(out / 'frames'), HP2_ARTIFACT_DIR=str(out),
                          SDL_MAC_BACKGROUND_APP='1', XDG_DATA_HOME=str(home / '.local/share'))
        (output, status, timeout, error), _, prune_errors = supervised_checkpoints(
            command, home=home, cwd=ROOT, timeout_seconds=180, directory=out / 'frames', keep={260, 300, 410})
        (out / 'run.log').write_bytes(output)
        (out / 'fixture.txt').write_text(fixture)
        (out / 'command.json').write_text(json.dumps(command, indent=2))
        setting = read_value(user / 'User.ini', 'Engine.PlayerPawn', 'ObjectDetail')
        text = output.decode('utf-8', errors='replace')
        results['runs'][label] = dict(status=status, timeout=timeout, error=error, prune_errors=prune_errors, setting=setting,
                                      opened_video='ChangePage Transient.FESoundVideoPage' in text)
    a = Image.open(artifacts / 'low/frames/frame_000410.png').convert('RGB')
    b = Image.open(artifacts / 'high/frames/frame_000410.png').convert('RGB')
    if a.size != b.size:
        raise ValueError('capture sizes differ')
    difference = ImageChops.difference(a, b)
    results['comparison'] = {'frame': 410, 'changed_pixels': sum(pixel != (0, 0, 0) for pixel in difference.getdata()),
                             'bounds': difference.getbbox()}
    (artifacts / 'report.json').write_text(json.dumps(results, indent=2))
    expected = {'low': 'ObjectDetailVeryLow', 'high': 'ObjectDetailVeryHigh'}
    ok = all(r['status'] == 0 and not r['timeout'] and not r['error'] and not r['prune_errors'] and r['opened_video']
             and r['setting'] == expected[label] for label, r in results['runs'].items())
    return 0 if ok and results['comparison']['changed_pixels'] > 0 else 1


if __name__ == '__main__':
    sys.exit(main())
