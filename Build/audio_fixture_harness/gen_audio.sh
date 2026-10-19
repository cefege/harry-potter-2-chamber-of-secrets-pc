#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/audio_fixtures.txt from the ORIGINAL
# UnAudio.cpp (pinned pre-port commit ff45170, before the Rust port; copied into the work
# dir as UnAudio.oracle.cpp and #included by the harness, -O3 -g). The work dir is that
# tree, so the oracle sees the original UnAudio.h with the wave methods still declared.
# Usage: gen_audio.sh <build-dir-with-compile_commands.json>
#
# Work-dir copies drop the IMPLEMENT_CLASS lines: they only build UClass objects and
# register natives at static-init time, which needs the whole object system. No fixture
# reads them; function bodies are untouched.
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
ORACLE_REF=ff45170
ENGINE_SRC=HarryPotter2/Unreal/Engine/Src
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$ORACLE_REF" | tar -x -C "$WORK"
git -C "$REPO" show "$ORACLE_REF:$ENGINE_SRC/UnAudio.cpp" | tr -d '\r' | grep -v -E '^IMPLEMENT_CLASS\(' > "$WORK/$ENGINE_SRC/UnAudio.oracle.cpp"
python3 - "$CC_DB" "$REPO" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, shlex, sys
db, repo, work = sys.argv[1:4]
e = [x for x in json.load(open(db)) if x['file'].endswith('UnAudio.cpp')][0]
toks = shlex.split(e['command'].replace(repo, work))
out, skip = [], False
for t in toks:
    if skip: skip = False; continue
    if t in ('-o', '-MF', '-MT'): skip = True; continue
    if t in ('-MD', '-MMD', '-c') or t.startswith('-O') or t.endswith('.cpp') or t.endswith('c++'): continue
    out.append(t)
print(' '.join(shlex.quote(t) for t in out))
PY
FLAGS=$(cat "$WORK/flags.txt")
cd "$WORK/$ENGINE_SRC"
eval "c++ $FLAGS -I$WORK/$ENGINE_SRC -O3 -g -c $REPO/Build/audio_fixture_harness/audio_fixture_harness.cpp -o $WORK/harness.o"
# Stub list: symbols the object references but does not define, that the linker reports as
# missing (it prints demangled names, so both forms are matched). Names the system libraries
# provide never appear in the report, so they are never shadowed.
if c++ "$WORK/harness.o" -o "$WORK/probe" 2> "$WORK/ld.log"; then
  : > "$WORK/stubs.txt"
else
  python3 - "$WORK/harness.o" "$WORK/ld.log" > "$WORK/stubs.txt" <<'PY'
import re, subprocess, sys
objs, log = sys.argv[1:3], sys.argv[2]
def names(args):
    out = subprocess.run(['nm'] + args + [objs[0]], capture_output=True, text=True, check=True).stdout
    return [l.split()[-1] for l in out.splitlines() if l.strip()]
undef = set(n for n in names(['-u']) if n.startswith('_'))
defined = set(names(['-gU']))
undef -= defined
reported = set(m.group(1) for l in open(log) for m in [re.match(r'^  "(.*)", referenced from:', l)] if m)
demangled = subprocess.run(['c++filt'], input='\n'.join(sorted(undef)) + '\n', capture_output=True, text=True, check=True).stdout.splitlines()
pairs = dict(zip(sorted(undef), demangled))
for m in sorted(undef):
    d = pairs[m]
    if m in reported or m[1:] in reported or d in reported:
        print(m)
PY
fi
python3 -c "
import sys
syms=[l.strip() for l in open(sys.argv[1]) if l.strip()]
print(''.join(f'__attribute__((used)) char stub{i}[64] __asm__(\"{s}\");' + chr(10) for i,s in enumerate(syms)))
" "$WORK/stubs.txt" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/stubs.o" -o "$WORK/audio_fixture"
mkdir -p "$REPO/crates/hp2-engine/tests/fixtures"
"$WORK/audio_fixture" > "$WORK/audio_fixtures.txt"
cp "$WORK/audio_fixtures.txt" "$REPO/crates/hp2-engine/tests/fixtures/audio_fixtures.txt"
echo "stubbed symbols: $(wc -l < "$WORK/stubs.txt")"
echo "wrote $(wc -l < "$REPO/crates/hp2-engine/tests/fixtures/audio_fixtures.txt") records"
