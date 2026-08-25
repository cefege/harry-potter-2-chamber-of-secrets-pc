#!/usr/bin/env bash
# Cutover: preserve the C++ oracle on a branch, then remove it from HEAD.
# Prescribed by the rewrite plan (Phase 6 step 4). DO NOT RUN until the
# parallel HP1 C++ session has landed its work — deleting HarryPotter2/Unreal
# C++ from HEAD while another session edits it will destroy that work.
#
# Usage: bash Build/cutover.sh
# Preconditions: clean working tree; retail product gate closed; parity
# evidence recorded (Docs/BEHAVIOR_MATRIX.md G6 row references it).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== 1. Preserve C++ oracle on cpp-reference branch =="
if git show-ref --verify --quiet refs/heads/cpp-reference; then
    echo "cpp-reference already exists; skipping branch creation"
else
    git branch cpp-reference
fi

echo "== 2. Delete legacy C++ trees from HEAD =="
git rm -r -q --ignore-unmatch \
    HarryPotter2/Unreal/Core \
    HarryPotter2/Unreal/Engine \
    HarryPotter2/Unreal/Render \
    HarryPotter2/Unreal/Editor \
    HarryPotter2/Unreal/Launch \
    HarryPotter2/Unreal/SDLLaunch \
    HarryPotter2/Unreal/ALAudio \
    HarryPotter2/Unreal/Vorbis \
    HarryPotter2/Unreal/OpenAL \
    ThirdParty/XOpenGLDrv \
    ThirdParty/UT99VulkanDrv \
    ThirdParty/vgmstream \
    dist/macos-arm64

echo "== 3. Remove C++ build targets =="
python3 - <<'PY'
# Strip CMake C++ target registrations; keep the harness-script tests that
# drive --engine-bin (they are era-agnostic). A human should review this diff.
import re
p = 'Build/CMake/HP2Targets.cmake'
s = open(p).read()
print(f"NOTE {p}: review manually — CMake target surgery is intentionally "
      "not automated. Expected end state: hp2_add_behavior_test entries that "
      "invoke Python harness scripts remain; native C++ test binaries and "
      "the C++ executable/library targets go.")
PY

echo "== 4. Commit =="
git commit -q -m "cutover: remove legacy c++ trees from head (preserved on cpp-reference)"

echo "== 5. Post-cutover verification =="
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --workspace
cargo test --workspace
python3 Build/check_bundle_rs.py

echo "CUTOVER COMPLETE — run G6 checklist (fresh clone + retail flow) next."
