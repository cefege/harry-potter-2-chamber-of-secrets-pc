#!/bin/sh
# Regenerate the camera oracle fixtures from the original UnCamera.cpp at 69c3263.
# Usage: gen_camera.sh <build-dir-with-compile_commands.json>
# Writes camera_render_size.txt (render_size + render_size_persp), camera_render_coords.txt and
# camera_draw_3d_line.txt under crates/hp2-engine/tests/fixtures/.
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=69c3263
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
CAM_REL=HarryPotter2/Unreal/Engine/Src/UnCamera.cpp
ANSI_REL=$(git -C "$REPO" ls-tree -r --name-only "$REF" | grep -E '/UnAnsi\.cpp$' | head -1)
MATH_REL=$(git -C "$REPO" ls-tree -r --name-only "$REF" | grep -E '/UnMath\.cpp$' | head -1)
python3 - "$WORK/$CAM_REL" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
s = p.read_text()

def swap(s, old, new):
    assert s.count(old) == 1, (old, s.count(old))
    return s.replace(old, new, 1)

# Virtual boundary calls (UnRenDev.h:189-192) become hooks defined in camera_harness.cpp.
s = swap(s, '\tViewport->RenDev->SetSceneNode( this );', '\tCameraHookSetSceneNode( Viewport, this );')
s = swap(s, 'Draw2DLine( Frame, Color, LineFlags, P1, P2 );', 'CameraHookDraw2DLine( this, Frame, Color, LineFlags, P1, P2 );')
s = swap(s, 'Draw2DPoint( Frame, Color, LINE_None, P1.X-1, P1.Y-1, P1.X+1, P1.Y+1, 1.f );',
         'CameraHookDraw2DPoint( this, Frame, Color, LINE_None, P1.X-1, P1.Y-1, P1.X+1, P1.Y+1, 1.f );')
# Qualified, non-virtual call: runs the original clip body without a vtable on the calloc'd device.
s = swap(s, '\tDraw2DClippedLine( Frame, Color, LineFlags, P1, P2 );',
         '\tURenderDevice::Draw2DClippedLine( Frame, Color, LineFlags, P1, P2 );')
# Hook prototypes go after the last #include so the engine types are complete.
lines = s.split('\n')
last = max(i for i, l in enumerate(lines) if l.startswith('#include'))
lines[last + 1:last + 1] = [
    'void CameraHookSetSceneNode( UViewport*, FSceneNode* );',
    'void CameraHookDraw2DLine( URenderDevice*, FSceneNode*, FPlane, DWORD, FVector, FVector );',
    'void CameraHookDraw2DPoint( URenderDevice*, FSceneNode*, FPlane, DWORD, FLOAT, FLOAT, FLOAT, FLOAT, FLOAT );',
]
s = '\n'.join(lines)
p.write_text(s)
PY
CAM_FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" "$CAM_REL" "$REPO" "$WORK")
ANSI_FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" "$ANSI_REL" "$REPO" "$WORK")
cd "$WORK/$(dirname "$CAM_REL")"
eval "c++ $CAM_FLAGS -O3 -ffp-contract=on -g -c UnCamera.cpp -o $WORK/UnCamera.o"
cd "$WORK/$(dirname "$ANSI_REL")"
eval "c++ $ANSI_FLAGS -O3 -ffp-contract=on -g -c UnAnsi.cpp -o $WORK/UnAnsi.o"
cd "$WORK/$(dirname "$MATH_REL")"
eval "c++ $ANSI_FLAGS -O3 -ffp-contract=on -g -c UnMath.cpp -o $WORK/UnMath.o"
eval "c++ $CAM_FLAGS -O3 -ffp-contract=on -g -fno-access-control -c $REPO/Build/camera_fixture_harness/camera_harness.cpp -o $WORK/harness.o"
{ nm -g --defined-only "$WORK/harness.o" "$WORK/UnCamera.o" "$WORK/UnAnsi.o" "$WORK/UnMath.o"; } \
  | awk '!/:$/ {print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ nm -u "$WORK/UnCamera.o" "$WORK/UnAnsi.o" "$WORK/UnMath.o" "$WORK/harness.o"; } | awk '!/:$/ {print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock|wcs[a-z]+)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
# Stubs: functions abort with STUB; data is zero-filled char[64]. Any stub hit fails the run.
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" "$WORK/UnCamera.o" "$WORK/UnAnsi.o" "$WORK/UnMath.o" "$WORK/harness.o" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnCamera.o" "$WORK/UnAnsi.o" "$WORK/UnMath.o" "$WORK/stubs.o" -o "$WORK/camera_harness"
run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/camera_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/camera_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
R="$WORK/run1.txt"
# Split by family: render_size and render_size_persp share one fixture; each family keeps its record order.
awk -v dir="$WORK" '/^case /{ f=$2; if (f=="render_size" || f=="render_size_persp") t="size"; else if (f=="render_coords") t="coords"; else t="draw3d" } { print > (dir "/split_" t ".txt") }' "$R"
S="$WORK/split_size.txt"; C="$WORK/split_coords.txt"; D="$WORK/split_draw3d.txt"
FIX=$REPO/crates/hp2-engine/tests/fixtures
test "$(grep -c '^case render_size ' "$S")" -eq 48
test "$(grep -c '^case render_size_persp ' "$S")" -eq 64
test "$(grep -c '^out ' "$S")" -eq 112
test "$(grep -c '^setscene 1$' "$S")" -eq 112
test "$(grep -c '^case render_coords ' "$C")" -eq 48
test "$(grep -c '^coords ' "$C")" -eq 48
test "$(grep -c '^uc ' "$C")" -eq 48
test "$(grep -c '^out ' "$C")" -eq 48
test "$(grep -c '^setscene 1$' "$C")" -eq 48
test "$(grep -c '^case draw3d ' "$D")" -eq 96
test "$(grep -c '^calls ' "$D")" -eq 96
for b in ortho_line ortho_dot ortho_dot_hidden persp_line persp_near_p1 persp_discard; do
  test "$(grep -c "^case draw3d [0-9]* branch=$b " "$D")" -eq 16
done
cp "$S" "$FIX/camera_render_size.txt"
cp "$C" "$FIX/camera_render_coords.txt"
cp "$D" "$FIX/camera_draw_3d_line.txt"
[ -z "${KEEP_OBJ:-}" ] || cp "$WORK/UnCamera.o" "$KEEP_OBJ"
printf 'wrote %s size, %s coords, %s draw3d lines\n' "$(wc -l < "$S")" "$(wc -l < "$C")" "$(wc -l < "$D")"
