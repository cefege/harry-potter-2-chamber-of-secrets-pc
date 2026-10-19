#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/physic_fixed_turn.txt from the
# ORIGINAL UnPhysic.cpp (-O3, real flags, from a git snapshot), so the fixture
# is independent of the Rust port. Usage: gen_fixed_turn.sh <build-dir-with-compile_commands.json> [git-ref]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-58a73d4}
WORK=$(mktemp -d)
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
python3 - "$CC_DB" "$REPO" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, shlex, sys
db, repo, work = sys.argv[1:4]
e = [x for x in json.load(open(db)) if x['file'].endswith('UnPhysic.cpp')][0]
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
eval "c++ $FLAGS -O3 -g -c UnPhysic.cpp -o $WORK/UnPhysic.o" 2>/dev/null
eval "c++ $FLAGS -O0 -g -c $REPO/Build/physic_fixture_harness/fixed_turn_harness.cpp -o $WORK/harness.o" 2>/dev/null
# Stub every undefined symbol except libc/libc++ ones; definitions (no extern) so the link resolves.
nm -u "$WORK/UnPhysic.o" | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' > "$WORK/syms.txt"
python3 -c "
import sys
syms=[l.strip() for l in open(sys.argv[1]) if l.strip()]
print(''.join(f'__attribute__((used)) char stub{i}[64] __asm__(\"_{s}\");' + chr(10) for i,s in enumerate(syms)))
" "$WORK/syms.txt" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnPhysic.o" "$WORK/stubs.o" -o "$WORK/fixed_turn"
"$WORK/fixed_turn" > "$REPO/crates/hp2-engine/tests/fixtures/physic_fixed_turn.txt" 2>/dev/null
echo "wrote $(wc -l < "$REPO/crates/hp2-engine/tests/fixtures/physic_fixed_turn.txt") records"
rm -rf "$WORK"
