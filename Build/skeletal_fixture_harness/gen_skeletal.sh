#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/skeletal_{math,trig,apply_anim,frame,root,bounds}.txt
# from the ORIGINAL UnSkeletalMesh.cpp (-O3, product flags from compile_commands.json,
# plus -g), linked with the ORIGINAL Core UnMath.cpp, UnAnsi.cpp and UnCache.cpp
# (same flag source) and Build/skeletal_fixture_harness/skeletal_harness.cpp.
# Sources come from a git snapshot of the oracle ref (never the working tree).
# Unreached out-of-line functions link to stubs that print their symbol name and
# abort; undefined data links to zeroed storage. Output is byte-identical across runs.
# Usage: gen_skeletal.sh <build-dir-with-compile_commands.json> [git-ref]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-32dcfda}
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
[ -n "${KEEP_WORK:-}" ] && echo "WORK=$WORK" >&2
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
SRC="$WORK/HarryPotter2/Unreal"
SDK=$(xcrun --show-sdk-path)

# flags_for <basename>: product flags for that TU, with paths rewritten into WORK.
flags_for() {
python3 - "$CC_DB" "$REPO" "$WORK" "$1" <<'PY'
import json, shlex, sys
db, repo, work, name = sys.argv[1:5]
e = [x for x in json.load(open(db)) if x['file'].endswith('/' + name)][0]
toks = shlex.split(e['command'].replace(repo, work))
out, skip = [], False
for t in toks:
    if skip: skip = False; continue
    if t in ('-o', '-MF', '-MT'): skip = True; continue
    if t in ('-MD', '-MMD', '-c') or t.startswith('-O') or t.endswith('.cpp') or t.endswith('c++'): continue
    out.append(t)
print(' '.join(shlex.quote(t) for t in out))
PY
}

ENG_FLAGS=$(flags_for UnSkeletalMesh.cpp)
MATH_FLAGS=$(flags_for UnMath.cpp)
ANSI_FLAGS=$(flags_for UnAnsi.cpp)
CACHE_FLAGS=$(flags_for UnCache.cpp)
UNIX_FLAGS=$(flags_for UnUnix.cpp)

cd "$SRC/Engine/Src"
eval "c++ $ENG_FLAGS -O3 -g -c UnSkeletalMesh.cpp -o $WORK/UnSkeletalMesh.o"
eval "c++ $ENG_FLAGS -O3 -g -c $REPO/Build/skeletal_fixture_harness/skeletal_harness.cpp -o $WORK/harness.o"
cd "$SRC/Core/Src"
eval "c++ $MATH_FLAGS -O3 -g -c UnMath.cpp -o $WORK/UnMath.o"
eval "c++ $ANSI_FLAGS -O3 -g -c UnAnsi.cpp -o $WORK/UnAnsi.o"
eval "c++ $CACHE_FLAGS -O3 -g -c UnCache.cpp -o $WORK/UnCache.o"
eval "c++ $UNIX_FLAGS -O3 -g -c UnUnix.cpp -o $WORK/UnUnix.o"

OBJS="$WORK/harness.o $WORK/UnSkeletalMesh.o $WORK/UnMath.o $WORK/UnAnsi.o $WORK/UnCache.o $WORK/UnUnix.o"

# Symbols defined anywhere in the link set.
for o in $OBJS; do nm -g --defined-only "$o" | awk '{print $NF}'; done | sort -u > "$WORK/defined.txt"
# Undefined symbols across the link set.
for o in $OBJS; do nm -u "$o" | awk '{print $NF}'; done | sort -u > "$WORK/undef.txt"
# System exports (libSystem, libc++, libc++abi) are provided by the SDK, not stubbed.
for t in "$SDK/usr/lib/libSystem.tbd" "$SDK/usr/lib/libc++.tbd" "$SDK/usr/lib/libc++abi.tbd"; do
  [ -f "$t" ] && grep -o '_[A-Za-z0-9_$.]*' "$t"
done | sort -u > "$WORK/sdk.txt"
comm -23 "$WORK/undef.txt" "$WORK/defined.txt" | comm -23 - "$WORK/sdk.txt" > "$WORK/syms.txt"
c++filt < "$WORK/syms.txt" > "$WORK/demangled.txt"

# Function-call relocations (BR26 = branch) mark out-of-line functions; everything else is data.
for o in $OBJS; do xcrun objdump --macho --reloc "$o"; done \
  | awk '$5 == "BR26" {print $NF}' | sort -u > "$WORK/funcs.txt"

python3 - "$WORK/syms.txt" "$WORK/funcs.txt" "$WORK/demangled.txt" > "$WORK/stubs.cpp" <<'PY'
import sys
syms = [l.strip() for l in open(sys.argv[1]) if l.strip()]
funcs = set(l.strip() for l in open(sys.argv[2]) if l.strip())
dem = [l.rstrip('\n') for l in open(sys.argv[3])]
data_prefix = ('_G', '__ZTV', '__ZTI', '__ZTS', '__ZTT', '__ZZ', '__ZGV')
print('#include <cstdio>\n#include <cstdlib>')
print('extern "C" void skel_unreached(const char* Name) { fprintf(stderr, "unreached: %s\\n", Name); abort(); }')
for i, s in enumerate(syms):
    is_func = (s in funcs or (s.startswith('__Z') and '(' in dem[i])) and not s.startswith(data_prefix)
    if is_func:
        print('extern "C" void stubF%d(void) __asm__("%s");' % (i, s))
        print('extern "C" void stubF%d(void) { skel_unreached("%s"); }' % (i, s))
    else:
        print('__attribute__((used)) alignas(16) char stubD%d[65536] __asm__("%s");' % (i, s))
PY
c++ -O1 -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ $OBJS "$WORK/stubs.o" -o "$WORK/skeletal_harness"

OUT="$REPO/crates/hp2-engine/tests/fixtures"
mkdir -p "$OUT"
for fam in math trig apply_anim frame root bounds; do
  case $fam in
    math) name=skeletal_math.txt ;;
    trig) name=skeletal_trig.txt ;;
    apply_anim) name=skeletal_apply_anim.txt ;;
    frame) name=skeletal_frame.txt ;;
    root) name=skeletal_root.txt ;;
    bounds) name=skeletal_bounds.txt ;;
  esac
  "$WORK/skeletal_harness" "$fam" > "$WORK/$name"
  mv "$WORK/$name" "$OUT/$name"
  echo "wrote $(wc -l < "$OUT/$name") lines to $name"
done
