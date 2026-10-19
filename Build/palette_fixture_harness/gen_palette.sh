#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/palette_quantize.txt from the ORIGINAL
# palette.cpp at the pinned ref (git archive, not the working tree).
#
# Usage: gen_palette.sh <build-dir-with-compile_commands.json> [git-ref]
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
e = [x for x in json.load(open(db)) if x['file'].endswith('palette.cpp') and 'Engine/Src' in x['file']][0]
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
eval "c++ $FLAGS -O3 -g -c palette.cpp -o $WORK/palette.o"
eval "c++ $FLAGS -O3 -g -c $REPO/Build/palette_fixture_harness/palette_harness.cpp -o $WORK/harness.o"

# Stub only the engine symbols palette.o references that the harness does not define
# (debugf's GLog/GError, appMemset and friends). They are never reached by fixture inputs.
nm -g --defined-only "$WORK/harness.o" | awk '{print $NF}' | sed 's/^_//' | sort > "$WORK/defined.txt"
nm -u "$WORK/palette.o" | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__chkstk|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt|_ZTV|_ZTI|_ZNS|_Znw|_Znam|_ZdlPv|_ZdaPv|__Z|_?bzero|memcpy|memset|memmove|memcmp|malloc|calloc|realloc|free|abort|qsort|printf|fprintf|puts|strcmp|strlen|sin|cos|sqrt|tan|atan2|fmod|floor|ceil|pow|exp|log|fabs|__error|___|_strlen|_stack_chk|dyld|_dyld|stdout|stderr|fflush|fputs|sprintf|snprintf|vsnprintf|vfprintf|vprintf)' \
  | sort | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
echo "stubbed symbols:" >&2
cat "$WORK/syms.txt" >&2
python3 -c "
import sys
syms=[l.strip() for l in open(sys.argv[1]) if l.strip()]
print(''.join(f'__attribute__((used)) char stub{i}[64] __asm__(\"_{s}\");' + chr(10) for i,s in enumerate(syms)))
" "$WORK/syms.txt" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/palette.o" "$WORK/stubs.o" -o "$WORK/palette_fixture"
"$WORK/palette_fixture" > "$WORK/palette_quantize.txt"
mkdir -p "$FIX"
mv "$WORK/palette_quantize.txt" "$FIX/palette_quantize.txt"
echo "wrote $(wc -l < "$FIX/palette_quantize.txt") palette records"
