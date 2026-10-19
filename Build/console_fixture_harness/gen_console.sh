#!/bin/sh
# Regenerate the UConsole oracle fixture from original UnCon.cpp at 4914c7e.
# Usage: gen_console.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=4914c7e
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
python3 - "$WORK/HarryPotter2/Unreal/Engine/Src/UnCon.cpp" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
s = p.read_text()
anchor = '#include "UnRender.h"'
decls = '''
extern void ConsoleHookInitExecution(UConsole*);
extern const TCHAR* ConsoleHookLocalizeAnsi(const ANSICHAR*, const TCHAR*);
extern void ConsoleHookLog(UConsole*, const TCHAR*);
extern void ConsoleHookEventMessage(UConsole*, const TCHAR*, EName);
extern INT ConsoleHookExec(UViewport*, const TCHAR*, FOutputDevice&);
extern void ConsoleHookSave(const TCHAR*, const TCHAR*);
extern UFont* ConsoleHookCreateFont(UViewport*, const TCHAR*, INT);
extern void ConsoleHookTick(UConsole*, FLOAT);
extern void ConsoleHookComputeRenderSize(FSceneNode*);
extern const TCHAR* g_arg_str;
extern INT g_arg_int;
'''
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + '\n' + decls, 1)
repls = [
    ('\tInitExecution();', '\tConsoleHookInitExecution(this);', 1),
    ('\tVERIFY_CLASS_SIZE(UConsole);', '', 1),
    ('Logf(', 'ConsoleHookLog(this, ', 4),
    ('LocalizeGeneral("Engine",', 'ConsoleHookLocalizeAnsi("Engine",', 1),
    ('LocalizeGeneral("Copyright",', 'ConsoleHookLocalizeAnsi("Copyright",', 1),
    ('eventMessage( 0, Data, 0, ThisType );', 'ConsoleHookEventMessage( this, Data, ThisType );', 1),
    ('\tP_GET_STR(S);\n\tP_FINISH;\n\n\t*(DWORD*)Result = Viewport->Exec( *S, *this );',
     '\tconst TCHAR* S = g_arg_str;\n\n\t*(DWORD*)Result = ConsoleHookExec( Viewport, S, *this );', 1),
    ('\tP_GET_STR(S);\n\tP_FINISH;\n\tappSaveStringToFile( S, TEXT("fps.txt"), GFileManager );',
     '\tconst TCHAR* S = g_arg_str;\n\tConsoleHookSave( S, TEXT("fps.txt") );', 1),
    ('\tP_GET_STR(FontName);\n\tP_GET_INT(Height);\n\tP_FINISH;\n\n\t*(UFont**)Result = Viewport->CreateNativeFont( *FontName, Height );',
     '\tconst TCHAR* FontName = g_arg_str;\n\tINT Height = g_arg_int;\n\n\t*(UFont**)Result = ConsoleHookCreateFont( Viewport, FontName, Height );', 1),
    ('eventTick( Viewport->CurrentTime - Viewport->LastUpdateTime );',
     'ConsoleHookTick( this, Viewport->CurrentTime - Viewport->LastUpdateTime );', 1),
    ('Frame->ComputeRenderSize();', 'ConsoleHookComputeRenderSize( Frame );', 1),
]
for old, new, count in repls:
    assert s.count(old) == count, (old, s.count(old))
    s = s.replace(old, new)
p.write_text(s)
PY
FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Engine/Src/UnCon.cpp "$REPO" "$WORK")
cd "$WORK/HarryPotter2/Unreal/Engine/Src"
eval "c++ $FLAGS -O3 -g -c UnCon.cpp -o $WORK/UnCon.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/console_fixture_harness/console_harness.cpp -o $WORK/harness.o"
{ nm -g --defined-only "$WORK/harness.o"; } \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ nm -u "$WORK/UnCon.o"; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock|wcs[a-z]+)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
# Stubs: functions abort with STUB; data is zero-filled 1 MiB, 16-aligned (bss-equivalent). Any stub hit fails the run.
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" "$WORK/UnCon.o" "$WORK/harness.o" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnCon.o" "$WORK/stubs.o" -o "$WORK/console_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/console_uncon.txt
run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/console_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/console_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
R="$WORK/run1.txt"
test "$(grep -c '^tick ' "$R")" -eq 3024
test "$(grep -c '^compute ' "$R")" -eq 3024
test "$(grep -c '^pre ' "$R")" -eq 3024
test "$(grep -c '^post ' "$R")" -eq 3024
test "$(grep -c '^event ' "$R")" -eq 3
test "$(grep -c '^exec ' "$R")" -eq 2
test "$(grep -c '^font ' "$R")" -eq 2
test "$(wc -l < "$R")" -eq 12116
cp "$R" "$OUT"
printf 'wrote %s records\n' "$(wc -l < "$OUT")"
