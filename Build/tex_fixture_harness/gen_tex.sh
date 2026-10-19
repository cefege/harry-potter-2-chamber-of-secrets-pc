#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/tex_fixtures.txt from the ORIGINAL
# UnTex.cpp (-O3 -g, real flags, from a git snapshot), so the fixture is
# independent of the Rust port. Usage: gen_tex.sh <build-dir-with-compile_commands.json> [git-ref]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-b5404e3}
WORK=$(mktemp -d)
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
python3 - "$CC_DB" "$REPO" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, shlex, sys
db, repo, work = sys.argv[1:4]
e = [x for x in json.load(open(db)) if x['file'].endswith('UnTex.cpp')][0]
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
eval "c++ $FLAGS -O3 -g -c UnTex.cpp -o $WORK/UnTex.o" 2>/dev/null
eval "c++ $FLAGS -O0 -g -c $REPO/Build/tex_fixture_harness/tex_fixture_harness.cpp -o $WORK/harness.o"
# Symbols defined by the harness are excluded; every other undefined symbol gets a data stub.
nm -u "$WORK/UnTex.o" | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi|GMalloc$)' \
  | grep -v -E '^_(Z7appSqrtd|Z10appMemmovePvPKvi|Z9appMemcpyPvPKvi|Z9appMemsetPvii|Z13appFailAssertPKcS0_i)$' > "$WORK/syms.txt"
python3 -c "
import sys
syms=[l.strip() for l in open(sys.argv[1]) if l.strip()]
print(''.join(f'__attribute__((used)) char stub{i}[64] __asm__(\"_{s}\");' + chr(10) for i,s in enumerate(syms)))
" "$WORK/syms.txt" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnTex.o" "$WORK/stubs.o" -o "$WORK/tex_fixture"
mkdir -p "$REPO/crates/hp2-engine/tests/fixtures"
"$WORK/tex_fixture" > "$REPO/crates/hp2-engine/tests/fixtures/tex_fixtures.txt"
echo "wrote $(wc -l < "$REPO/crates/hp2-engine/tests/fixtures/tex_fixtures.txt") records"
rm -rf "$WORK"
