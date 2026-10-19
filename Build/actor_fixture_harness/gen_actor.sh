#!/bin/sh
# Regenerate the UnActor.cpp (EngActorCore slice) oracle fixture from the original
# UnActor.cpp at 90d1e8d. Usage: gen_actor.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=90d1e8d  # last commit whose UnActor.cpp is the unported C++ (same blob as da56c30); the working tree is modified by the Rust forwards
SRC=HarryPotter2/Unreal/Engine/Src
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" HarryPotter2/Unreal | tar -x -C "$WORK"
ln -s "$REPO/out" "$WORK/out"

# Rewrite the oracle: CRLF -> LF, hook include, engine call sites -> ActorHook*.
python3 - "$WORK/$SRC/UnActor.cpp" "$REPO/Build/actor_fixture_harness/actor_hooks.h" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
hooks = sys.argv[2]
s = p.read_bytes().decode('latin-1').replace('\r\n', '\n')

inc = '#include "EnginePrivate.h"'
assert s.count(inc) == 1, s.count(inc)
s = s.replace(inc, inc + '\n#include "' + hooks + '"', 1)

def exact(old, new, n=1):
    global s
    c = s.count(old)
    assert c == n, (old, c)
    s = s.replace(old, new)

def generic(pat, fn, min_n=1):
    global s
    out, n = re.subn(pat, fn, s)
    assert n >= min_n, (pat, n)
    s = out

OBJ = r'((?:[A-Za-z_][\w\[\]\.]*(?:\(\))?->)*)'  # optional receiver chain, e.g. "Brush->", "Mesh->"

def obj_or_this(m):
    o = m.group(1)
    return o[:-2] if o else 'this'

# Class tests / flags / lifecycle on any receiver.
generic(OBJ + r'IsA\((\w+)::StaticClass\(\)\)',
        lambda m: 'ActorHookIsA(%s, "%s")' % (obj_or_this(m), m.group(2)))
generic(OBJ + r'IsBrush\(\)',
        lambda m: 'ActorHookIsBrush(%s)' % obj_or_this(m))
generic(OBJ + r'SetFlags\(', lambda m: 'ActorHookSetFlags(%s, ' % obj_or_this(m))
exact('AuxAnims(a)->ConditionalDestroy();', 'ActorHookConditionalDestroy(AuxAnims(a));')
generic(OBJ + r'ConditionalDestroy\(\)',
        lambda m: 'ActorHookConditionalDestroy(%s)' % obj_or_this(m))
generic(OBJ + r'RemoveFromRoot\(\)',
        lambda m: 'ActorHookRemoveFromRoot(%s)' % obj_or_this(m))
exact('delete RenderInterface;', 'ActorHookDeleteRI(RenderInterface);')
generic(OBJ + r'LoadLocalized\(\)',
        lambda m: 'ActorHookLoadLocalized(%s)' % obj_or_this(m))
generic(OBJ + r'ValidateActor\(this\)',
        lambda m: 'ActorHookValidate(%s, this)' % obj_or_this(m))
# Harness has no vtable for AActor, so unqualified virtual GetPrimitive() would load a null vptr. Call the base body directly.
generic(r'(?<![\w:.>])GetPrimitive\(\)', lambda m: 'AActor::GetPrimitive()')
generic(OBJ + r'GetCollisionExtent\(this\)',
        lambda m: 'ActorHookCollisionExtent(%s, (void*)this)' % obj_or_this(m))

# Geometry chains.
exact('Mesh->GetRenderBoundingBox( this, Exact )',
      'ActorHookMeshRenderBox(Mesh, this, Exact)')
exact('Brush->GetRenderBoundingBox( this, Exact ).GetCoords()',
      'ActorHookBrushRenderBox(Brush, this, Exact)')
exact('GetLevel()->SingleLineCheck(', 'ActorHookSingleLineCheck(GetLevel(), ')
exact('Hit.BspSurf()', 'ActorHookBspSurf(Hit)')
generic(r'GetLevel\(\)->Hash->(AddActor|RemoveActor)\( this \)',
        lambda m: 'ActorHookHash(this, "%s")' % m.group(1))
net_start = s.index('void AActor::PreNetReceive()')
net_end = s.index('/*-----------------------------------------------------------------------------', s.index('void AActor::PostNetReceive()', net_start))
net = s[net_start:net_end]
for old, new, expected in [
    ('GetLevel()->FarMoveActor(', 'ActorHookFarMove(GetLevel(), ', 2),
    ('GetLevel()->MoveActor(', 'ActorHookMoveActor(GetLevel(), ', 1),
    ('moveSmooth(', 'ActorHookMoveSmooth(this, ', 1),
    ('Mover->setPhysics(', 'ActorHookSetPhysics(Mover, ', 1),
]:
    assert net.count(old) == expected, (old, net.count(old))
    net = net.replace(old, new)
net, n = re.subn(r'Cast\s*<\s*(AMover|APawn|APlayerPawn|APlayerReplicationInfo)\s*>\s*\(\s*this\s*\)',
                 lambda m: 'ActorHookCast<%s>(this, "%s")' % (m.group(1), m.group(1)), net)
assert n >= 3, n
for cls in ('AMover', 'APawn', 'APlayerPawn', 'APlayerReplicationInfo'):
    net = net.replace('IsA(%s::StaticClass())' % cls,
                      'ActorHookIsA(this, "%s")' % cls)
s = s[:net_start] + net + s[net_end:]

exact('Super::ProcessEvent( Function, Parms, Result );',
      'ActorHookSuperProcessEvent( this, Function, Parms, Result );')
exact('GCache.Flush();', 'ActorHookCacheFlush();')

# Serialization.
exact('UClass::StaticClass()', 'ActorHookUClassStatic()', n=s.count('UClass::StaticClass()'))
exact('GetClass()->SerializeTaggedPersistentProperties( Ar, (BYTE*)this, GetClass() );',
      'ActorHookSerializeTagged(GetClass(), &Ar, this);')
exact('GetClass()->SerializeBin( Ar, (BYTE*)this );',
      'ActorHookSerializeBin(GetClass(), &Ar, this);')
exact('GetClass()->GetPropertiesSize()', 'ActorHookPropertiesSize(GetClass())')
exact('Ar.CountBytes( Size, Size );', 'ActorHookCountBytes(&Ar, Size);')
exact('Ar.Preload( TheClass );', 'ActorHookPreload(&Ar, TheClass);')
generic(r'Ar\.(IsLoading|IsSaving|IsTrans)\(\)',
        lambda m: 'ActorHook%s(&Ar)' % m.group(1))
exact('debugf( NAME_Log, TEXT("Possessed PlayerPawn: %s"), GetFullName() );',
      'ActorHookPossessed(this);')
exact('eventBump( SavedBase );', 'ActorHookEvent(this, "Bump", SavedBase);')
exact('SavedBase->eventBump( this );', 'ActorHookEvent(SavedBase, "Bump", this);')
exact('eventPossess();', 'ActorHookEvent(this, "Possess", NULL);')
exact('Actor->eventTouch( Other );', 'ActorHookEvent(Actor, "Touch", Other);')
exact('eventUnTouch( Other );', 'ActorHookEvent(this, "UnTouch", Other);')
exact('Other->eventUnTouch( this );', 'ActorHookEvent(Other, "UnTouch", this);')
exact('Owner->eventLostChild( this );', 'ActorHookEvent(Owner, "LostChild", this);')
exact('Owner->eventGainedChild( this );', 'ActorHookEvent(Owner, "GainedChild", this);')
exact('Base->eventDetach( this );', 'ActorHookEvent(Base, "Detach", this);')
exact('Base->eventAttach( this );', 'ActorHookEvent(Base, "Attach", this);')
exact('eventBaseChange();', 'ActorHookEvent(this, "BaseChange", NULL);')
exact('check(Actor->Touching[i]->IsValid());', 'check(ActorHookIsValid(Actor->Touching[i]));')

# execTraceTexture: P_GET_* macros become harness-provided arguments.
start = s.index('AActor::execTraceTexture')
end = s.index('FCheckResult Hit(1.0)', start)
head = s[start:end]
reps = [
    ('P_GET_VECTOR(TraceEnd);', 'FVector TraceEnd = ActorArgTraceEnd;'),
    ('P_GET_VECTOR(TraceStart);', 'FVector TraceStart = ActorArgTraceStart;'),
    ('P_GET_INT_REF(Flags);', 'INT* Flags = ActorArgFlags;'),
    ('P_GET_UBOOL_OPTX(bTraceDecals, 0);', 'UBOOL bTraceDecals = ActorArgDecals;'),
    ('P_FINISH;', ''),
]
for old, new in reps:
    assert head.count(old) == 1, (old, head.count(old))
    head = head.replace(old, new)
s = s[:start] + head + s[end:]
p.write_bytes(s.encode('latin-1'))
PY

FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" "$SRC/UnActor.cpp" "$REPO" "$WORK")
cd "$WORK/$SRC"
eval "c++ $FLAGS -O3 -g -c UnActor.cpp -o $WORK/UnActor.o"
(cd "$WORK/HarryPotter2/Unreal/Core/Src" && eval "c++ $FLAGS -O3 -g -c UnAnsi.cpp -o $WORK/UnAnsi.o")
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/actor_fixture_harness/actor_harness.cpp -o $WORK/harness.o"
{ nm -g --defined-only "$WORK/harness.o" "$WORK/UnAnsi.o"; } \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ nm -u "$WORK/UnActor.o"; nm -u "$WORK/UnAnsi.o"; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock|wcs[a-z]+)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
# Stubs abort with STUB; data is zero-filled. Any STUB line fails the run.
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" "$WORK/UnActor.o" "$WORK/UnAnsi.o" "$WORK/harness.o" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnActor.o" "$WORK/UnAnsi.o" "$WORK/stubs.o" -o "$WORK/actor_harness"

run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3" "$2"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/actor_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/actor_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
R="$WORK/run1.txt"
test "$(grep -c '^case ' "$R")" -eq 220
test "$(grep -c '^case get_skin ' "$R")" -eq 16
test "$(grep -c '^case get_net_priority ' "$R")" -eq 18
test "$(grep -c '^case set_collision ' "$R")" -eq 12
test "$(grep -c '^case set_collision_size ' "$R")" -eq 12
test "$(grep -c '^case set_owner ' "$R")" -eq 4
test "$(grep -c '^case set_base ' "$R")" -eq 12
test "$(grep -c '^case begin_touch ' "$R")" -eq 12
test "$(grep -c '^case end_touch ' "$R")" -eq 8
test "$(grep -c '^case is_overlapping ' "$R")" -eq 6
test "$(grep -c '^case get_visibility_box ' "$R")" -eq 3
test "$(grep -c '^case carcass_update_frequency ' "$R")" -eq 12
test "$(grep -c '^case get_hit_actor ' "$R")" -eq 2
test "$(grep -c '^case post_edit_change ' "$R")" -eq 2
test "$(grep -c '^case zone_info_post_edit_change ' "$R")" -eq 2
test "$(grep -c '^case get_primitive ' "$R")" -eq 12
test "$(grep -c '^case spawned ' "$R")" -eq 8
test "$(grep -c '^case process_event ' "$R")" -eq 4
test "$(grep -c '^case destroy ' "$R")" -eq 2
test "$(grep -c '^case post_load ' "$R")" -eq 16
test "$(grep -c '^case set_player ' "$R")" -eq 4
test "$(grep -c '^case get_cylinder_extent ' "$R")" -eq 14
test "$(grep -c '^case get_render_bounding_box ' "$R")" -eq 8
test "$(grep -c '^case serialize_persistence ' "$R")" -eq 6
test "$(grep -c '^case trace_texture ' "$R")" -eq 6
test "$(grep -c '^case trace_decal_flip ' "$R")" -eq 8
test "$(grep -c '^case net_receive ' "$R")" -eq 11
OUT=$REPO/crates/hp2-engine/tests/fixtures/actor_unactor.txt
cp "$R" "$OUT"
printf 'wrote %s records\n' "$(wc -l < "$OUT")"
