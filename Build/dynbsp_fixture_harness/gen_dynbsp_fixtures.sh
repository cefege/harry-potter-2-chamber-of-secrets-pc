#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/dynbsp_tracker.txt.
#
# Oracle = the ORIGINAL C++ UnDynBsp.cpp at 1b797b9 (pre-port body; HEAD's copy is the Rust shell), compiled with the real flags from
# out/macos-arm64/compile_commands.json at -O3. HEAD's UnFPoly/UnModel/UnTrace are shells over
# hp2_rs_engine_*, so those three TUs come from 1b797b9 (their pre-port C++ bodies). The Rust engine
# lib is NOT linked: the oracle never calls Rust.
#
# Stubs: engine symbols (`__Z*` functions, `_G[A-Z]*` data) that the oracle objects reference but nothing
# defines get a 256-byte zero-filled blob in __DATA (not executable). Calling a stubbed function faults;
# reading a stubbed data global reads 0. So the generator fails if a tracker object (UnDynBsp_oracle,
# UnModel, UnFPoly, UnTrace) references a stubbed `_G*` data symbol. libc/libc++/libc++abi/libSystem are never stubbed.
#
# Usage: gen_dynbsp_fixtures.sh [build-dir=out/macos-arm64] [git-ref=3b88e2c]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
BUILD=$(cd "${1:-$REPO/out/macos-arm64}" && pwd)
CC_DB="$BUILD/compile_commands.json"
REF=${2:-3b88e2c}
PRE=1b797b9
WORK=$(mktemp -d)
trap '[ $? -eq 0 ] && rm -rf "$WORK" || echo "generator failed; work dir kept: $WORK" >&2' EXIT

git -C "$REPO" archive "$REF" HarryPotter2 | tar -x -C "$WORK"
SRC="$WORK/HarryPotter2/Unreal"
for f in UnFPoly UnModel UnTrace; do
	git -C "$REPO" show "$PRE:HarryPotter2/Unreal/Engine/Src/$f.cpp" > "$SRC/Engine/Src/$f.cpp"
done
# Class storage comes from the harness (asm-labelled pointers); drop the lazy IMPLEMENT_CLASS(ABrush).
n=$(git -C "$REPO" show "$PRE:HarryPotter2/Unreal/Engine/Src/UnDynBsp.cpp" | grep -c 'IMPLEMENT_CLASS(ABrush)')
[ "$n" -eq 1 ] || { echo "expected one IMPLEMENT_CLASS(ABrush), found $n" >&2; exit 1; }
git -C "$REPO" show "$PRE:HarryPotter2/Unreal/Engine/Src/UnDynBsp.cpp" | grep -v 'IMPLEMENT_CLASS(ABrush)' > "$SRC/Engine/Src/UnDynBsp_oracle.cpp"

# Compile flags for one TU, from its compile DB entry. Only the snapshot's HarryPotter2 prefix is
# rewritten; build-tree includes (generated headers under out/) keep their real paths.
flags_for() {
python3 - "$CC_DB" "$REPO" "$WORK" "$1" <<'PY'
import json, shlex, sys
db, repo, work, name = sys.argv[1:5]
e = [x for x in json.load(open(db)) if x['file'].endswith('/' + name)][0]
toks = shlex.split(e['command'].replace(repo + '/HarryPotter2', work + '/HarryPotter2'))
out, skip = [], False
for t in toks:
    if skip: skip = False; continue
    if t in ('-o', '-MF', '-MT'): skip = True; continue
    if t in ('-MD', '-MMD', '-c') or t.startswith('-O') or t.endswith('.cpp') or t.endswith('c++'): continue
    out.append(t)
print(' '.join(shlex.quote(t) for t in out))
PY
}

OBJS=""
for f in Core/Src/Core Core/Src/UnMath Core/Src/UnMem Core/Src/UnName Core/Src/UnMisc Core/Src/UnAnsi Engine/Src/UnFPoly Engine/Src/UnModel Engine/Src/UnTrace Engine/Src/UnDynBsp_oracle; do
	base=$(basename "$f")
	ref=$base.cpp
	case "$base" in UnDynBsp_oracle) ref=UnDynBsp.cpp ;; esac
	eval "c++ -c -O3 $(flags_for "$ref") \"$SRC/$f.cpp\" -o \"$WORK/$base.o\""
	OBJS="$OBJS $WORK/$base.o"
done
eval "c++ -c -O0 $(flags_for UnDynBsp.cpp) \"$REPO/Build/dynbsp_fixture_harness/dynbsp_harness.cpp\" -o \"$WORK/harness.o\""
OBJS="$OBJS $WORK/harness.o"

# Referenced-but-undefined mangled names across all objects (LC_ALL=C: byte order for comm).
export LC_ALL=C
nm -u $OBJS | awk 'NF==1{print $1}' | sort -u > "$WORK/undef.txt"
nm -g --defined-only $OBJS | awk 'NF==3{print $3}' | sort -u > "$WORK/def.txt"
comm -23 "$WORK/undef.txt" "$WORK/def.txt" > "$WORK/missing.txt"
python3 - "$WORK/missing.txt" > "$WORK/syms.txt" <<'PY'
import re, sys
for s in open(sys.argv[1]).read().split():
    if re.search(r'cxxabiv|NSt|St[0-9]|__Z[nd][aw]?[A-Za-z]', s):
        continue  # libc++/libc++abi/operator new+delete: system-provided, never stubbed
    if s.startswith('__Z') or re.match(r'^_G[A-Z]', s):
        print(s)
PY
# Tracker objects must not read stubbed data globals: a zero stub would silently pick a branch.
for o in UnDynBsp_oracle UnModel UnFPoly UnTrace; do nm -u "$WORK/$o.o"; done | awk 'NF==1 && $1 !~ /:$/{print $1}' | sort -u > "$WORK/tracker_undef.txt"
bad=$(comm -12 "$WORK/tracker_undef.txt" "$WORK/syms.txt" | grep '^_G' || true)
[ -z "$bad" ] || { echo "tracker objects read stubbed data globals (would silently read 0):" >&2; echo "$bad" >&2; exit 1; }
{
	i=0
	while read -r s; do
		[ -n "$s" ] || continue
		case "$s" in
		*GRegisterNative*) echo "void nop$i(void) __asm__(\"$s\"); void nop$i(void) {}" ;;  # static-init registrar: no-op body so main() runs
		*) echo "__attribute__((aligned(16), used)) unsigned char stub$i[256] __asm__(\"$s\");" ;;
		esac
		i=$((i + 1))
	done < "$WORK/syms.txt"
} > "$WORK/stubs.c"
cc -c "$WORK/stubs.c" -o "$WORK/stubs.o"

eval "c++ $OBJS $WORK/stubs.o -o $WORK/dynbsp_oracle"
echo "fused multiply-add instructions in tracker objects: $(for o in UnDynBsp_oracle UnFPoly UnModel UnTrace; do otool -tV "$WORK/$o.o"; done | grep -cE '[[:space:]](fmadd|fmsub|fnmadd|fnmsub)[[:space:]]' || true)" >&2
"$WORK/dynbsp_oracle" > "$WORK/out.txt"
python3 - "$WORK/out.txt" <<'PY' || { echo "fixture assertions failed; work dir kept: $WORK" >&2; exit 1; }
import sys
# Per-case LIVE = sporadic nodes on mover brush MoverLink chains, dumped before each END_STATE.
GROW = {"two_movers_partial_outside", "coplanar_front_inside", "mover_outside_plus_x", "mover_straddles_plus_xy",
        "coplanar_back_outside", "three_movers_outside", "regrow_after_flush"}
ZERO = {"single_mover_inside_rotate"}
SINGLE = {"coplanar_front_inside", "mover_outside_plus_x", "mover_straddles_plus_xy", "coplanar_back_outside", "regrow_after_flush"}
cases, blocks, name, op, ends, nb = {}, {}, None, None, 0, 0
def close(name, op, nb):
    if op is not None: blocks.setdefault(name, []).append((op, nb))
for line in open(sys.argv[1]):
    t = line.split()
    if not t: continue
    if t[0] == 'CASE': close(name, op, nb); name, op, nb = t[1], None, 0; cases[name] = []
    elif t[0] == 'END': close(name, op, nb); op, nb = None, 0; ends += 1
    elif t[0] == 'OP' and t[1] != 'POSE': close(name, op, nb); op, nb = ' '.join(t[1:]), 0
    elif t[0] == 'NODE': nb += 1
    elif t[0] == 'LIVE': cases[name].append((op, int(t[1])))
bad = []
NAN = {'7fc00000', 'ffc00000'}  # float32 NaN bit patterns printed by hexf
for line in open(sys.argv[1]):
    t = line.split()
    if t and t[0] != 'OP' and NAN & set(t): bad.append('NaN in dump: ' + line.strip())
if set(cases) != GROW | ZERO or ends != len(cases): bad.append('case set / END count: ' + ' '.join(sorted(cases)))
for n, ops in cases.items():
    vals = [v for _, v in ops]
    bl = blocks[n]
    if ops[:2] != [('INIT', 0), ('CTOR', 0)]: bad.append(n + ': INIT/CTOR LIVE not 0')
    if bl[0][0] != 'INIT' or bl[-1][0] != 'DTOR' or bl[-1][1] != bl[0][1]: bad.append(n + ': DTOR did not return NODE count to the INIT baseline')
    if n in ZERO and any(vals): bad.append(n + ': sporadic nodes on an inside mover')
    if n in GROW and not any(o.startswith('UPDATE') and v > 0 for o, v in ops): bad.append(n + ': Update inserted no sporadic node')
    if n in SINGLE and ops[-2] != ('FLUSH 0', 0): bad.append(n + ': FLUSH 0 did not return LIVE to 0')
    print(n + ': ' + ' '.join(f'{o}={v}' for o, v in ops), file=sys.stderr)
if bad:
    print('\n'.join(bad), file=sys.stderr)
    sys.exit(1)
PY
mv "$WORK/out.txt" "$REPO/crates/hp2-engine/tests/fixtures/dynbsp_tracker.txt"
echo "wrote $(wc -l < "$REPO/crates/hp2-engine/tests/fixtures/dynbsp_tracker.txt" | tr -d ' ') lines; $(wc -l < "$WORK/syms.txt" | tr -d ' ') engine symbols stubbed"
