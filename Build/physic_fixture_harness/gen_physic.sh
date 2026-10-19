#!/bin/sh
# Regenerates the scripted and family physics fixtures under
# crates/hp2-engine/tests/fixtures/.
# Takes the ORIGINAL UnPhysic.cpp and UnMath.h from the pinned git ref (default
# 58a73d4, the pre-port source), applies a WORK-copy FRotator::Vector shim to the
# scripted harness hook, compiles them -O3
# with the real flags from compile_commands.json, and links them with
# physic_harness.cpp, which scripts the engine services the bodies call.
# Output is written to a temp file and renamed into place only on success.
# Usage: gen_physic.sh <build-dir-with-compile_commands.json> [git-ref]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-58a73d4}
OUT=$REPO/crates/hp2-engine/tests/fixtures/physic_scripted.txt
TMP_OUT=$OUT.tmp
WORK=$(mktemp -d)
cleanup()
{
	rm -f "$TMP_OUT"
	[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"
}
trap cleanup EXIT

git -C "$REPO" archive "$REF" | tar -x -C "$WORK"

python3 - "$WORK" <<'PY'
import sys
work = sys.argv[1]

def match_brace(s, i):
    # Index of the '}' closing the '{' at s[i]; skips comments and literals.
    depth, j, n = 0, i, len(s)
    while j < n:
        if s.startswith('//', j):
            j = s.index('\n', j); continue
        if s.startswith('/*', j):
            j = s.index('*/', j) + 2; continue
        if s[j] in '"\'':
            q = s[j]; j += 1
            while s[j] != q:
                j += 2 if s[j] == '\\' else 1
            j += 1; continue
        if s[j] == '{': depth += 1
        elif s[j] == '}':
            depth -= 1
            if depth == 0: return j
        j += 1
    raise SystemExit('unbalanced braces')

def shim(path, head, body):
    s = open(path, newline='').read()
    assert s.count(head) == 1, (path, head, s.count(head))
    k = s.index(head)
    b = s.index('{', k)
    e = match_brace(s, b)
    open(path, 'w', newline='').write(s[:b] + body + s[e + 1:])

shim(work + '/HarryPotter2/Unreal/Core/Inc/UnMath.h',
     'inline FVector FRotator::Vector()',
     '{\n\tFVector hp2_harness_rotation_vector( const FRotator& Rot );\n\treturn hp2_harness_rotation_vector( *this );\n}')

src = open(work + '/HarryPotter2/Unreal/Core/Src/UnMath.cpp', newline='').read()
head = 'FRotator FCoords::OrthoRotation() const'
assert src.count(head) == 1, src.count(head)
k = src.index(head)
b = src.index('{', k)
e = match_brace(src, b)
open(work + '/OrthoRotation.cpp', 'w', newline='').write(
    '#include "CorePrivate.h"\n' + src[k:e + 1] + '\n')
PY

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
eval "c++ $FLAGS -O3 -g -c UnPhysic.cpp -o $WORK/UnPhysic.o"
eval "c++ $FLAGS -O3 -g -c $WORK/OrthoRotation.cpp -o $WORK/OrthoRotation.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/physic_fixture_harness/physic_harness.cpp -o $WORK/harness.o"
# Stub every undefined symbol except libc/libc++ ones and those the harness defines.
nm -g --defined-only "$WORK/harness.o" "$WORK/OrthoRotation.o" | awk 'NF && $NF !~ /:$/ {print $NF}' | sed 's/^_//' | sort > "$WORK/defined.txt"
nm -u "$WORK/UnPhysic.o" "$WORK/OrthoRotation.o" | awk 'NF && $NF !~ /:$/ {print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi|_Z7appSqrtd$|GMath$)' \
  | sort | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
python3 -c "
import sys
syms=[l.strip() for l in open(sys.argv[1]) if l.strip()]
print(''.join(f'__attribute__((used)) char stub{i}[64] __asm__(\"_{s}\");' + chr(10) for i,s in enumerate(syms)))
" "$WORK/syms.txt" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnPhysic.o" "$WORK/OrthoRotation.o" "$WORK/stubs.o" -o "$WORK/physic_harness"
FIXTURES=$REPO/crates/hp2-engine/tests/fixtures
for FAM in scripted motion walk fall swim move dispatch interp smooth; do
	if [ "$FAM" = scripted ]; then OUT=$FIXTURES/physic_scripted.txt; else OUT=$FIXTURES/physic_$FAM.txt; fi
	TMP_OUT=$OUT.tmp
	"$WORK/physic_harness" "$FAM" > "$TMP_OUT"
	mv "$TMP_OUT" "$OUT"
	echo "wrote $FAM: $(wc -l < "$OUT") records"
done
