#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/cam_mgr_exec.txt from the ORIGINAL
# UnCamMgr.cpp at the pinned ref (git archive, not the working tree).
#
# Usage: gen_cam_mgr.sh <build-dir-with-compile_commands.json> [git-ref]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-5782e7f}
FIX=$REPO/crates/hp2-engine/tests/fixtures
WORK=$(mktemp -d)
trap '[ $? -eq 0 ] && rm -rf "$WORK"' EXIT

git -C "$REPO" archive "$REF" | tar -x -C "$WORK"

python3 - "$CC_DB" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, shlex, sys
db, work = sys.argv[1:3]
e = [x for x in json.load(open(db)) if x['file'].endswith('UnCamMgr.cpp') and 'Engine/Src' in x['file']][0]
source_root = e['file'].split('/HarryPotter2/')[0]
command = e['command'].replace(source_root + '/HarryPotter2', work + '/HarryPotter2')
toks = shlex.split(command)
out, skip = [], False
for t in toks:
    if skip: skip = False; continue
    if t in ('-o', '-MF', '-MT'): skip = True; continue
    if t in ('-MD', '-MMD', '-c') or t.startswith('-O') or t.endswith('.cpp') or t.endswith('c++'): continue
    out.append(t)
print(' '.join(shlex.quote(t) for t in out))
PY
FLAGS=$(cat "$WORK/flags.txt")
cd "$WORK/HarryPotter2/Unreal/Engine/Src"
eval "c++ $FLAGS -O3 -g -c UnCamMgr.cpp -o $WORK/cam_mgr.o"
eval "c++ $FLAGS -O3 -g -c $REPO/Build/cam_mgr_fixture_harness/cam_mgr_harness.cpp -o $WORK/harness.o"

# Stub every undefined symbol that neither object defines and that libc/libc++ does not
# provide. nm names keep their Mach-O leading underscore until the label is written.
# Function-like engine symbols get a stub that prints its name and aborts. Data-like
# engine symbols (vtables, typeinfo, class globals, G* globals) get a zeroed blob.
python3 - "$WORK" > "$WORK/stubs.cpp" <<'PY'
import re, subprocess, sys
work = sys.argv[1]
objs = [f'{work}/cam_mgr.o', f'{work}/harness.o']

def names(args):
    out = subprocess.run(['nm', *args], capture_output=True, text=True, check=True).stdout
    return {p[-1] for p in (l.split() for l in out.splitlines()) if p}

defined, undef = set(), set()
for o in objs:
    defined |= names(['-g', '--defined-only', o])
    undef |= names(['-u', o])

runtime = re.compile(
    r'^(tan|atan|atan2|mem[a-z]+|str[a-z]+|wcs[a-z]+|vswprintf|swprintf|vsnprintf|snprintf|'
    r'v?printf|f?puts|put[a-z]*|fprintf|fflush|fwrite|malloc|calloc|realloc|free|abort|qsort|atof|'
    r'sin|cos|sqrt|floor|ceil|pow|exp|log|fabs|fmod|bzero|towlower|towupper|iswspace|'
    r'setjmp|longjmp|__cxa_|__gxx_|_Unwind|__stack_chk|__chkstk|__error|_?_std(in|out|err)p|stdout|stderr|'
    r'_Znw|_Znam|_ZdlPv|_ZdaPv|_ZSt|_ZNSt|_ZNKSt|'
    r'_ZTV(N10__cxxabiv1|St)|_ZTI(N10__cxxabiv1|St|[a-zA-Z]$|P[a-zA-Z]$)|_ZTS(N10__cxxabiv1|St))')
data = re.compile(r'^(_ZT[VIS]|_ZN9__Context3EnvE|G[A-Z]|.*PrivateStaticClass)')

stubbed = []
lines = ['#include <stdio.h>', '#include <stdlib.h>']
for s in sorted(undef - defined):
    if not s.startswith('_'):
        continue
    name = s[1:]
    if runtime.match(name):
        continue
    stubbed.append(s)
    if data.match(name):
        lines.append(f'__attribute__((used)) char stub_{len(stubbed)}[4096] __asm__("{s}");')
    else:
        lines.append(f'static const char name_{len(stubbed)}[] = "{s}";')
        lines.append(f'__attribute__((used)) void stub_{len(stubbed)}(void) __asm__("{s}");')
        lines.append(f'void stub_{len(stubbed)}(void) {{ fputs(name_{len(stubbed)}, stderr); abort(); }}')
print('\n'.join(lines))
print('stubbed symbols: ' + ' '.join(stubbed), file=sys.stderr)
PY
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/cam_mgr.o" "$WORK/stubs.o" -o "$WORK/cam_mgr_fixture"
"$WORK/cam_mgr_fixture" > "$WORK/cam_mgr_exec.txt"
mkdir -p "$FIX"
mv "$WORK/cam_mgr_exec.txt" "$FIX/cam_mgr_exec.txt"
echo "wrote $(wc -l < "$FIX/cam_mgr_exec.txt") cam_mgr lines"
