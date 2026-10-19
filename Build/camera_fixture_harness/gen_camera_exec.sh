#!/bin/sh
# Regenerate the UViewport exec-path oracle fixtures from the original UnCamera.cpp at 69c3263.
# Usage: gen_camera_exec.sh <build-dir-with-compile_commands.json>
# Writes camera_bitmap.txt (bitmap family) and camera_exec.txt (exec, lock, unlock, setdrag,
# iswire, readinput families) under crates/hp2-engine/tests/fixtures/.
# The original TU is compiled unmodified; callees outside it are recording doubles in camera_exec_harness.cpp.
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
CAM_FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" "$CAM_REL" "$REPO" "$WORK")
ANSI_FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" "$ANSI_REL" "$REPO" "$WORK")
cd "$WORK/$(dirname "$CAM_REL")"
eval "c++ $CAM_FLAGS -O3 -ffp-contract=on -g -c UnCamera.cpp -o $WORK/UnCamera.o"
cd "$WORK/$(dirname "$ANSI_REL")"
eval "c++ $ANSI_FLAGS -O3 -ffp-contract=on -g -c UnAnsi.cpp -o $WORK/UnAnsi.o"
cd "$WORK/$(dirname "$MATH_REL")"
eval "c++ $ANSI_FLAGS -O3 -ffp-contract=on -g -c UnMath.cpp -o $WORK/UnMath.o"
eval "c++ $CAM_FLAGS -O3 -ffp-contract=on -g -fno-access-control -c $REPO/Build/camera_fixture_harness/camera_exec_harness.cpp -o $WORK/harness.o"
# UnCamera.o must be in the defined set: the harness references UViewport::/URenderDevice:: symbols it defines.
{ nm -g --defined-only "$WORK/harness.o" "$WORK/UnCamera.o" "$WORK/UnAnsi.o" "$WORK/UnMath.o"; } \
  | awk '!/:$/ {print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ nm -u "$WORK/UnCamera.o" "$WORK/UnAnsi.o" "$WORK/UnMath.o" "$WORK/harness.o"; } | awk '!/:$/ {print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock|wcs[a-z]+)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
# Stubs: functions abort with STUB; data is zero-filled char[64]. Any stub hit fails the run.
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" "$WORK/UnCamera.o" "$WORK/UnAnsi.o" "$WORK/UnMath.o" "$WORK/harness.o" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnCamera.o" "$WORK/UnAnsi.o" "$WORK/UnMath.o" "$WORK/stubs.o" -o "$WORK/camera_exec_harness"
run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/camera_exec_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/camera_exec_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
R="$WORK/run1.txt"
# Split by family: bitmap keeps its own fixture; every other family shares camera_exec.txt.
awk -v dir="$WORK" '/^case /{ t = ($2 == "bitmap") ? "bitmap" : "exec" } { print > (dir "/split_" t ".txt") }' "$R"
B="$WORK/split_bitmap.txt"; E="$WORK/split_exec.txt"
FIX=$REPO/crates/hp2-engine/tests/fixtures
test "$(grep -c '^case bitmap ' "$B")" -eq 6
test "$(grep -c '^  STATE ' "$B")" -eq 6
test "$(grep -c '^case exec ' "$E")" -eq 35
test "$(grep -c '^case lock ' "$E")" -eq 5
test "$(grep -c '^case unlock ' "$E")" -eq 3
test "$(grep -c '^case setdrag ' "$E")" -eq 3
test "$(grep -c '^case iswire ' "$E")" -eq 8
test "$(grep -c '^case readinput ' "$E")" -eq 3
test "$(grep -c '^  STATE ' "$E")" -eq 57
cp "$B" "$FIX/camera_bitmap.txt"
cp "$E" "$FIX/camera_exec.txt"
[ -z "${KEEP_OBJ:-}" ] || cp "$WORK/UnCamera.o" "$KEEP_OBJ"
printf 'wrote %s bitmap, %s exec lines\n' "$(wc -l < "$B")" "$(wc -l < "$E")"
