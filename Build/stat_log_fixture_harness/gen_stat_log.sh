#!/bin/sh
# Regenerate the AStatLog oracle fixture from the original AStatLog.cpp and the real Core
# MD5 + string/memory helper regions at 69c3263.
# Usage: gen_stat_log.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=69c3263
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" HarryPotter2/Unreal | tar -x -C "$WORK"
SRC=$WORK/HarryPotter2/Unreal/Engine/Src
CORE=$WORK/HarryPotter2/Unreal/Core/Src
python3 - "$SRC/AStatLog.cpp" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
s = p.read_text()
anchor = '#include "EnginePrivate.h"'
decls = '''
extern const TCHAR* g_arg_str;
extern UObject* g_arg_obj;
extern FString* g_arg_strref;
'''
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + '\n' + decls, 1)
def sub(pat, rep, n, flags=0):
    global s
    s, k = re.subn(pat, rep, s, flags=flags)
    assert k == n, (pat, k)
sub(r'P_GET_OBJECT\((\w+), (\w+)\);', r'\1* \2 = (\1*)g_arg_obj;', 3)
sub(r'P_GET_STR_REF\((\w+)\);', r'FString* \1 = g_arg_strref;', 2)
sub(r'P_GET_STR\((\w+)\);', r'FString \1 = FString(g_arg_str);', 3)
sub(r'^[ \t]*P_FINISH;\n', '', 16, re.M)
sub(r'IMPLEMENT_CLASS\(AMutator\);', '', 1)
assert 'P_GET_' not in s and 'P_FINISH' not in s
s += '''
// Harness entry: the original UCheckSumCommandlet::Main on a zeroed object (qualified call, no vtable).
INT HarnessChecksumCommandletMain( const TCHAR* Parms )
{
	static unsigned char buf[sizeof(UCheckSumCommandlet)] __attribute__((aligned(16)));
	for( size_t i = 0; i < sizeof buf; ++i ) buf[i] = 0;
	return ((UCheckSumCommandlet*)buf)->UCheckSumCommandlet::Main( Parms );
}
'''
p.write_text(s)
PY
python3 - "$CORE/UnMisc.cpp" "$CORE/UnAnsi.cpp" "$CORE/core_extract.cpp" <<'PY'
import pathlib, sys
misc = pathlib.Path(sys.argv[1]).read_text().splitlines(keepends=True)
# MD5 section: from the "MD5 functions" banner through appMD5Decode.
assert misc[507].lstrip().startswith('/*---'), misc[507]
assert 'PADDING[64]' in misc[532], misc[532]
assert misc[770].rstrip() == '}', misc[770]
ansi = pathlib.Path(sys.argv[2]).read_text().splitlines(keepends=True)
# String/memory helpers: appMemmove .. appStrcpy.
assert ansi[2034].startswith('CORE_API void* appMemmove'), ansi[2034]
assert ansi[2195].rstrip() == '}', ansi[2195]
out = ('#include "CorePrivate.h"\n'
       + ''.join(misc[507:771]) + '\n'
       + ''.join(ansi[2034:2196]))
pathlib.Path(sys.argv[3]).write_text(out)
PY
FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Engine/Src/AStatLog.cpp "$REPO" "$WORK")
cd "$SRC"
eval "c++ $FLAGS -O3 -g -c AStatLog.cpp -o $WORK/AStatLog.o"
cd "$CORE"
eval "c++ $FLAGS -O3 -g -c core_extract.cpp -o $WORK/core.o"
cd "$SRC"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/stat_log_fixture_harness/stat_log_harness.cpp -o $WORK/harness.o"
for o in harness core AStatLog; do nm -g --defined-only "$WORK/$o.o"; done \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
for o in harness core AStatLog; do nm -u "$WORK/$o.o"; done \
  | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|_setjmp|longjmp|_longjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?swprintf|swprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock|wcs[a-z]+|vfwprintf)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
# Stubs: functions abort with STUB; data is zero-filled char[64]. Any stub hit fails the run.
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" "$WORK/AStatLog.o" "$WORK/core.o" "$WORK/harness.o" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/AStatLog.o" "$WORK/core.o" "$WORK/stubs.o" -o "$WORK/stat_log_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/stat_log_cases.txt
run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/stat_log_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/stat_log_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
R="$WORK/run1.txt"
if grep -q 'UNEXPECTED' "$R"; then cat "$R" >&2; echo "unexpected boundary call" >&2; exit 1; fi
test "$(grep -c '^case=' "$R")" -eq 31
cp "$R" "$OUT"
printf 'wrote %s records\n' "$(wc -l < "$OUT")"
