#!/bin/sh
# Regenerate the pawn move native fixture from the original UnPawn.cpp at 32dcfda.
# Usage: sh gen_pawn_native_move.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$(cd "$1" && pwd)/compile_commands.json
REF=32dcfda
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"

# Rewrite the 18 move natives (function spans only) so their engine leaves call
# PawnMove* hooks and their parameter reads pop the scenario queue.
python3 - "$WORK/HarryPotter2/Unreal/Engine/Src/UnPawn.cpp" <<'PY'
import pathlib, re, sys

p = pathlib.Path(sys.argv[1])
s = p.read_bytes().decode("utf-8", "surrogateescape").replace("\r\n", "\n")
anchor = '#include "FConfigCacheIni.h"'
decls = '''
extern INT PawnMoveIsA(const AActor*, INT);
extern UBOOL PawnMoveIsProbing(const AActor*, INT);
extern INT PawnMoveNetMode(const AActor*);
extern INT PawnMoveReachSpecCount(const AActor*);
extern ANavigationPoint* PawnMoveLevelNavList(const AActor*);
extern APawn* PawnMoveLevelPawnList(const AActor*);
extern INT PawnMoveActorReachable(const AActor*, const AActor*);
extern INT PawnMoveCanMoveTo(const AActor*, const AActor*, const AActor*);
extern INT PawnMoveFindPathTo(const AActor*, const FVector*, INT, INT, AActor**);
extern INT PawnMoveFindPathToward(const AActor*, const AActor*, INT, INT, AActor**);
extern INT PawnMoveFindRandomDest(const AActor*, AActor**);
extern void PawnMoveClearPaths(const AActor*);
extern void PawnMoveHandleSpecial(const AActor*, AActor**);
extern void PawnMoveSetMoveTimer(const AActor*, FLOAT);
extern INT PawnMoveRotateToward(const AActor*, const FVector*);
extern INT PawnMoveMoveToward(const AActor*, const FVector*);
extern void PawnMoveSetLatent(const AActor*, INT);
extern void PawnMoveEventAlterDestination(APawn*);
extern void PawnMoveEventSeePlayer(const AActor*, const AActor*);
extern DWORD PawnMoveLineOfSightTo(const AActor*, const AActor*, INT);
extern void PawnMoveCheckNoiseHearing(const AActor*, FLOAT);
extern FVector PawnMoveRotationVector(const AActor*);
extern FVector PawnMovePopVector();
extern AActor* PawnMovePopActor();
extern FLOAT PawnMovePopFloat();
extern FLOAT PawnMovePopFloatOptx(FLOAT);
extern INT PawnMovePopIntOptx(INT);
extern UBOOL PawnMovePopUboolOptx(UBOOL);
'''
assert s.count(anchor) == 1, s.count(anchor)
s = s.replace(anchor, anchor + decls, 1)

headers = [
    "void APawn::execactorReachable(",
    "void APawn::execFindPathTo(",
    "void APawn::execFindPathToward(",
    "void APawn::execFindRandomDest(",
    "void AActor::execMakeNoise(",
    "void APawn::execMoveTo(",
    "void APawn::execPollMoveTo(",
    "void APawn::execMoveToward(",
    "void APawn::execPollMoveToward(",
    "void APawn::execStrafeTo(",
    "void APawn::execPollStrafeTo(",
    "void APawn::execStrafeFacing(",
    "void APawn::execPollStrafeFacing(",
    "void APawn::execTurnToward(",
    "void APawn::execPollTurnToward(",
    "void APawn::execTurnTo(",
    "void APawn::execPollTurnTo(",
    "void APawn::ShowSelf(",
]
spans = []
for h in headers:
    assert s.count(h) == 1, (h, s.count(h))
    a = s.index(h)
    b = s.index("\n}\n", a) + 3
    spans.append((a, b))
for (a1, b1), (a2, b2) in zip(spans, spans[1:]):
    assert b1 <= a2, (a1, b1, a2)
segs = [s[a:b] for a, b in spans]

def sub(pat, rep, want):
    total = 0
    for i in range(len(segs)):
        segs[i], n = re.subn(pat, rep, segs[i])
        total += n
    if total != want:
        raise SystemExit("%s: %d != %d" % (pat, total, want))

# Queue-backed parameter reads and the finish marker.
sub(r"P_GET_VECTOR\((\w+)\)[ \t]*;?", r"FVector \1 = PawnMovePopVector();", sum(
    len(re.findall(r"P_GET_VECTOR\(\w+\)[ \t]*;?", t)) for t in segs))
sub(r"P_GET_ACTOR\((\w+)\);", r"AActor* \1 = PawnMovePopActor();", sum(
    len(re.findall(r"P_GET_ACTOR\(\w+\);", t)) for t in segs))
sub(r"P_GET_FLOAT\((\w+)\);", r"FLOAT \1 = PawnMovePopFloat();", sum(
    len(re.findall(r"P_GET_FLOAT\(\w+\);", t)) for t in segs))
sub(r"P_GET_FLOAT_OPTX\((\w+),\s*([^)]+)\);", r"FLOAT \1 = PawnMovePopFloatOptx(\2);", sum(
    len(re.findall(r"P_GET_FLOAT_OPTX\(\w+,\s*[^)]+\);", t)) for t in segs))
sub(r"P_GET_INT_OPTX\((\w+),\s*([^)]+)\);", r"INT \1 = PawnMovePopIntOptx(\2);", sum(
    len(re.findall(r"P_GET_INT_OPTX\(\w+,\s*[^)]+\);", t)) for t in segs))
sub(r"P_GET_UBOOL_OPTX\((\w+),\s*([^)]+)\);", r"UBOOL \1 = PawnMovePopUboolOptx(\2);", sum(
    len(re.findall(r"P_GET_UBOOL_OPTX\(\w+,\s*[^)]+\);", t)) for t in segs))
sub(r"P_FINISH;", ";", sum(t.count("P_FINISH;") for t in segs))

# Engine leaves: class tests, property tests, path searches, steering, latent state.
sub(r"(\w+)->IsA\(AInventory::StaticClass\(\)\)", r"PawnMoveIsA(\1, 1)", 1)
sub(r"(\w+)->IsA\(ANavigationPoint::StaticClass\(\)\)", r"PawnMoveIsA(\1, 2)", 2)
sub(r"(\w+)->IsA\(APawn::StaticClass\(\)\)", r"PawnMoveIsA(\1, 0)", 3)
sub(r"(\w+)->IsProbing\(NAME_SpecialHandling\)", r"PawnMoveIsProbing(\1, 0)", 2)
sub(r"(\w+)->IsProbing\(NAME_SeePlayer\)", r"PawnMoveIsProbing(\1, 1)", 1)
sub(r"GetLevel\(\)->ReachSpecs\.Num\(\)", r"PawnMoveReachSpecCount(this)", 1)
sub(r"GetLevel\(\)->GetLevelInfo\(\)->NavigationPointList", r"PawnMoveLevelNavList(this)", 1)
sub(r"GetLevel\(\)->GetLevelInfo\(\)->NetMode", r"PawnMoveNetMode(this)", 1)
sub(r"GetLevel\(\)->GetLevelInfo\(\)->PawnList", r"PawnMoveLevelPawnList(this)", 1)
sub(r"Rotation\.Vector\(\)", r"PawnMoveRotationVector(this)", 4)
sub(r"GetStateFrame\(\)->LatentAction = ([^;]+);", r"PawnMoveSetLatent(this, \1);", 15)
sub(r"(?<![A-Za-z_])CheckNoiseHearing\((\w+)\);", r"PawnMoveCheckNoiseHearing(this, \1);", 1)
sub(r"(?<![A-Za-z_])actorReachable\((\w+)\)", r"PawnMoveActorReachable(this, \1)", 1)
sub(r"(?<![A-Za-z_])CanMoveTo\(([^,()]+),\s*([^()]+)\)", r"PawnMoveCanMoveTo(this, \1, \2)", 2)
sub(r"(?<![A-Za-z_])findPathTo\(\s*(\w+)\s*,\s*(\w+)\s*,\s*(\w+)\s*,\s*(\w+)\s*\)",
    r"PawnMoveFindPathTo(this, &\1, \2, \4, &\3)", 1)
sub(r"(?<![A-Za-z_])findPathToward\(\s*(\w+)\s*,\s*(\w+)\s*,\s*(\w+)\s*,\s*(\w+)\s*\)",
    r"PawnMoveFindPathToward(this, \1, \2, \4, &\3)", 1)
sub(r"(?<![A-Za-z_])findRandomDest\((\w+)\)", r"PawnMoveFindRandomDest(this, &\1)", 1)
sub(r"(?<![A-Za-z_])clearPaths\(\);", r"PawnMoveClearPaths(this);", 1)
sub(r"(?<![A-Za-z_])HandleSpecial\((\w+)\);", r"PawnMoveHandleSpecial(this, &\1);", 2)
sub(r"(?<![A-Za-z_])setMoveTimer\((.*?)\);", r"PawnMoveSetMoveTimer(this, \1);", 4)
sub(r"(?<![A-Za-z_])rotateToward\(\s*(\w+)\s*\)", r"PawnMoveRotateToward(this, &\1)", 12)
sub(r"(?<![A-Za-z_])moveToward\((\w+)\)", r"PawnMoveMoveToward(this, &\1)", 8)
sub(r"(?<![A-Za-z_])eventAlterDestination\(\);", r"PawnMoveEventAlterDestination(this);", 2)
sub(r"(\w+)->eventSeePlayer\(this\)", r"PawnMoveEventSeePlayer(\1, this)", 1)
sub(r"(\w+)->LineOfSightTo\(this, 1\)", r"PawnMoveLineOfSightTo(\1, this, 1)", 1)

leftover = re.compile(
    r"(?<![A-Za-z_])(rotateToward|moveToward|setMoveTimer|findPathTo|findPathToward|findRandomDest"
    r"|clearPaths|HandleSpecial|CanMoveTo|actorReachable|LineOfSightTo|CheckNoiseHearing"
    r"|eventAlterDestination|eventSeePlayer)\("
    r"|->IsA\(|(?<![A-Za-z_])IsProbing\(|Rotation\.Vector\(\)|\bReachSpecs\b|\bNavigationPointList\b"
    r"|\bNetMode\b|\bPawnList\b|GetLevelInfo\(|P_GET_\w+\(|P_FINISH|GetStateFrame\("
    r"|LatentAction|ProcessEvent"
)
for i, t in enumerate(segs):
    m = leftover.search(re.sub(r"//[^\n]*", "", t))
    if m:
        raise SystemExit("unrewritten leftover in span %d: %r" % (i, m.group(0)))

out = []
prev = 0
for (a, b), t in zip(spans, segs):
    out.append(s[prev:a])
    out.append(t)
    prev = b
out.append(s[prev:])
p.write_bytes("".join(out).encode("utf-8", "surrogateescape"))
PY

python3 - "$CC_DB" "$REPO" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, os, shlex, sys
cc, repo, work = sys.argv[1], sys.argv[2], sys.argv[3]
db = json.load(open(cc))
ents = [e for e in db if e["file"].endswith("HarryPotter2/Unreal/Engine/Src/UnPawn.cpp")]
if len(ents) != 1:
    raise SystemExit("UnPawn.cpp compile entries: %d" % len(ents))
e = ents[0]
argv = e["arguments"] if "arguments" in e else shlex.split(e["command"])

def absify(path):
    if not os.path.isabs(path):
        path = os.path.normpath(os.path.join(e["directory"], path))
    if path == repo or path.startswith(repo + os.sep):
        moved = work + path[len(repo):]
        if os.path.exists(moved):
            return moved
    return path

PATH_OPTS = {"-I", "-isystem", "-iquote", "-idirafter", "-F", "-include", "-imacros"}
DROP_WITH_ARG = {"-o", "-MF", "-MT", "-MQ"}
DROP = {"-c", "-MD", "-MMD", "-g"}
out = []
i = 1
while i < len(argv):
    a = argv[i]
    if a in DROP_WITH_ARG:
        i += 2
        continue
    if a in DROP or a.startswith("-O") or a.endswith(".cpp"):
        i += 1
        continue
    if a in PATH_OPTS:
        out += [a, absify(argv[i + 1])]
        i += 2
        continue
    for opt in ("-I", "-isystem", "-iquote"):
        if a.startswith(opt) and len(a) > len(opt):
            out.append(opt + absify(a[len(opt):]))
            break
    else:
        out.append(a)
    i += 1
print(" ".join(shlex.quote(t) for t in out))
PY
FLAGS=$(cat "$WORK/flags.txt")
cd "$WORK/HarryPotter2/Unreal/Engine/Src"
eval "c++ $FLAGS -O3 -g -c UnPawn.cpp -o $WORK/UnPawn.o"
eval "c++ $FLAGS -O3 -g -c $WORK/HarryPotter2/Unreal/Core/Src/UnMath.cpp -o $WORK/UnMath.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/pawn_native_move_fixture_harness/pawn_native_move_harness.cpp -o $WORK/harness.o"

python3 - "$WORK" > "$WORK/stubs.cpp" <<'PY'
import re, subprocess, sys

work = sys.argv[1]

def names(args, path):
    r = subprocess.run(["nm"] + args + [path], capture_output=True, text=True, check=True)
    out = set()
    for line in r.stdout.splitlines():
        parts = line.split()
        if parts:
            n = parts[-1]
            out.add(n[1:] if n.startswith("_") else n)
    return out

defined = names(["-g", "--defined-only"], f"{work}/harness.o") | names(["-g", "--defined-only"], f"{work}/UnMath.o")
undef = names(["-u"], f"{work}/UnPawn.o") | names(["-u"], f"{work}/UnMath.o")
ALLOW = re.compile(r"^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)")
LIBC = re.compile(r"^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc"
                  r"|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?"
                  r"|acos|asin|rand|srand|time|clock)$")
syms = sorted(n for n in undef if n not in defined and not ALLOW.match(n) and not LIBC.match(n))
bad = [n for n in syms if "PawnMove" in n]
if bad:
    raise SystemExit("unresolved PawnMove hooks: %s" % bad)
with open(f"{work}/syms.txt", "w") as f:
    f.write("\n".join(syms) + "\n")

FORBID = ("FindFunctionChecked", "ProcessEvent", "IsA", "IsProbing", "IsValid", "eventHearNoise",
          "eventEnemyNotVisible", "eventTouch", "eventSpecialHandling", "appFrand", "FastLineCheck",
          "SingleLineCheck", "GetCylinderExtent", "setPhysics", "SafeNormal", "Rotation", "GMath",
          "appSin", "appPow", "appIsNan", "appAtan2", "appRound", "appCycles", "GetStateFrame",
          "GetSuperState", "GetFName")
out = ["#include <cstdlib>"]
for i, s in enumerate(syms):
    if any(f in s for f in FORBID):
        out.append(f'extern "C" void trap{i}(void) __asm__("_{s}");')
        out.append(f"void trap{i}(void) {{ std::abort(); }}")
    else:
        out.append(f'__attribute__((used)) char stub{i}[64] __asm__("_{s}");')
print("\n".join(out))
print("stubs: %d" % len(syms), file=sys.stderr)
PY
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnPawn.o" "$WORK/UnMath.o" "$WORK/stubs.o" -o "$WORK/pawn_native_move_harness"

OUT=$REPO/crates/hp2-engine/tests/fixtures/pawn_native_move.txt
TMP1=$OUT.tmp.1
TMP2=$OUT.tmp.2
"$WORK/pawn_native_move_harness" > "$TMP1"
"$WORK/pawn_native_move_harness" > "$TMP2"
cmp "$TMP1" "$TMP2"
test "$(wc -l < "$TMP1")" -eq 576
for n in actorReachable findPathTo findPathToward findRandomDest makeNoise moveTo pollMoveTo \
    moveToward pollMoveToward strafeTo pollStrafeTo strafeFacing pollStrafeFacing turnToward \
    pollTurnToward turnTo pollTurnTo showSelf; do
  test "$(grep -c "^$n c=" "$TMP1")" -eq 32
done
mv "$TMP1" "$OUT"
rm "$TMP2"
printf 'wrote %s records\n' "$(wc -l < "$OUT")"
