#!/usr/bin/env python3
"""Compile fake-only picker harness and retain one process per scenario."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent / 'Fixtures' / 'PickerSafety'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--source', type=Path, required=True)
    p.add_argument('--rustc', type=Path, required=True)
    p.add_argument('--clang', type=Path, required=True)
    args = p.parse_args()
    root = Path(os.environ['HP2_ARTIFACT_DIR']).resolve()
    root.mkdir(parents=True, exist_ok=True)
    out = Path(tempfile.mkdtemp(prefix='picker-fake-', dir=root))
    source = args.source.resolve(strict=True)
    rustc = args.rustc.resolve(strict=True)
    clang = args.clang.resolve(strict=True)
    for src, dst in [(source, 'picker.rs'), (HERE / 'harness.rs', 'harness.rs'),
                     (HERE / 'fake_nfd.c', 'fake_nfd.c')]:
        shutil.copyfile(src, out / dst)
    identity = {'source': str(source), 'source_sha256': sha(out / 'picker.rs'),
                'harness_sha256': sha(out / 'harness.rs'), 'shim_sha256': sha(out / 'fake_nfd.c'),
                'rustc': str(rustc), 'rustc_sha256': sha(rustc),
                'clang': str(clang), 'clang_sha256': sha(clang), 'expect': 'fixed'}
    commands = [
        [str(rustc), '--version', '--verbose'],
        [str(clang), '--version'],
        [str(clang), '-std=c11', '-c', str(out / 'fake_nfd.c'), '-o', str(out / 'fake_nfd.o')],
        [str(rustc), '--edition=2024', '--cfg', 'feature="nfd"', '--cfg', 'feature="sdl"',
         str(out / 'harness.rs'), '-C', 'link-arg=' + str(out / 'fake_nfd.o'), '-o', str(out / 'harness')],
    ]
    def run(name, cmd):
        try:
            result = subprocess.run(cmd, text=True, capture_output=True, timeout=30, check=False)
            status = result.returncode
            stdout, stderr = result.stdout, result.stderr
        except subprocess.TimeoutExpired as error:
            status = None
            stdout = (error.stdout or b'').decode(errors='replace') if isinstance(error.stdout, bytes) else (error.stdout or '')
            stderr = (error.stderr or b'').decode(errors='replace') if isinstance(error.stderr, bytes) else (error.stderr or '')
        record = {'command': cmd, 'returncode': status, 'stdout': stdout, 'stderr': stderr}
        (out / (name + '.json')).write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
        return status
    for index, cmd in enumerate(commands):
        if run('setup-' + str(index), cmd) != 0:
            identity.update({'passed': False, 'failed_setup': index, 'output': str(out)})
            (out / 'result.json').write_text(json.dumps(identity, indent=2) + '\n', encoding='utf-8')
            print(json.dumps(identity, indent=2)); return 1
    controls = ['pthread'] + [kind + '-' + picker for kind in ('success', 'cancel', 'error', 'null', 'initfail') for picker in ('folder', 'file')]
    regressions = ['wrong-' + picker for picker in ('folder', 'file')] + [
        'nested-' + stage + '-' + picker for stage in ('dialog', 'free', 'quit', 'log') for picker in ('folder', 'file')]
    extended = (['initlog-' + picker for picker in ('folder', 'file')]
                + ['filter-' + case + '-file' for case in ('empty', 'order', 'nul')]
                + ['nested-cross-dialog-' + picker for picker in ('folder', 'file')])
    results = {}
    for scenario in controls + regressions + extended:
        status = run(scenario, [str(out / 'harness'), scenario])
        results[scenario] = {'returncode': status, 'expected': 0, 'matched': status == 0}
    identity.update({'output': str(out), 'scenarios': results,
                     'passed': all(result['matched'] for result in results.values())})
    (out / 'result.json').write_text(json.dumps(identity, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(identity, indent=2))
    return 0 if identity['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
