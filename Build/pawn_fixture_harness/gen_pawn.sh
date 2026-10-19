#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/pawn_moves.txt from the ORIGINAL
# UnPawn.cpp (-O3, real flags, from a git snapshot of the oracle ref) linked
# against pawn_harness.cpp, which scripts ULevel::MoveActor/FarMoveActor and
# APawn::findWaterLine. Output is byte-identical across runs.
# Usage: gen_pawn.sh <build-dir-with-compile_commands.json> [git-ref]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-32dcfda}
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
python3 - "$CC_DB" "$REPO" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, shlex, sys
db, repo, work = sys.argv[1:4]
e = [x for x in json.load(open(db)) if x['file'].endswith('UnPawn.cpp')][0]
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
eval "c++ $FLAGS -O3 -g -c UnPawn.cpp -o $WORK/UnPawn.o"
eval "c++ $FLAGS -O3 -g -c $REPO/Build/pawn_fixture_harness/pawn_harness.cpp -o $WORK/harness.o"
# Stub every undefined symbol except libc/libc++ ones and those the harness defines.
nm -g --defined-only "$WORK/harness.o" | awk '{print $NF}' | sed 's/^_//' | sort > "$WORK/defined.txt"
nm -u "$WORK/UnPawn.o" | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock)$' \
  | sort | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
python3 -c "
import sys
syms=[l.strip() for l in open(sys.argv[1]) if l.strip()]
print(''.join(f'__attribute__((used)) char stub{i}[64] __asm__(\"_{s}\");' + chr(10) for i,s in enumerate(syms)))
" "$WORK/syms.txt" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnPawn.o" "$WORK/stubs.o" -o "$WORK/pawn_harness"
mkdir -p "$REPO/crates/hp2-engine/tests/fixtures"
"$WORK/pawn_harness" > "$REPO/crates/hp2-engine/tests/fixtures/pawn_moves.txt"
echo "wrote $(wc -l < "$REPO/crates/hp2-engine/tests/fixtures/pawn_moves.txt") records"
