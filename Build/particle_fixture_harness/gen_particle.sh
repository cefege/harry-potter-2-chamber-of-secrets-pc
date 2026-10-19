#!/bin/sh
#   crates/hp2-engine/tests/fixtures/particle_update.txt  (UParticle::Update records, "upd")
#   crates/hp2-engine/tests/fixtures/particle_list.txt    (UParticleListPriv operations, "list")
#   crates/hp2-engine/tests/fixtures/particle_trig.txt    (GMath.TrigFLOAT table, "trig")
#
# Usage: gen_particle.sh <build-dir-with-compile_commands.json> [git-ref]
# Builds the ORIGINAL UnParticle.cpp / UnParticleList.cpp / UnMath.cpp from git-ref (default
# 452b122), links them with the harness, and writes the three fixtures.
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-452b122}
FIX=$REPO/crates/hp2-engine/tests/fixtures
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
python3 - "$CC_DB" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, shlex, sys
db, work = sys.argv[1:3]
e = [x for x in json.load(open(db)) if x['file'].endswith('UnParticle.cpp')][0]
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
SRC=$WORK/HarryPotter2/Unreal/Engine/Src
CORE=$WORK/HarryPotter2/Unreal/Core/Src
cd "$SRC"
eval "c++ $FLAGS -I$SRC -O3 -g -fno-access-control -c $REPO/Build/particle_fixture_harness/particle_fixture_harness.cpp -o $WORK/harness.o"
eval "c++ $FLAGS -O3 -g -c UnParticle.cpp -o $WORK/unparticle.o"
eval "c++ $FLAGS -I$SRC -O3 -g -c $CORE/UnMath.cpp -o $WORK/unmath.o"
OBJS="$WORK/harness.o $WORK/unparticle.o $WORK/unmath.o"

# Stub only the engine symbols the harness objects reference that nothing defines. Calling a stub
# aborts with its name, so an unexpected engine call is visible rather than silently wrong.
# libc/libm and C++ runtime symbols stay unresolved so they link to the real libraries.
nm -g --defined-only $OBJS | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
nm -u $OBJS | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__chkstk|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt|_ZNSt|_ZNKSt|_ZTVN10__cxxabi|_ZTIN10__cxxabi|_ZTSN10__cxxabi|_ZTVSt|_ZTISt|_ZTSSt|_Znw|_Znam|_ZdlPv|_ZdaPv|__Z|_?bzero|memcpy|memset|memmove|memcmp|malloc|calloc|realloc|free|abort|qsort|printf|fprintf|puts|fputs|fwrite|exit|_exit|strcmp|strlen|sin|cos|sqrt|tan|acos|asin|atan|atan2|fmod|floor|ceil|pow|exp|log|fabs|__error|___|_strlen|_stack_chk|dyld|_dyld|__std|putchar|_ZT[IS][iPc])' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
echo "stubbed symbols:" >&2
cat "$WORK/syms.txt" >&2
python3 -c "
import sys
syms=[l.strip() for l in open(sys.argv[1]) if l.strip()]
# Only _Z functions/data and G[A-Z] engine globals are stubbed. Other C names stay unresolved
# so they link to libSystem (a missing one fails at link time, by name).
is_g_global=lambda s: len(s) > 1 and s[0] == 'G' and s[1].isupper()
syms=[s for s in syms if s.startswith('_Z') or is_g_global(s)]
out=['#include <stdio.h>', '#include <stdlib.h>', 'static void stub_fail(const char* s) { fprintf(stderr, \"particle harness: unresolved engine symbol called: %s\\\\n\", s); abort(); }']
def is_data(s):
    # Non-_Z names here are G[A-Z] engine globals (GMalloc, GError, ...): zeroed storage. _Z names
    # are data only for vtables, typeinfo, static class pointers and the Env singleton; every other
    # _Z name is a function.
    if not s.startswith('_Z'):
        return True
    return s.endswith('PrivateStaticClassE') or s.startswith(('_ZTV', '_ZTI', '_ZTS', '_ZN9__Context'))
for i,s in enumerate(syms):
    if is_data(s):
        out.append(f'unsigned char stub{i}[4096] __asm__(\"_{s}\") __attribute__((aligned(16)));')
    else:
        out.append(f'void stub{i}() __asm__(\"_{s}\");')
        out.append(f'void stub{i}() {{ stub_fail(\"{s}\"); }}')
print(chr(10).join(out))
" "$WORK/syms.txt" > "$WORK/stubs.cpp"
c++ -O1 -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ $OBJS "$WORK/stubs.o" -o "$WORK/particle_fixture"
"$WORK/particle_fixture" > "$WORK/out.txt"

grep -E '^upd ' "$WORK/out.txt" | cut -c5- > "$WORK/update.txt"
grep -E '^list ' "$WORK/out.txt" | cut -c6- > "$WORK/list.txt"
grep -E '^trig ' "$WORK/out.txt" | cut -c6- > "$WORK/trig.txt"
mv "$WORK/update.txt" "$FIX/particle_update.txt"
mv "$WORK/list.txt" "$FIX/particle_list.txt"
mv "$WORK/trig.txt" "$FIX/particle_trig.txt"
echo "records: update=$(wc -l < "$FIX/particle_update.txt" | tr -d ' ') list=$(wc -l < "$FIX/particle_list.txt" | tr -d ' ') trig=$(wc -l < "$FIX/particle_trig.txt" | tr -d ' ')"
