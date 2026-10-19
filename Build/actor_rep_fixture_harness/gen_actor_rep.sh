#!/bin/sh
# Regenerate the actor replication oracle fixture from original UnActor.cpp at 90d1e8d.
# Usage: gen_actor_rep.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=90d1e8d
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Engine/Src/UnActor.cpp "$REPO" "$WORK")
cd "$WORK/HarryPotter2/Unreal/Engine/Src"
eval "c++ $FLAGS -O3 -g -c UnActor.cpp -o $WORK/UnActor.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/actor_rep_fixture_harness/actor_rep_harness.cpp -o $WORK/harness.o"
{ nm -g --defined-only "$WORK/harness.o"; nm -g --defined-only "$WORK/UnActor.o"; } \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ nm -u "$WORK/UnActor.o"; nm -u "$WORK/harness.o"; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock|wcs[a-z]+)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
# Stubs: functions abort with STUB; data is zero-filled char[64] (bss-equivalent). Any stub hit fails the run.
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" "$WORK/UnActor.o" "$WORK/harness.o" > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnActor.o" "$WORK/stubs.o" -o "$WORK/actor_rep_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/actor_rep_replication.txt
run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/actor_rep_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/actor_rep_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
R="$WORK/run1.txt"
[ "$(grep -c '^case actor_rep ' "$R")" -eq 246 ] || { echo "count mismatch: actor_rep" >&2; exit 1; }
[ "$(grep -c '^case actor_freq ' "$R")" -eq 313 ] || { echo "count mismatch: actor_freq" >&2; exit 1; }
[ "$(grep -c '^case pawn_rep ' "$R")" -eq 297 ] || { echo "count mismatch: pawn_rep" >&2; exit 1; }
[ "$(grep -c '^case player_pawn_rep ' "$R")" -eq 339 ] || { echo "count mismatch: player_pawn_rep" >&2; exit 1; }
[ "$(grep -c '^case mover_rep ' "$R")" -eq 267 ] || { echo "count mismatch: mover_rep" >&2; exit 1; }
[ "$(grep -c '^case zone_rep ' "$R")" -eq 273 ] || { echo "count mismatch: zone_rep" >&2; exit 1; }
[ "$(grep -c '^case level_rep ' "$R")" -eq 288 ] || { echo "count mismatch: level_rep" >&2; exit 1; }
[ "$(grep -c '^case pri_rep ' "$R")" -eq 315 ] || { echo "count mismatch: pri_rep" >&2; exit 1; }
[ "$(grep -c '^case gri_rep ' "$R")" -eq 306 ] || { echo "count mismatch: gri_rep" >&2; exit 1; }
[ "$(grep -c '^case inv_rep ' "$R")" -eq 279 ] || { echo "count mismatch: inv_rep" >&2; exit 1; }
[ "$(grep -c '^case inv_freq ' "$R")" -eq 279 ] || { echo "count mismatch: inv_freq" >&2; exit 1; }
[ "$(grep -c '^case inv_script ' "$R")" -eq 279 ] || { echo "count mismatch: inv_script" >&2; exit 1; }
[ "$(grep -c '^case inv_novars ' "$R")" -eq 279 ] || { echo "count mismatch: inv_novars" >&2; exit 1; }
[ "$(grep -c '^case weapon_rep ' "$R")" -eq 288 ] || { echo "count mismatch: weapon_rep" >&2; exit 1; }
[ "$(grep -c '^case ammo_rep ' "$R")" -eq 285 ] || { echo "count mismatch: ammo_rep" >&2; exit 1; }
[ "$(grep -c '^case pickup_rep ' "$R")" -eq 282 ] || { echo "count mismatch: pickup_rep" >&2; exit 1; }
[ "$(grep -c '^case ' "$R")" -eq 4615 ] || { echo "count mismatch: total records" >&2; exit 1; }
[ "$(wc -l < "$R")" -eq 10380 ] || { echo "count mismatch: total lines" >&2; exit 1; }
cp "$R" "$OUT"
printf 'wrote %s lines\n' "$(wc -l < "$OUT")"
