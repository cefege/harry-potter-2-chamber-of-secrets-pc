#!/bin/sh
# Regenerates the levact fixtures from the ORIGINAL UnLevAct.cpp (real flags, -O3 -g, from a git
# snapshot), so they are independent of the Rust port:
#   crates/hp2-engine/tests/fixtures/levact_rotation.txt  (rotation helpers)
#   crates/hp2-engine/tests/fixtures/levact_checks.txt   (SinglePoint/MultiPoint/SingleLine/MultiLine)
# Usage: gen_levact_fixtures.sh <build-dir-with-compile_commands.json> [git-ref]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-0b4f9a6}
FIX=$REPO/crates/hp2-engine/tests/fixtures
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

git -C "$REPO" archive "$REF" | tar -x -C "$WORK"

python3 - "$CC_DB" "$REPO" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, shlex, sys
db, repo, work = sys.argv[1:4]
e = [x for x in json.load(open(db)) if x['file'].endswith('UnLevAct.cpp')][0]
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
cd "$WORK/HarryPotter2/Unreal/Engine/Src"
eval "c++ $FLAGS -O3 -g -c UnLevAct.cpp -o $WORK/UnLevAct.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/levact_fixture_harness/levact_harness.cpp -o $WORK/harness.o"

# Stub only the engine symbols UnLevAct.o needs that the harness does not define. libc/libm and
# C++ runtime symbols stay unresolved so they link to the real libraries.
nm -g --defined-only "$WORK/harness.o" | awk '{print $NF}' | sed 's/^_//' | sort > "$WORK/defined.txt"
nm -u "$WORK/UnLevAct.o" | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt|_ZTV|_ZTI|_ZNS|_Znw|_Znam|_ZdlPv|_ZdaPv|__Z|_?bzero|memcpy|memset|memmove|memcmp|malloc|calloc|realloc|free|abort|qsort|printf|fprintf|puts|strcmp|strlen|sin|cos|sqrt|tan|atan2|fmod|floor|ceil|pow|exp|log|fabs|__error|___|_strlen|_stack_chk|dyld|_dyld)' \
  | sort | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
echo "stubbed symbols:" >&2
cat "$WORK/syms.txt" >&2
python3 -c "
import sys
syms=[l.strip() for l in open(sys.argv[1]) if l.strip()]
print(''.join(f'__attribute__((used)) char stub{i}[64] __asm__(\"_{s}\");' + chr(10) for i,s in enumerate(syms)))
" "$WORK/syms.txt" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnLevAct.o" "$WORK/stubs.o" -o "$WORK/levact_fixture"
"$WORK/levact_fixture" > "$WORK/out.txt"

grep -v '^chk ' "$WORK/out.txt" > "$WORK/rotation.txt"
grep '^chk ' "$WORK/out.txt" | cut -c5- > "$WORK/checks.txt"
mv "$WORK/rotation.txt" "$FIX/levact_rotation.txt"
mv "$WORK/checks.txt" "$FIX/levact_checks.txt"
echo "wrote $(wc -l < "$FIX/levact_rotation.txt") rotation and $(wc -l < "$FIX/levact_checks.txt") check records"
