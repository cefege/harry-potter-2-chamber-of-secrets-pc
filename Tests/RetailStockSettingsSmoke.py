#!/usr/bin/env python3
"""Replay the stock retail settings book in an isolated home.

Requires an explicitly supplied disposable retail copy and TSV size/SHA-256
inventory. This test opens an SDL window. Captures are engine render output,
not proof of desktop presentation or physical input. It does not validate all
settings, audio output, or original-media authenticity.
"""
from __future__ import annotations

import argparse
import configparser
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import sys
import threading

import InputScriptSmoke as replay

ROOT = Path(__file__).resolve().parent.parent
FIXTURE = ROOT / 'Tests/Fixtures/retail_stock_settings_replay.txt'
RESTART_FIXTURE = ROOT / 'Tests/Fixtures/retail_stock_settings_restart.txt'
FIRST_CHECKPOINTS = {'book': 170, 'matrix_invert_off_center_on': 220,
    'matrix_invert_on_center_on': 240, 'matrix_invert_on_center_off': 255,
    'matrix_invert_off_center_off': 270, 'matrix_invert_on_center_off_again': 285,
    'video': 510, 'texture_popup': 520, 'texture_selected': 540, 'resolution_confirm': 630, 'resolution_reverted': 730,
    'resolution_accepted': 850, 'fps_enabled': 890, 'color_depth_list': 930, 'resumed': 1035}
RESTART_CHECKPOINTS = {'restart_input': 240, 'restart_video': 340}


def prune_new_frames(directory: Path, keep: set[int], *, finished: bool) -> list[str]:
    """Remove only unselected frame files in a fresh, harness-owned capture directory."""
    if not directory.is_dir():
        return []
    frames = []
    for path in directory.iterdir():
        match = re.fullmatch(r'frame_(\d{6})\.png', path.name)
        if match and path.is_file() and not path.is_symlink():
            frames.append((int(match.group(1)), path))
    # Preserve the startup image and the 2 latest images, including on a
    # failure. The renderer can still be writing a recent file.
    newest = max((index for index, _ in frames), default=-1)
    protected = keep | {0, newest, newest - 1}
    removed = []
    for index, path in frames:
        if index not in protected:
            try:
                path.unlink()
                removed.append(path.name)
            except FileNotFoundError:
                pass
    return removed


def supervised_checkpoints(command, *, home, cwd, timeout_seconds, directory, keep):
    """Bound temporary frame disk use while retaining requested evidence."""
    stopped = threading.Event()
    removed = []
    errors = []
    def reap():
        while not stopped.wait(0.2):
            try:
                removed.extend(prune_new_frames(directory, keep, finished=False))
            except OSError as error:
                errors.append(str(error))
                break
    worker = threading.Thread(target=reap, daemon=True)
    worker.start()
    try:
        result = replay._supervised_run(command, home=home, cwd=cwd, timeout_seconds=timeout_seconds)
    finally:
        stopped.set()
        worker.join()
        try:
            removed.extend(prune_new_frames(directory, keep, finished=True))
        except OSError as error:
            errors.append(str(error))
    return result, removed, errors


def set_ini(text: str, section: str, key: str, value: str) -> str:
    match = re.search(r'(?im)^\[' + re.escape(section) + r'\][ \t]*$', text)
    if match is None:
        return text.rstrip() + f'\n\n[{section}]\n{key}={value}\n'
    following = re.search(r'(?m)^\[', text[match.end():])
    end = match.end() + following.start() if following else len(text)
    block = text[match.end():end]
    block = re.sub(r'(?im)^' + re.escape(key) + r'[ \t]*=.*(?:\n|$)', '', block)
    return text[:match.end()] + block.rstrip() + f'\n{key}={value}\n\n' + text[end:]


def read_value(path: Path, section: str, key: str) -> str:
    parser = configparser.ConfigParser(strict=False, interpolation=None)
    parser.read(path, encoding='utf-8')
    return parser.get(section, key)


def png_dimensions(path: Path):
    with path.open('rb') as stream:
        header = stream.read(24)
    if len(header) != 24 or header[:8] != b'\x89PNG\r\n\x1a\n' or header[12:16] != b'IHDR':
        raise ValueError(f'invalid PNG: {path}')
    return struct.unpack('>II', header[16:24])


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def identity(data: Path, inventory: Path) -> list[dict]:
    data = data.resolve(strict=True)
    rows = []
    seen = set()
    for line in inventory.read_text(encoding='utf-8').splitlines():
        name, size, expected = line.split('\t')
        relative = Path(name)
        target = (data / relative).resolve(strict=True)
        if relative.is_absolute() or '..' in relative.parts or not target.is_relative_to(data) or name in seen:
            raise ValueError(f'unsafe or duplicate inventory path: {name}')
        seen.add(name)
        digest = hashlib.sha256()
        with target.open('rb') as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                digest.update(chunk)
        actual = digest.hexdigest()
        rows.append({'path': name, 'size': target.stat().st_size, 'sha256': actual,
                     'matches': target.stat().st_size == int(size) and actual == expected.lower()})
    if not rows:
        raise ValueError('empty inventory')
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-root', type=Path, required=True, help='disposable retail copy, not the live retail root')
    parser.add_argument('--inventory', type=Path, required=True, help='TSV: relative path, size, SHA-256')
    parser.add_argument('--engine-bin', type=Path, required=True)
    parser.add_argument('--map', default='PrivetDr.unr', help='retail replay map URL (default: PrivetDr.unr)')
    parser.add_argument('--fixture', type=Path, default=FIXTURE, help='explicit replay fixture; retain prior evidence when recalibrating')
    parser.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    parser.add_argument('--allow-window', action='store_true', help='acknowledge that an SDL window can appear')
    parser.add_argument('--audio', action='store_true', help='enable real audio output; samples are not recorded')
    args = parser.parse_args()
    if not args.allow_window or args.artifact_dir is None:
        parser.error('--allow-window and --artifact-dir (or HP2_ARTIFACT_DIR) are required')
    data = args.data_root.resolve(strict=True)
    inventory = args.inventory.resolve(strict=True)
    engine = args.engine_bin.resolve(strict=True)
    artifacts = Path(args.artifact_dir).resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts):
        parser.error('data and artifacts must be separate trees')
    if data.is_relative_to(Path.home().resolve() / 'Library/Application Support/Harry Potter 2'):
        parser.error('use a disposable retail copy, not the user data tree')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()):
        parser.error('artifact directory must be empty')
    engine_before = sha256(engine)
    before = identity(data, inventory)
    (artifacts / 'inventory-before.json').write_text(json.dumps(before, indent=2))
    if not all(row['matches'] for row in before):
        print('blocked: retail copy does not match inventory', file=sys.stderr)
        return 3
    fixture_text = args.fixture.read_text(encoding='utf-8')
    tick, frames = replay.parse_fixture(fixture_text)
    replay.validate_launch_safe_frames(frames)
    home = artifacts / 'home'
    user = replay.prepare_user_home(home, data, replay.compile_replay(args.map, tick, frames))
    game_ini = user / 'Game.ini'
    text = game_ini.read_text(encoding='utf-8')
    for section, key, value in [('SDLDrv.SDLClient', 'CaptureMouse', 'False'),
                                ('SDLDrv.SDLClient', 'WindowedViewportX', '800'),
                                ('SDLDrv.SDLClient', 'WindowedViewportY', '600'),
                                ('SDLDrv.SDLClient', 'TextureDetail', 'High'),
                                ('SDLDrv.SDLClient', 'ShowFPS', 'False'),
                                ('ALAudio.ALAudioSubsystem', 'MusicVolume', '0.53'),
                                ('ALAudio.ALAudioSubsystem', 'SoundVolume', '0.90'),
                                ('Core.System', 'SavePath', str(user / 'Save')),
                                ('Core.System', 'CachePath', str(user / 'Cache'))]:
        text = set_ini(text, section, key, value)
    game_ini.write_text(text, encoding='utf-8')
    user_ini = user / 'User.ini'
    user_text = user_ini.read_text(encoding='utf-8')
    for section, key, value in [('Engine.PlayerPawn', 'bInvertMouse', 'False'),
                                ('Engine.PlayerPawn', 'bAutoJump', 'False'),
                                ('HGame.Harry', 'bAutoCenterCamera', 'True'),
                                ('HGame.Harry', 'bAutoQuaff', 'True'),
                                ('HGame.Harry', 'bInvertBroomPitch', 'False')]:
        user_text = set_ini(user_text, section, key, value)
    user_ini.write_text(user_text, encoding='utf-8')
    initial_invert = read_value(user_ini, 'Engine.PlayerPawn', 'bInvertMouse')
    initial_brightness = read_value(game_ini, 'SDLDrv.SDLClient', 'Brightness')
    if initial_invert.lower() != 'false':
        print('blocked: fixture requires initial bInvertMouse=False', file=sys.stderr)
        return 3
    command = replay.build_launch_command(engine, data, len(frames) + 80)
    if args.audio and '-nosound' in command:
        command.remove('-nosound')
    command.append(f'-INI="{game_ini}"')
    (artifacts / 'fixture.txt').write_text(fixture_text)
    (artifacts / 'command.json').write_text(json.dumps(command, indent=2))
    os.environ['HP2_CAPTURE_FRAMES'] = str(artifacts / 'frames')
    os.environ['HP2_ARTIFACT_DIR'] = str(artifacts)
    os.environ['SDL_MAC_BACKGROUND_APP'] = '1'
    os.environ['XDG_DATA_HOME'] = str(home / '.local/share')
    (output, status, timed_out, launch_error), first_discarded, first_prune_errors = supervised_checkpoints(
        command, home=home, cwd=ROOT, timeout_seconds=180, directory=artifacts / 'frames',
        keep=set(FIRST_CHECKPOINTS.values()))
    (artifacts / 'run.log').write_bytes(output)
    log = output.decode('utf-8', errors='replace')
    def snapshot():
        fields = [('invert', user_ini, 'Engine.PlayerPawn', 'bInvertMouse'),
                  ('auto_jump', user_ini, 'Engine.PlayerPawn', 'bAutoJump'),
                  ('auto_center', user_ini, 'HGame.Harry', 'bAutoCenterCamera'),
                  ('auto_quaff', user_ini, 'HGame.Harry', 'bAutoQuaff'),
                  ('invert_broom', user_ini, 'HGame.Harry', 'bInvertBroomPitch'),
                  ('forward_g', user_ini, 'Engine.Input', 'G'),
                  ('object_detail', user_ini, 'Engine.PlayerPawn', 'ObjectDetail'),
                  ('show_fps', game_ini, 'SDLDrv.SDLClient', 'ShowFPS'),
                  ('brightness', game_ini, 'SDLDrv.SDLClient', 'Brightness'),
                  ('texture_detail', game_ini, 'SDLDrv.SDLClient', 'TextureDetail'),
                  ('width', game_ini, 'SDLDrv.SDLClient', 'WindowedViewportX'),
                  ('height', game_ini, 'SDLDrv.SDLClient', 'WindowedViewportY'),
                  ('music', game_ini, 'ALAudio.ALAudioSubsystem', 'MusicVolume'),
                  ('sound', game_ini, 'ALAudio.ALAudioSubsystem', 'SoundVolume')]
        result = {}
        for name, path, section, key in fields:
            try:
                result[name] = read_value(path, section, key)
            except (configparser.Error, OSError):
                result[name] = None
        return result

    phase1 = snapshot()
    (artifacts / 'settings-before-restart.json').write_text(json.dumps(phase1, indent=2))
    (artifacts / 'User-before-restart.ini').write_text(user_ini.read_text(encoding='utf-8'))
    (artifacts / 'Game-before-restart.ini').write_text(game_ini.read_text(encoding='utf-8'))
    engine_between = sha256(engine)
    restart_output, restart_status, restart_timed_out, restart_error = b'', None, False, 'first phase incomplete or executable changed'
    restart_discarded, restart_prune_errors = [], []
    if status == 0 and engine_before == engine_between and phase1['width'] == '640' and phase1['height'] == '480':
        restart_text = RESTART_FIXTURE.read_text(encoding='utf-8')
        restart_tick, restart_frames = replay.parse_fixture(restart_text)
        replay.validate_launch_safe_frames(restart_frames)
        (user / 'InputScriptSmoke.rep').write_bytes(replay.compile_replay(args.map, restart_tick, restart_frames))
        (artifacts / 'restart-fixture.txt').write_text(restart_text)
        restart_command = replay.build_launch_command(engine, data, len(restart_frames) + 80)
        if args.audio and '-nosound' in restart_command:
            restart_command.remove('-nosound')
        restart_command.append(f'-INI="{game_ini}"')
        (artifacts / 'restart-command.json').write_text(json.dumps(restart_command, indent=2))
        os.environ['HP2_CAPTURE_FRAMES'] = str(artifacts / 'restart-frames')
        (restart_output, restart_status, restart_timed_out, restart_error), restart_discarded, restart_prune_errors = supervised_checkpoints(
            restart_command, home=home, cwd=ROOT, timeout_seconds=180, directory=artifacts / 'restart-frames',
            keep=set(RESTART_CHECKPOINTS.values()))
        (artifacts / 'restart.log').write_bytes(restart_output)
    phase2 = snapshot()
    engine_after = sha256(engine)
    after = identity(data, inventory)
    (artifacts / 'inventory-after.json').write_text(json.dumps(after, indent=2))
    final_invert = phase1['invert']
    final_brightness = phase1['brightness']
    failure_markers = replay.game_test._failure_markers(output, ROOT, home) + replay.game_test._failure_markers(restart_output, ROOT, home)
    restart_log = restart_output.decode('utf-8', errors='replace')
    transitions = ['OpenBook INGAME', 'ChangePage Transient.FEInputPage',
                   'ChangePage Transient.FEInGamePage', 'ChangePage Transient.FESoundVideoPage',
                   'ChangePage Transient.FEInGamePage', 'CloseBook Called']
    cursor = 0
    missing = []
    for marker in transitions:
        position = log.find(marker, cursor)
        if position < 0:
            missing.append(marker)
        else:
            cursor = position + len(marker)
    shots = sorted((artifacts / 'frames').glob('frame_*.png'))
    checkpoints = {name: artifacts / 'frames' / f'frame_{index:06}.png'
                   for name, index in FIRST_CHECKPOINTS.items()}
    checkpoints.update({name: artifacts / 'restart-frames' / f'frame_{index:06}.png'
                        for name, index in RESTART_CHECKPOINTS.items()})
    capture_policy = {'retained_first': FIRST_CHECKPOINTS, 'retained_restart': RESTART_CHECKPOINTS,
                      'discarded_first': first_discarded, 'discarded_restart': restart_discarded,
                      'errors': first_prune_errors + restart_prune_errors,
                      'note': 'frame_meta.json describes every captured frame; named checkpoints, startup, and the final 2 frames are retained'}
    (artifacts / 'capture-policy.json').write_text(json.dumps(capture_policy, indent=2) + '\n')
    missing_checkpoints = [name for name, path in checkpoints.items()
                           if not path.is_file() or path.stat().st_size == 0]
    failures = []
    if status != 0 or timed_out or launch_error:
        failures.append('process failed or timed out')
    if not engine_before == engine_between == engine_after:
        failures.append('executable changed during the run')
    if missing:
        failures.append('missing ordered page transitions: ' + ', '.join(missing))
    expected = {'invert': 'True', 'auto_jump': 'True', 'auto_center': 'False',
                'auto_quaff': 'False', 'invert_broom': 'True', 'texture_detail': 'Medium',
                'width': '640', 'height': '480', 'show_fps': 'True'}
    for key, value in expected.items():
        if phase1[key] != value:
            failures.append(f'{key}: expected {value}, got {phase1[key]}')
    if not phase1['object_detail'] or phase1['object_detail'] == 'ObjectDetailVeryHigh':
        failures.append('object detail did not decrease')
    if not phase1['forward_g'] or 'MOVEFORWARD' not in phase1['forward_g'].upper():
        failures.append('G binding did not persist MoveForward')
    if not final_brightness or float(final_brightness) <= float(initial_brightness):
        failures.append('brightness did not persist an increase')
    if not phase1['music'] or float(phase1['music']) >= 0.53:
        failures.append('music volume did not decrease')
    if not phase1['sound'] or float(phase1['sound']) >= 0.90:
        failures.append('sound volume did not decrease')
    if restart_status != 0 or restart_timed_out or restart_error:
        failures.append('restart failed or did not run')
    if phase1 != phase2:
        failures.append('settings changed across restart')
    for marker in ['ChangePage Transient.FEInputPage', 'ChangePage Transient.FESoundVideoPage']:
        if marker not in restart_log:
            failures.append('restart missing ' + marker)
    input_markers = ['MouseInvert changed to True', 'AutoCenterCam changed to False',
                     'MouseInvert changed to False', 'MouseInvert changed to True',
                     'AutoJumpChanged changed to True', 'bAutoQuaff changed to False',
                     "attempt to set as 71:G"]
    cursor = 0
    for marker in input_markers:
        position = log.find(marker, cursor)
        if position < 0:
            failures.append('missing ordered input evidence: ' + marker)
        else:
            cursor = position + len(marker)
    resolution_events = ['Screen Settings Changed', 'HPMessageBox Setup message=, timeOut=14.000000',
                         'HPMessageBox button clicked No', 'HPMessageBox Close , Result= No',
                         'HPMessageBox Setup message=, timeOut=0.000000', 'HPMessageBox button clicked OK',
                         'Screen Settings Changed', 'HPMessageBox Setup message=, timeOut=14.000000',
                         'HPMessageBox button clicked Yes', 'HPMessageBox Close , Result= Yes']
    cursor = 0
    for marker in resolution_events:
        position = log.find(marker, cursor)
        if position < 0:
            failures.append('resolution event sequence missing ' + marker)
        else:
            cursor = position + len(marker)
    if args.audio and 'ALAudio: subsystem initialized.' not in log:
        failures.append('audio subsystem initialization not observed')
    if args.audio and 'Playing Streaming sound' not in log:
        failures.append('streaming audio playback not observed')
    if failure_markers:
        failures.append('engine failure markers observed')
    if first_prune_errors or restart_prune_errors:
        failures.append('capture pruning failed; check capture-policy.json')
    if missing_checkpoints:
        failures.append('missing renderer checkpoints: ' + ', '.join(missing_checkpoints))
    expected_dimensions = {'book': (800, 600), 'resolution_confirm': (640, 480),
                           'resolution_reverted': (800, 600), 'resolution_accepted': (640, 480),
                           'fps_enabled': (640, 480), 'color_depth_list': (640, 480),
                           'resumed': (640, 480), 'restart_input': (640, 480), 'restart_video': (640, 480)}
    checkpoint_dimensions = {}
    for name, expected_size in expected_dimensions.items():
        try:
            actual_size = png_dimensions(checkpoints[name])
            checkpoint_dimensions[name] = actual_size
            if actual_size != expected_size:
                failures.append(f'{name} dimensions {actual_size}, expected {expected_size}')
        except (OSError, ValueError) as error:
            failures.append(f'{name} invalid capture: {error}')
    if before != after or not all(row['matches'] for row in after):
        failures.append('retail copy changed')
    cut_errors = [line for line in (log + '\n' + restart_log).splitlines() if 'CutError:' in line and not line.startswith('Destroying ')]
    if cut_errors:
        failures.append('stock cutscene errors observed; not waived')
    report = {'status': 'fail' if failures else 'pass', 'data_profile': 'data-retail', 'map': args.map,
              'inventory_count': len(before), 'inventory_matches_after': all(r['matches'] for r in after),
              'engine_sha256_before': engine_before, 'engine_sha256_between': engine_between,
              'engine_sha256_after': engine_after,
              'process_exit': status, 'timed_out': timed_out, 'launch_error': launch_error,
              'restart_exit': restart_status, 'restart_timed_out': restart_timed_out, 'restart_error': restart_error,
              'settings_before_restart': phase1, 'settings_after_restart': phase2,
              'missing_transitions': missing, 'invert_before': initial_invert, 'invert_after': final_invert,
              'brightness_before': initial_brightness, 'brightness_after': final_brightness,
              'retained_first_frames': len(shots), 'discarded_first_frames': len(first_discarded),
              'retained_restart_frames': len(list((artifacts / 'restart-frames').glob('frame_*.png'))),
              'discarded_restart_frames': len(restart_discarded), 'failure_markers': failure_markers,
              'engine_diagnostics': [line for line in (log + '\n' + restart_log).splitlines()
                                     if line.startswith('hp2_game:')],
              'visual_checkpoints': {name: str(path) for name, path in checkpoints.items()},
              'missing_visual_checkpoints': missing_checkpoints, 'checkpoint_dimensions': checkpoint_dimensions,
              'cut_errors': cut_errors, 'failures': failures,
              'audio_enabled': args.audio,
              'audio_log_lines': [line for line in (log + '\n' + restart_log).splitlines()
                                  if any(token in line.lower() for token in ('openal', 'alaudio:', 'audio initialized', 'audio subsystem', 'playing streaming sound'))][:40],
              'limitations': ['frames require visual review; resolution event logs do not prove displayed labels', 'live renderer texture LOD not independently measured', 'replay is not physical input',
                              'audio samples and audibility not measured' if args.audio else 'nosound; audible output not tested',
                              'inventory does not prove original-media authenticity']}
    (artifacts / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
