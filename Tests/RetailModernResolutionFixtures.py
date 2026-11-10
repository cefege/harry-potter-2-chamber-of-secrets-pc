#!/usr/bin/env python3
"""Prepare, but do not launch, retail 1280x720 resolution replay fixtures.

Coordinates are game-viewport replay coordinates, not OS mouse coordinates.
Calibrate them from the matching snapshot and popup capture after the display
and scale changes stabilize. Each scenario requires its own isolated home.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path

import InputScriptSmoke as replay


def point(value: str) -> tuple[int, int]:
    try:
        x, y = (int(v) for v in value.split(','))
    except ValueError as exc:
        raise argparse.ArgumentTypeError('expected X,Y') from exc
    if x < 0 or y < 0:
        raise argparse.ArgumentTypeError('coordinates must be non-negative')
    return x, y


def click(p: tuple[int, int]) -> str:
    return f'mouse {p[0]} {p[1]}\npress LeftMouse\n'


def build_fixtures(video, combo, choice, yes, no, restart_modern, scroll=()):
    for name, p, bounds in [('video', video, (800, 600)), ('combo', combo, (800, 600)),
                            ('choice', choice, (800, 600)), ('yes', yes, (1280, 720)),
                            ('no', no, (1280, 720)), ('restart_modern', restart_modern, (1280, 720))]:
        if not (0 <= p[0] < bounds[0] and 0 <= p[1] < bounds[1]):
            raise ValueError(f'{name} is outside the {bounds[0]}x{bounds[1]} logical viewport')
    if any(not (0 <= x < 800 and 0 <= y < 600) for x, y in scroll):
        raise ValueError('scroll point is outside the 800x600 logical viewport')
    start = 'tick 0.033333335\nidle 60\npress Enter\nidle 40\npress Escape\nidle 80\n'
    open_video = click(video) + 'idle 50\n'
    popup = click(combo) + 'idle 10\n'
    for position in scroll:
        popup += click(position) + 'idle 8\n'
    select = click(choice) + 'idle 40\n'
    prefix = start + open_video + popup + select
    fixtures = {
        'accept.txt': prefix + click(yes) + 'idle 80\n',
        'reject.txt': prefix + click(no) + 'idle 80\n',
        'timeout.txt': prefix + 'idle 760\n',
        'restart-accepted.txt': start + click(restart_modern) + 'idle 80\n',
        'restart-restored.txt': start + click(video) + 'idle 80\n',
    }
    for content in fixtures.values():
        tick, frames = replay.parse_fixture(content)
        replay.validate_launch_safe_frames(frames)
        replay.compile_replay('PrivetDr.unr', tick, frames)
    return fixtures


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output-dir', type=Path, required=True, help='new directory, preferably under HP2_ARTIFACT_DIR')
    for option in ('video', 'combo', 'choice', 'yes', 'no', 'restart-modern'):
        p.add_argument('--' + option, type=point, required=True, metavar='X,Y')
    p.add_argument('--scroll', type=point, action='append', default=[], metavar='X,Y',
                   help='optional popup scrollbar click, repeated in order')
    a = p.parse_args()
    try:
        fixtures = build_fixtures(a.video, a.combo, a.choice, a.yes, a.no, a.restart_modern, a.scroll)
    except ValueError as exc:
        p.error(str(exc))
    a.output_dir.mkdir(parents=True, exist_ok=False)
    for name, content in fixtures.items():
        (a.output_dir / name).write_text(content, encoding='utf-8')
    manifest = {
        'status': 'prepared-not-run', 'target_resolution': [1280, 720], 'initial_resolution': [800, 600],
        'data_profile_required': 'data-retail',
        'coordinates': {'video': a.video, 'combo': a.combo, 'choice': a.choice, 'yes': a.yes,
                        'no': a.no, 'restart_modern': a.restart_modern, 'scroll': a.scroll},
        'matrix': [
            {'display_mode': mode, 'first': first + '.txt',
             'restart': 'restart-accepted.txt' if first == 'accept' else 'restart-restored.txt',
             'expected_resolution_after_restart': [1280, 720] if first == 'accept' else [800, 600]}
            for mode in ('windowed', 'fullscreen-desktop') for first in ('accept', 'reject', 'timeout')],
        'requirements': [
            'Wait for display-mode and scale changes; use an explicitly pinned binary and hash.',
            'Calibrate the popup row and all click points on that exact snapshot and display.',
            'Run every matrix row in a separate isolated home; use a disposable verified retail copy.',
            'Check the popup visibly contains 1280x720 before interpreting a selection.',
            'Capture the selection, confirmation, result, and restart; inspect displayed labels.',
            'Verify executable hash before, between, and after processes and the retail inventory before and after.',
            'Check active mode INI, actual logical render size, surface presentation, and restart independently.',
            'Retain and report CutError and occluded frames; never overwrite earlier reports.',
            'Timeout is runtime-dependent; prove the timer elapsed with log and replay timing evidence.',
            'Replay coordinates do not verify physical SDL mouse alignment.',
        ]}
    (a.output_dir / 'plan.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    print(f'Prepared {len(fixtures)} fixtures in {a.output_dir}; no game was launched.')


if __name__ == '__main__':
    main()
