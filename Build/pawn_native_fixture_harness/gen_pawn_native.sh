#!/bin/sh
# Regenerate the pawn-native input oracle fixture from original UnPawn.cpp at 32dcfda.
# Usage: gen_pawn_native.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=32dcfda
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
python3 - "$WORK/HarryPotter2/Unreal/Engine/Src/UnPawn.cpp" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
s = p.read_text()
anchor = '#include "FConfigCacheIni.h"'
decls = '''
extern UBOOL PawnInputIsA(const APlayerPawn*, const UClass*);
extern UClass* PawnInputGetClass(APlayerPawn*);
extern UClass* PawnInputClassSuper(UClass*);
extern UBOOL PawnInputClassPathEquals(UClass*, const TCHAR*);
extern UBoolProperty* PawnInputFindBool(UClass*, const TCHAR*);
extern UObjectProperty* PawnInputFindObject(UClass*, const TCHAR*);
extern UState* PawnInputStateNode(UObject*);
extern UState* PawnInputStateSuper(UState*);
extern UBOOL PawnInputStateNameEquals(UState*, const TCHAR*);
extern void PawnInputDebugf(EName, const TCHAR*, ...);
extern void PawnInputDebugf(const TCHAR*, ...);
#undef debugf
#define debugf PawnInputDebugf
'''
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + decls, 1)
repls = [
    ('PlayerPawn->IsA(HarryClass)', 'PawnInputIsA(PlayerPawn,HarryClass)', 1),
    ('PlayerPawn->GetClass()', 'PawnInputGetClass(PlayerPawn)', 1),
    ('Candidate->GetSuperClass()', 'PawnInputClassSuper(Candidate)', 1),
    ('appStricmp(Candidate->GetPathName(),TEXT("HGame.Harry"))==0',
     'PawnInputClassPathEquals(Candidate,TEXT("HGame.Harry"))', 1),
    ('FindField<UBoolProperty>(', 'PawnInputFindBool(', 13),
    ('FindField<UObjectProperty>(', 'PawnInputFindObject(', 1),
    ('!Object->GetStateFrame() )', '!PawnInputStateNode(Object) )', 1),
    ('const FName RequiredState( StateName, FNAME_Find );', '(void)0;', 1),
    ('for( UState* State=Object->GetStateFrame()->StateNode; State; State=State->GetSuperState() )',
     'for( UState* State=PawnInputStateNode(Object); State; State=PawnInputStateSuper(State) )', 1),
    ('if( State->GetFName()==RequiredState )', 'if( PawnInputStateNameEquals(State,StateName) )', 1),
]
for old, new, count in repls:
    assert s.count(old) == count, (old, s.count(old))
    s = s.replace(old, new)
p.write_text(s)
PY
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
eval "c++ $FLAGS -O3 -g -c $WORK/HarryPotter2/Unreal/Core/Src/UnMath.cpp -o $WORK/UnMath.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/pawn_native_fixture_harness/pawn_native_input_harness.cpp -o $WORK/harness.o"
{ nm -g --defined-only "$WORK/harness.o"; nm -g --defined-only "$WORK/UnMath.o"; } \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ nm -u "$WORK/UnPawn.o"; nm -u "$WORK/UnMath.o"; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
python3 - "$WORK/syms.txt" > "$WORK/stubs.cpp" <<'PY'
import sys
syms=[line.strip() for line in open(sys.argv[1]) if line.strip()]
forbidden=('FindFunctionChecked','ProcessEvent','IsA','IsProbing','IsValid','eventHearNoise',
           'eventEnemyNotVisible','eventTouch','eventSpecialHandling','appFrand',
           'FastLineCheck','SingleLineCheck','GetCylinderExtent','setPhysics',
           'SafeNormal','Rotation','GMath','appSin','appPow','appIsNan',
           'appAtan2','appRound','appCycles','GetStateFrame','GetSuperState','GetFName')
out=[]
for i,s in enumerate(syms):
    if any(name in s for name in forbidden):
        # Live callbacks used only by non-input natives in this TU: trap if reached.
        out.append(f'extern "C" void trap{i}(void) __asm__("_{s}");\nvoid trap{i}(void) {{ abort(); }}\n')
    else:
        out.append(f'__attribute__((used)) char stub{i}[64] __asm__("_{s}");\n')
print('#include <cstdlib>\n' + ''.join(out))
PY
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnPawn.o" "$WORK/UnMath.o" "$WORK/stubs.o" -o "$WORK/pawn_native_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/pawn_native_input.txt
TMP1=$OUT.tmp.1
TMP2=$OUT.tmp.2
"$WORK/pawn_native_harness" > "$TMP1"
"$WORK/pawn_native_harness" > "$TMP2"
cmp "$TMP1" "$TMP2"
test "$(grep -c '^begin ' "$TMP1")" -eq 129
test "$(grep -c '^finish ' "$TMP1")" -eq 258
test "$(grep -c '^apply ' "$TMP1")" -eq 129
test "$(wc -l < "$TMP1")" -eq 516
mv "$TMP1" "$OUT"
rm "$TMP2"
printf 'wrote %s records\n' "$(wc -l < "$OUT")"
