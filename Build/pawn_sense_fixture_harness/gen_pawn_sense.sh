#!/bin/sh
# Regenerate the pawn-sense oracle fixture from original UnPawn.cpp at 32dcfda.
# Usage: gen_pawn_sense.sh <build-dir-with-compile_commands.json>
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
extern INT PawnSenseActorReachableHook(APawn*, AActor*, INT);
extern INT PawnSenseFindPathTowardHook(APawn*, AActor*, INT, AActor*&, INT);
extern FVector PawnSenseSafeNormalHook(FVector);
extern FRotator PawnSenseVectorRotationHook(FVector);
extern UObject* PawnSenseGetZoneDefaultObjectHook(UClass*);
'''
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + decls, 1)
assert s.count('Region.Zone->GetClass()->GetDefaultObject()') == 1
s = s.replace('Region.Zone->GetClass()->GetDefaultObject()',
              'PawnSenseGetZoneDefaultObjectHook(Region.Zone->GetClass())', 1)
repls = {
    'if ( actorReachable(bestPath) )':
        'if ( PawnSenseActorReachableHook(this,bestPath,0) )',
    '(bestPath != realPath) && actorReachable(ReturnedActor)':
        '(bestPath != realPath) && PawnSenseActorReachableHook(this,ReturnedActor,0)',
    'int success = findPathToward(bestPath, 0, newPath, 1);':
        'int success = PawnSenseFindPathTowardHook(this,bestPath,0,newPath,1);',
    'Acceleration = Direction.SafeNormal();':
        'Acceleration = PawnSenseSafeNormalHook(Direction);',
    'DesiredRotation = Direction.Rotation();':
        'DesiredRotation = PawnSenseVectorRotationHook(Direction);',
    'FVector SightDir = (Other->Location - Location).SafeNormal();':
        'FVector SightDir = PawnSenseSafeNormalHook(Other->Location - Location);',
    'FVector ShortDir = Dir.SafeNormal();':
        'FVector ShortDir = PawnSenseSafeNormalHook(Dir);',
    'IMPLEMENT_CLASS(APawn);':
        '',
    'IMPLEMENT_CLASS(APlayerPawn);':
        '',
}
for old, new in repls.items():
    assert s.count(old) == 1, (old, s.count(old))
    s = s.replace(old, new, 1)
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
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/pawn_sense_fixture_harness/pawn_sense_harness.cpp -o $WORK/harness.o"
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
           'appAtan2','appRound','appCycles')
bad=[s for s in syms if any(name in s for name in forbidden)]
if bad:
    raise SystemExit('refusing generic storage stubs for live callback/class/event symbols: '+', '.join(bad))
print(''.join(f'__attribute__((used)) char stub{i}[64] __asm__("_{s}");\n' for i,s in enumerate(syms)))
PY
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnPawn.o" "$WORK/UnMath.o" "$WORK/stubs.o" -o "$WORK/pawn_sense_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/pawn_sense.txt
TMP1=$OUT.tmp.1
TMP2=$OUT.tmp.2
"$WORK/pawn_sense_harness" > "$TMP1"
"$WORK/pawn_sense_harness" > "$TMP2"
cmp "$TMP1" "$TMP2"
test "$(wc -l < "$TMP1")" -eq 102
mv "$TMP1" "$OUT"
rm "$TMP2"
printf 'wrote %s records\n' "$(wc -l < "$OUT")"
