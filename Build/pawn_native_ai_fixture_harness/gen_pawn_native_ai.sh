#!/bin/sh
# Regenerate the pawn-native ai oracle fixture from original UnPawn.cpp at 32dcfda.
# Usage: gen_pawn_native_ai.sh <build-dir-with-compile_commands.json>
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
extern FVector PawnAiRotationVector(const FRotator&);
extern INT PawnAiStateLatentAction(AActor*);
extern void PawnAiSetStateLatentAction(AActor*, INT);
extern INT PawnAiLevelNetMode(ULevel*);
extern APawn* PawnAiLevelPawnList(ULevel*);
extern void PawnAiSetLevelPawnList(ULevel*, APawn*);
extern INT PawnAiLevelTeamGame(ULevel*);
extern INT PawnAiLevelActorCount(ULevel*);
extern AActor* PawnAiLevelActor(ULevel*, INT);
extern INT PawnAiViewportCount(ULevel*);
extern APlayerPawn* PawnAiViewportActor(ULevel*, INT);
extern UBOOL PawnAiActorNetTemporary(AActor*);
extern UBOOL PawnAiActorProjTarget(AActor*);
extern UBOOL PawnAiActorIsPawn(AActor*);
extern INT PawnAiPawnHealth(APawn*);
extern UBOOL PawnAiPawnBehindView(APlayerPawn*);
extern INT PawnAiReplicationTeam(APlayerReplicationInfo*);
extern UBOOL PawnAiLineOfSightTo(APawn*, AActor*);
extern void PawnAiSingleLineCheck(ULevel*, FCheckResult&, AActor*, const FVector&, const FVector&, DWORD, const FVector& = FVector(0.f, 0.f, 0.f));
extern void PawnAiEventLongFall(APawn*);
extern FLOAT g_ai_best_aim, g_ai_best_dist, g_ai_delta_time;
extern FVector g_ai_fire_dir, g_ai_proj_start;
'''
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + decls, 1)

def scoped(text, sig, repls):
    assert text.count(sig) == 1, (sig, text.count(sig))
    start = text.index(sig)
    end = text.index('\n}\n', start)
    body = text[start:end]
    for old, new, count in repls:
        assert body.count(old) == count, (sig, old, body.count(old))
        body = body.replace(old, new)
    return text[:start] + body + text[end:]

P_GET = [
    ('P_GET_FLOAT_REF(bestAim);', 'FLOAT* bestAim = &g_ai_best_aim;', 1),
    ('P_GET_FLOAT_REF(bestDist);', 'FLOAT* bestDist = &g_ai_best_dist;', 1),
    ('P_GET_VECTOR(FireDir);', 'FVector FireDir = g_ai_fire_dir;', 1),
    ('P_GET_VECTOR(projStart);', 'FVector projStart = g_ai_proj_start;', 1),
]
LOS = ('LineOfSightTo(next)', 'PawnAiLineOfSightTo(this, next)', 1)
scope = [
    ('void APawn::execStopWaiting( FFrame& Stack, RESULT_DECL )', [
        ('if( GetStateFrame()->LatentAction == EPOLL_Sleep )',
         'if( PawnAiStateLatentAction(this) == EPOLL_Sleep )', 1),
    ]),
    ('void AActor::execPlayerCanSeeMe( FFrame& Stack, RESULT_DECL )', [
        ('int NetMode = GetLevel()->GetLevelInfo()->NetMode;',
         'int NetMode = PawnAiLevelNetMode(GetLevel());', 1),
        ('bNetTemporary ||', 'PawnAiActorNetTemporary(this) ||', 1),
        ('i<GetLevel()->Engine->Client->Viewports.Num()',
         'i<PawnAiViewportCount(GetLevel())', 1),
        ('TestCanSeeMe( GetLevel()->Engine->Client->Viewports(i)->Actor )',
         'TestCanSeeMe( PawnAiViewportActor(GetLevel(), i) )', 1),
        ('for ( APawn *next=GetLevel()->GetLevelInfo()->PawnList;',
         'for ( APawn *next=PawnAiLevelPawnList(GetLevel());', 1),
    ]),
    ('int AActor::TestCanSeeMe( APlayerPawn *Viewer )', [
        ('Viewer->bBehindView', 'PawnAiPawnBehindView(Viewer)', 1),
        ('Viewer->ViewRotation.Vector()', 'PawnAiRotationVector(Viewer->ViewRotation)', 1),
        ('Viewer->LineOfSightTo(this)', 'PawnAiLineOfSightTo(Viewer, this)', 1),
    ]),
    ('void APawn::execPickTarget( FFrame& Stack, RESULT_DECL )', P_GET + [
        ('for ( APawn *next=GetLevel()->GetLevelInfo()->PawnList;',
         'for ( APawn *next=PawnAiLevelPawnList(GetLevel());', 1),
        ('(next->Health > 0) && next->bProjTarget',
         '(PawnAiPawnHealth(next) > 0) && PawnAiActorProjTarget(next)', 1),
        ('!GetLevel()->GetLevelInfo()->Game->bTeamGame',
         '!PawnAiLevelTeamGame(GetLevel())', 1),
        ('(PlayerReplicationInfo->Team != next->PlayerReplicationInfo->Team)',
         '(PawnAiReplicationTeam(PlayerReplicationInfo) != PawnAiReplicationTeam(next->PlayerReplicationInfo))', 1),
        LOS,
    ]),
    ('void APawn::execPickAnyTarget( FFrame& Stack, RESULT_DECL )', P_GET + [
        ('iActor<GetLevel()->Actors.Num()', 'iActor<PawnAiLevelActorCount(GetLevel())', 1),
        ('if( GetLevel()->Actors(iActor) )',
         'if( AActor* PawnAiPicked = PawnAiLevelActor(GetLevel(), iActor) )', 1),
        ('AActor* next = GetLevel()->Actors(iActor);', 'AActor* next = PawnAiPicked;', 1),
        ('next->bProjTarget && !next->IsA(APawn::StaticClass())',
         'PawnAiActorProjTarget(next) && !PawnAiActorIsPawn(next)', 1),
        LOS,
    ]),
    ('void APawn::execAddPawn( FFrame& Stack, RESULT_DECL )', [
        ('nextPawn = GetLevel()->GetLevelInfo()->PawnList;',
         'nextPawn = PawnAiLevelPawnList(GetLevel());', 1),
        ('GetLevel()->GetLevelInfo()->PawnList = this;',
         'PawnAiSetLevelPawnList(GetLevel(), this);', 1),
    ]),
    ('void APawn::execRemovePawn( FFrame& Stack, RESULT_DECL )', [
        ('APawn *next = GetLevel()->GetLevelInfo()->PawnList;',
         'APawn *next = PawnAiLevelPawnList(GetLevel());', 1),
        ('GetLevel()->GetLevelInfo()->PawnList = next->nextPawn;',
         'PawnAiSetLevelPawnList(GetLevel(), next->nextPawn);', 1),
    ]),
    ('void APawn::execWaitForLanding( FFrame& Stack, RESULT_DECL )', [
        ('GetStateFrame()->LatentAction = AI_PollWaitForLanding;',
         'PawnAiSetStateLatentAction(this, AI_PollWaitForLanding);', 1),
    ]),
    ('void APawn::execPollWaitForLanding( FFrame& Stack, RESULT_DECL )', [
        ('GetStateFrame()->LatentAction = 0;', 'PawnAiSetStateLatentAction(this, 0);', 1),
        ('eventLongFall();', 'PawnAiEventLongFall(this);', 1),
    ]),
    ('void APawn::execFindStairRotation( FFrame& Stack, RESULT_DECL )', [
        ('P_GET_FLOAT(deltaTime);', 'FLOAT deltaTime = g_ai_delta_time;', 1),
        ('LookRot.Vector()', 'PawnAiRotationVector(LookRot)', 1),
        ('GetLevel()->SingleLineCheck(', 'PawnAiSingleLineCheck(GetLevel(), ', 5),
    ]),
]
for sig, repls in scope:
    s = scoped(s, sig, repls)
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
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/pawn_native_ai_fixture_harness/pawn_native_ai_harness.cpp -o $WORK/harness.o"
{ nm -g --defined-only "$WORK/harness.o"; nm -g --defined-only "$WORK/UnMath.o"; } \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ nm -u "$WORK/UnPawn.o"; nm -u "$WORK/UnMath.o"; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
if grep -q 'PawnAi' "$WORK/syms.txt"; then
  echo "undefined PawnAi hook symbol(s) (harness definition does not match UnPawn.cpp declaration):" >&2
  grep 'PawnAi' "$WORK/syms.txt" >&2
  exit 1
fi
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
        # Live callbacks used only by non-ai natives in this TU: trap if reached.
        out.append(f'extern "C" void trap{i}(void) __asm__("_{s}");\nvoid trap{i}(void) {{ abort(); }}\n')
    else:
        out.append(f'__attribute__((used)) char stub{i}[64] __asm__("_{s}");\n')
print('#include <cstdlib>\n' + ''.join(out))
PY
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnPawn.o" "$WORK/UnMath.o" "$WORK/stubs.o" -o "$WORK/pawn_native_ai_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/pawn_native_ai.txt
TMP1=$OUT.tmp.1
TMP2=$OUT.tmp.2
"$WORK/pawn_native_ai_harness" > "$TMP1"
"$WORK/pawn_native_ai_harness" > "$TMP2"
cmp "$TMP1" "$TMP2"
test "$(grep -c '^player_can_see_me ' "$TMP1")" -eq 24
test "$(grep -c '^test_can_see_me ' "$TMP1")" -eq 32
test "$(grep -c '^pick_target ' "$TMP1")" -eq 48
test "$(grep -c '^pick_any_target ' "$TMP1")" -eq 48
test "$(grep -c '^add_pawn ' "$TMP1")" -eq 12
test "$(grep -c '^remove_pawn ' "$TMP1")" -eq 16
test "$(grep -c '^stop_waiting ' "$TMP1")" -eq 12
test "$(grep -c '^wait_for_landing ' "$TMP1")" -eq 12
test "$(grep -c '^poll_wait_for_landing ' "$TMP1")" -eq 16
test "$(grep -c '^find_stair_rotation ' "$TMP1")" -eq 64
test "$(wc -l < "$TMP1")" -eq 284
mv "$TMP1" "$OUT"
rm "$TMP2"
printf 'wrote %s records\n' "$(wc -l < "$OUT")"
