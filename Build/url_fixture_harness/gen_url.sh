#!/bin/sh
# Regenerates crates/hp2-engine/tests/fixtures/url_fixture.txt from the ORIGINAL
# UnURL.cpp at the pinned ref (git archive, not the working tree). Core string
# helpers come from the real UnAnsi.cpp and UnMisc.cpp at the same ref.
#
# Usage: gen_url.sh <build-dir-with-compile_commands.json> [git-ref]
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=${2:-42a1e81}
FIX=$REPO/crates/hp2-engine/tests/fixtures
WORK=$(mktemp -d "${TMPDIR:-/tmp}/url_fixture.XXXXXX")
trap '[ $? -eq 0 ] && rm -rf "$WORK"' EXIT

git -C "$REPO" archive "$REF" | tar -x -C "$WORK"

FLAGS=$(python3 - "$CC_DB" "$REPO" "$WORK" <<'EOF'
import json, shlex, sys
db, repo, work = sys.argv[1], sys.argv[2], sys.argv[3]
src = "HarryPotter2/Unreal/Engine/Src/UnURL.cpp"
for e in json.load(open(db)):
    if e["file"].endswith("/" + src):
        argv = shlex.split(e["command"])
        out, skip = [], False
        for a in argv[1:]:
            if skip:
                skip = False
                continue
            if a in ("-o", "-MF", "-MT", "-MQ"):
                skip = True
                continue
            if a == "-c" or a.startswith("-O") or a.endswith(".cpp") or a.startswith("-g"):
                continue
            out.append(a)
        print(" ".join(shlex.quote(a.replace(repo + "/HarryPotter2", work + "/HarryPotter2")) for a in out))
        sys.exit(0)
sys.exit("no compile command for " + src)
EOF
)

CXX="c++ $FLAGS -O3 -g -ffp-contract=on"
SRC=$WORK/HarryPotter2/Unreal
$CXX -c "$SRC/Engine/Src/UnURL.cpp" -o "$WORK/url.o"
$CXX -c "$SRC/Core/Src/UnAnsi.cpp" -o "$WORK/ansi.o"
$CXX -c "$SRC/Core/Src/UnMisc.cpp" -o "$WORK/misc.o"
$CXX -c "$REPO/Build/url_fixture_harness/url_harness.cpp" -o "$WORK/harness.o"

# Abort stubs for engine/core symbols the objects reference but do not define.
# Mach-O names carry a leading underscore; asm labels use the raw nm name.
# A mangled name whose demangled form has no parameter list is a variable and
# gets a zeroed data blob; anything else is a function that aborts when called.
# libc and libc++ symbols are left to the system libraries.
python3 - "$WORK" "$WORK/url.o" "$WORK/ansi.o" "$WORK/misc.o" "$WORK/harness.o" <<'EOF'
import subprocess, sys
work, objs = sys.argv[1], sys.argv[2:]
defined, undefined = set(), set()
for o in objs:
    for line in subprocess.run(["nm", "-g", "--defined-only", o], capture_output=True, text=True).stdout.splitlines():
        defined.add(line.split()[-1])
    for line in subprocess.run(["nm", "-u", o], capture_output=True, text=True).stdout.splitlines():
        undefined.add(line.split()[-1])
todo = sorted(s for s in undefined - defined if not s.startswith(("__ZNSt", "__ZNKSt", "__ZSt", "__Zd", "__Zn", "__Za")))
demangled = subprocess.run(["c++filt"], input="\n".join(todo), capture_output=True, text=True).stdout.splitlines()
out = ['#include <cstdio>', '#include <cstdlib>']
n = 0
for s, dm in zip(todo, demangled):
    n += 1
    if s.startswith("_Z") or s.startswith("__Z"):
        if "(" in dm:
            out.append(f'extern "C" void stub_{n}() __asm__("{s}");')
            out.append(f'void stub_{n}() {{ fprintf(stderr, "STUB %s\\n", "{s}"); abort(); }}')
        else:
            out.append(f'extern "C" char stub_{n}[4096] __asm__("{s}") = {{}};')
    elif s.startswith("_"):
        plain = s[1:]
        if plain[:1] == "G" and len(plain) > 1 and plain[1].isupper() or plain.startswith("LocalPackageName"):
            out.append(f'extern "C" char stub_{n}[4096] __asm__("{s}") = {{}};')
        elif plain.startswith("app"):
            out.append(f'extern "C" void stub_{n}() __asm__("{s}");')
            out.append(f'void stub_{n}() {{ fprintf(stderr, "STUB %s\\n", "{plain}"); abort(); }}')
open(work + "/stubs.cpp", "w").write("\n".join(out) + "\n")
EOF
$CXX -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ -O3 -g "$WORK/harness.o" "$WORK/url.o" "$WORK/ansi.o" "$WORK/misc.o" "$WORK/stubs.o" -o "$WORK/url_fixture"

"$WORK/url_fixture" > "$WORK/url_fixture.txt"
mkdir -p "$FIX"
cp "$WORK/url_fixture.txt" "$FIX/url_fixture.txt"
echo "records: $(wc -l < "$FIX/url_fixture.txt")"
