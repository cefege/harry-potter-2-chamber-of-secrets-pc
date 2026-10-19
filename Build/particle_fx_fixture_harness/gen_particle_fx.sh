#!/bin/sh
# Regenerates the AParticleFX replay fixtures from the ORIGINAL engine sources at pinned refs
# (git archive, never the working tree):
#   UnParticleFX.cpp ............................ ref 5782e7f (oracle)
#   UnParticle.cpp, UnParticleList.cpp,
#   UnMath.cpp, UnAnsi.cpp ...................... ref 452b122
#
# Every TU is built at -O3 -g with the per-TU flags from compile_commands.json and with
# -ftrivial-auto-var-init=zero (the flag the original oracle was built with; it zero-fills
# automatic variables the oracle reads before writing, e.g. Tail, ChaosDelay, Spin in
# AddParticle). The harness is additionally built with -fno-access-control.
#
# The real UnAnsi.o supplies appPow/appExp/appSin/appCos/appSqrt/appFrand/appRandRange/appMemcpy,
# so the harness defines none of them. Symbols the link still cannot resolve are taken from the
# linker's own "Undefined symbols" list and stubbed by fixture_common.py: a function stub prints
# "STUB <name>" and aborts; data stubs are zeroed storage. A "STUB" line fails the regeneration.
#
# Usage: gen_particle_fx.sh <build-dir-with-compile_commands.json> [oracle-ref] [helper-ref]
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-5782e7f}
HELPER_REF=${3:-452b122}
FIX=$REPO/crates/hp2-engine/tests/fixtures
WORK=$(mktemp -d)
trap '[ $? -eq 0 ] && rm -rf "$WORK"' EXIT
CXXFLAGS="-O3 -g -ftrivial-auto-var-init=zero"

# extract_flags <source suffix> <tree root>: prints the compile flags for that TU, with the
# source root rewritten to the extracted tree and the compile-only/output options removed.
extract_flags() {
    python3 - "$CC_DB" "$1" "$2" <<'PY'
import json, shlex, sys
db, suffix, work = sys.argv[1:4]
e = [x for x in json.load(open(db)) if x['file'].endswith(suffix)][0]
source_root = e['file'].split('/HarryPotter2/')[0]
command = e['command'].replace(source_root + '/HarryPotter2', work + '/HarryPotter2')
out, skip = [], False
for t in shlex.split(command):
    if skip:
        skip = False
        continue
    if t in ('-o', '-MF', '-MT'):
        skip = True
        continue
    if t in ('-MD', '-MMD', '-c') or t.startswith('-O') or t.endswith('.cpp') or t.endswith('c++'):
        continue
    out.append(t)
print(' '.join(shlex.quote(t) for t in out))
PY
}

mkdir -p "$WORK/oracle" "$WORK/helpers"
git -C "$REPO" archive "$REF" | tar -x -C "$WORK/oracle"
git -C "$REPO" archive "$HELPER_REF" | tar -x -C "$WORK/helpers"

extract_flags Engine/Src/UnParticleFX.cpp "$WORK/oracle" > "$WORK/flags_fx.txt"
extract_flags Engine/Src/UnParticle.cpp "$WORK/helpers" > "$WORK/flags_part.txt"
extract_flags Engine/Src/UnParticleList.cpp "$WORK/helpers" > "$WORK/flags_list.txt"
extract_flags Core/Src/UnMath.cpp "$WORK/helpers" > "$WORK/flags_math.txt"
extract_flags Core/Src/UnAnsi.cpp "$WORK/helpers" > "$WORK/flags_ansi.txt"
extract_flags Engine/Src/UnURL.cpp "$WORK/helpers" > "$WORK/flags_url.txt"
FX_FLAGS=$(cat "$WORK/flags_fx.txt")

cd "$WORK/oracle/HarryPotter2/Unreal/Engine/Src"
eval "c++ $FX_FLAGS $CXXFLAGS -c UnParticleFX.cpp -o $WORK/UnParticleFX.o"
eval "c++ $FX_FLAGS $CXXFLAGS -fno-access-control -c $REPO/Build/particle_fx_fixture_harness/particle_fx_harness.cpp -o $WORK/harness.o"

cd "$WORK/helpers/HarryPotter2/Unreal/Engine/Src"
eval "c++ $(cat "$WORK/flags_part.txt") $CXXFLAGS -c UnParticle.cpp -o $WORK/UnParticle.o"
eval "c++ $(cat "$WORK/flags_list.txt") $CXXFLAGS -c UnParticleList.cpp -o $WORK/UnParticleList.o"
cd "$WORK/helpers/HarryPotter2/Unreal/Core/Src"
eval "c++ $(cat "$WORK/flags_math.txt") $CXXFLAGS -c UnMath.cpp -o $WORK/UnMath.o"
eval "c++ $(cat "$WORK/flags_ansi.txt") $CXXFLAGS -c UnAnsi.cpp -o $WORK/UnAnsi.o"
cd "$WORK/helpers/HarryPotter2/Unreal/Engine/Src"
eval "c++ $(cat "$WORK/flags_url.txt") $CXXFLAGS -c UnURL.cpp -o $WORK/UnURL.o"

OBJS="$WORK/harness.o $WORK/UnParticleFX.o $WORK/UnParticle.o $WORK/UnParticleList.o $WORK/UnMath.o $WORK/UnAnsi.o $WORK/UnURL.o"

# Link without stubs; the linker's undefined-symbol list drives the stub set.
if c++ $OBJS -o "$WORK/probe" 2> "$WORK/link_probe.txt"; then
    : > "$WORK/syms.txt"
else
    if ! grep -q '^Undefined symbols' "$WORK/link_probe.txt"; then
        cat "$WORK/link_probe.txt" >&2
        echo "link failed for a reason other than undefined symbols" >&2
        exit 1
    fi
    python3 - "$WORK/link_probe.txt" $OBJS > "$WORK/syms.txt" <<'PY'
import re, subprocess, sys
text = open(sys.argv[1]).read()
linker = set(re.findall(r'^\s+"([^"]+)", referenced from', text, re.M))
def names(args, o):
    out = subprocess.run(['nm', *args, o], capture_output=True, text=True, check=True).stdout
    return {p[-1] for p in (l.split() for l in out.splitlines()) if p}
defined, undef = set(), set()
for o in sys.argv[2:]:
    defined |= names(['-g', '--defined-only'], o)
    undef |= names(['-u'], o)
# ld64 prints C++ names demangled; keep a mangled symbol iff the linker reported it, raw or demangled.
cand = sorted(undef - defined)
dem = subprocess.run(['c++filt'], input='\n'.join(cand), capture_output=True, text=True, check=True).stdout.splitlines()
for m, d in zip(cand, dem):
    if m in linker or d in linker:
        print(m[1:] if m.startswith('_') else m)
PY
fi
if ! python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" $OBJS > "$WORK/stubs.cpp" 2> "$WORK/stubs.log"; then
    cat "$WORK/stubs.log" >&2
    exit 1
fi
cat "$WORK/stubs.log" >&2
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ $OBJS "$WORK/stubs.o" -o "$WORK/particle_fx_fixture"

# Two runs, byte-compared: the output must be deterministic.
run() {
    mkdir -p "$1"
    if ! "$WORK/particle_fx_fixture" "$1/emit.txt" "$1/bbox.txt" "$1/update.txt" \
            "$1/params.txt" "$1/trig.txt" "$1/sqrt.txt" 2> "$1/stderr.txt"; then
        cat "$1/stderr.txt" >&2
        echo "harness failed" >&2
        exit 1
    fi
    if grep -q '^STUB ' "$1/stderr.txt"; then
        cat "$1/stderr.txt" >&2
        echo "a stub fired" >&2
        exit 1
    fi
}
run "$WORK/run1"
run "$WORK/run2"
for f in emit.txt bbox.txt update.txt params.txt trig.txt sqrt.txt; do
    cmp "$WORK/run1/$f" "$WORK/run2/$f"
done
grep -h '^header ' "$WORK/run1/emit.txt" "$WORK/run1/bbox.txt" "$WORK/run1/update.txt" "$WORK/run1/params.txt"

mkdir -p "$FIX"
mv "$WORK/run1/emit.txt" "$FIX/particle_fx_emit.txt"
mv "$WORK/run1/bbox.txt" "$FIX/particle_fx_bbox.txt"
mv "$WORK/run1/update.txt" "$FIX/particle_fx_update.txt"
mv "$WORK/run1/params.txt" "$FIX/particle_fx_params.txt"
mv "$WORK/run1/trig.txt" "$FIX/particle_trig.txt"
mv "$WORK/run1/sqrt.txt" "$FIX/particle_fx_sqrt.txt"
for f in emit bbox update params; do
    echo "wrote $(wc -l < "$FIX/particle_fx_$f.txt") particle_fx_$f lines"
done
echo "wrote $(wc -l < "$FIX/particle_trig.txt") particle_trig lines"
echo "wrote $(wc -l < "$FIX/particle_fx_sqrt.txt") particle_fx_sqrt lines"
